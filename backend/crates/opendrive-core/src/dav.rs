//! Adaptador WebDAV sobre el árbol de nodos de OpenDrive.
//!
//! # Cómo se mapean las rutas
//!
//! La raíz de WebDAV (`/`) corresponde a la carpeta «Mi Drive» del usuario
//! autenticado. Cada segmento de la ruta es el nombre de un nodo:
//!
//! ```text
//! /Documentos/2026/informe.pdf  ->  Mi Drive / Documentos / 2026 / informe.pdf
//! ```
//!
//! # Autenticación
//!
//! El sistema de archivos es `GuardedFileSystem<DavUser>`: cada operación
//! recibe las credenciales del usuario y comprueba que el nodo le pertenece.
//! Un usuario no puede ver, leer ni escribir nada de otro.
//!
//! # Escrituras
//!
//! WebDAV no admite subidas incrementales: el cliente envía el archivo entero
//! en una sola petición. El contenido se escribe primero en un archivo
//! temporal del almacén de blobs y se confirma al vaciar el archivo, de forma
//! que un fallo a mitad no deja basura.

use std::io::SeekFrom;
use std::time::SystemTime;

use crate::db::NodeRepository;
use crate::db::nodes::NewFile;
use crate::storage::{BlobStore, BlobWriter};
use bytes::{Buf, Bytes};
use dav_server::davpath::DavPath;
use dav_server::fs::{
    DavDirEntry, DavFile, DavMetaData, FsError, FsFuture, FsResult, FsStream, GuardedFileSystem,
    OpenOptions, ReadDirMeta,
};
use futures_util::stream::StreamExt;
use sqlx::PgPool;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use uuid::Uuid;

use crate::error::Error;
use crate::models::node::{Node, NodeKind};

/// Credenciales del usuario en una petición WebDAV.
#[derive(Debug, Clone)]
pub struct DavUser {
    /// Identificador del usuario autenticado.
    pub user_id: Uuid,
    /// Nombre de usuario, que se usa como principal de los bloqueos.
    pub username: String,
}

/// Tamaño máximo de un archivo escrito por WebDAV (512 MiB).
///
/// WebDAV no admite subidas incrementales: el cliente envía el archivo entero
/// en una sola petición. El límite evita que una petición agote la memoria.
const MAX_WRITE_SIZE: u64 = 512 * 1024 * 1024;

/// Sistema de archivos WebDAV sobre el árbol de nodos.
#[derive(Debug, Clone)]
pub struct DavFs {
    pool: PgPool,
    nodes: NodeRepository,
    blobs: BlobStore,
}

impl DavFs {
    /// Crea el sistema de archivos.
    pub fn new(pool: PgPool, nodes: NodeRepository, blobs: BlobStore) -> Self {
        Self { pool, nodes, blobs }
    }

    /// Divide una ruta WebDAV en sus segmentos.
    fn segments(path: &DavPath) -> Vec<String> {
        path.as_bytes()
            .split(|byte| *byte == b'/')
            .filter(|segment| !segment.is_empty())
            .map(|segment| String::from_utf8_lossy(segment).into_owned())
            .collect()
    }

    /// Convierte una marca de tiempo de Postgres en `SystemTime`.
    fn system_time(timestamp: i64) -> SystemTime {
        if timestamp < 0 {
            return SystemTime::UNIX_EPOCH;
        }
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64)
    }

    /// Resuelve una ruta WebDAV en un nodo del árbol.
    ///
    /// La búsqueda parte de la carpeta raíz del usuario y baja por cada
    /// segmento. Si un segmento no existe o no le pertenece, devuelve `None`.
    async fn resolve(&self, path: &DavPath, user_id: Uuid) -> Option<Node> {
        let mut current = self.nodes.get_root(user_id).await.ok()??;

        for segment in Self::segments(path) {
            current = self
                .nodes
                .find_child(user_id, current.id, &segment)
                .await
                .ok()??;
        }

        Some(current)
    }

    /// Resuelve la carpeta padre de una ruta y el nombre del último segmento.
    async fn resolve_parent(&self, path: &DavPath, user_id: Uuid) -> Option<(Node, String)> {
        let name = path.file_name()?.to_string();
        if name.is_empty() {
            return None;
        }
        let parent = self.resolve(&path.parent(), user_id).await?;
        Some((parent, name))
    }
}

/// Traduce los errores del dominio a los de `dav-server`.
fn fs_error(err: &Error) -> FsError {
    match err {
        Error::NotFound(_) => FsError::NotFound,
        Error::Forbidden => FsError::Forbidden,
        Error::Conflict(_) => FsError::Exists,
        Error::Validation(_) => FsError::Forbidden,
        Error::QuotaExceeded => FsError::InsufficientStorage,
        Error::PayloadTooLarge { .. } => FsError::TooLarge,
        _ => FsError::GeneralFailure,
    }
}

/// Metadatos de un nodo, adaptados al trait de `dav-server`.
#[derive(Debug, Clone)]
struct NodeMeta {
    size: u64,
    is_dir: bool,
    modified: SystemTime,
    created: SystemTime,
}

impl NodeMeta {
    fn from_node(node: &Node) -> Self {
        Self {
            size: node.size_bytes.max(0) as u64,
            is_dir: node.kind.is_folder(),
            modified: DavFs::system_time(node.updated_at.timestamp()),
            created: DavFs::system_time(node.created_at.timestamp()),
        }
    }
}

impl DavMetaData for NodeMeta {
    fn len(&self) -> u64 {
        self.size
    }

    fn modified(&self) -> FsResult<SystemTime> {
        Ok(self.modified)
    }

    fn is_dir(&self) -> bool {
        self.is_dir
    }

    fn accessed(&self) -> FsResult<SystemTime> {
        Ok(self.modified)
    }

    fn created(&self) -> FsResult<SystemTime> {
        Ok(self.created)
    }
}

/// Implementación del sistema de archivos con control de acceso.
impl GuardedFileSystem<DavUser> for DavFs {
    fn open<'a>(
        &'a self,
        path: &'a DavPath,
        options: OpenOptions,
        credentials: &'a DavUser,
    ) -> FsFuture<'a, Box<dyn DavFile>> {
        let fs = self.clone();
        let path = path.clone();
        let user = credentials.clone();

        Box::pin(async move {
            if options.write {
                // Escritura: se prepara un blob temporal y el nodo se crea o
                // se reemplaza al vaciar el archivo.
                let (parent, name) = fs
                    .resolve_parent(&path, user.user_id)
                    .await
                    .ok_or(FsError::NotFound)?;
                if !parent.kind.is_folder() {
                    return Err(FsError::Forbidden);
                }

                let blob_key = BlobStore::new_key();
                let writer = fs
                    .blobs
                    .begin_write(&blob_key)
                    .await
                    .map_err(|e| fs_error(&e))?;

                // Si ya hay un archivo con ese nombre, se sustituye.
                let existing = fs
                    .nodes
                    .find_child(user.user_id, parent.id, &name)
                    .await
                    .ok()
                    .flatten();

                return Ok(Box::new(DavFileImpl::Write {
                    writer,
                    blobs: fs.blobs.clone(),
                    nodes: fs.nodes.clone(),
                    blob_key,
                    user_id: user.user_id,
                    parent_id: parent.id,
                    name,
                    existing_id: existing.map(|node| node.id),
                    written: 0,
                }) as Box<dyn DavFile>);
            }

            // Lectura.
            let node = fs
                .resolve(&path, user.user_id)
                .await
                .ok_or(FsError::NotFound)?;
            if node.kind.is_folder() {
                return Err(FsError::Forbidden);
            }
            let blob_key = node.blob_key.clone().ok_or(FsError::NotFound)?;
            let file = fs
                .blobs
                .open(&blob_key)
                .await
                .map_err(|_| FsError::NotFound)?;

            Ok(Box::new(DavFileImpl::Read {
                file,
                meta: NodeMeta::from_node(&node),
            }) as Box<dyn DavFile>)
        })
    }

    fn read_dir<'a>(
        &'a self,
        path: &'a DavPath,
        _meta: ReadDirMeta,
        credentials: &'a DavUser,
    ) -> FsFuture<'a, FsStream<Box<dyn DavDirEntry>>> {
        let fs = self.clone();
        let path = path.clone();
        let user = credentials.clone();

        Box::pin(async move {
            let node = fs
                .resolve(&path, user.user_id)
                .await
                .ok_or(FsError::NotFound)?;
            if !node.kind.is_folder() {
                return Err(FsError::Forbidden);
            }

            let children = fs
                .nodes
                .list_children(
                    user.user_id,
                    node.id,
                    &crate::models::node::NodeListQuery::default(),
                )
                .await
                .map_err(|e| fs_error(&e))?;

            let entries: Vec<Result<Box<dyn DavDirEntry>, FsError>> = children
                .into_iter()
                .map(|child| {
                    Ok(Box::new(NodeDirEntry {
                        name: child.name.into_bytes(),
                        size: child.size_bytes.max(0) as u64,
                        is_dir: child.kind == NodeKind::Folder,
                        modified: DavFs::system_time(child.updated_at.timestamp()),
                    }) as Box<dyn DavDirEntry>)
                })
                .collect();

            Ok(futures_util::stream::iter(entries).boxed())
        })
    }

    fn metadata<'a>(
        &'a self,
        path: &'a DavPath,
        credentials: &'a DavUser,
    ) -> FsFuture<'a, Box<dyn DavMetaData>> {
        let fs = self.clone();
        let path = path.clone();
        let user = credentials.clone();

        Box::pin(async move {
            let node = fs
                .resolve(&path, user.user_id)
                .await
                .ok_or(FsError::NotFound)?;
            Ok(Box::new(NodeMeta::from_node(&node)) as Box<dyn DavMetaData>)
        })
    }

    fn create_dir<'a>(&'a self, path: &'a DavPath, credentials: &'a DavUser) -> FsFuture<'a, ()> {
        let fs = self.clone();
        let path = path.clone();
        let user = credentials.clone();

        Box::pin(async move {
            let (parent, name) = fs
                .resolve_parent(&path, user.user_id)
                .await
                .ok_or(FsError::NotFound)?;
            fs.nodes
                .create_folder(user.user_id, parent.id, &name)
                .await
                .map_err(|e| fs_error(&e))?;
            Ok(())
        })
    }

    fn remove_file<'a>(&'a self, path: &'a DavPath, credentials: &'a DavUser) -> FsFuture<'a, ()> {
        let fs = self.clone();
        let path = path.clone();
        let user = credentials.clone();

        Box::pin(async move {
            let node = fs
                .resolve(&path, user.user_id)
                .await
                .ok_or(FsError::NotFound)?;
            if node.kind.is_folder() {
                return Err(FsError::Forbidden);
            }
            fs.nodes
                .trash(user.user_id, node.id)
                .await
                .map_err(|e| fs_error(&e))?;
            Ok(())
        })
    }

    fn remove_dir<'a>(&'a self, path: &'a DavPath, credentials: &'a DavUser) -> FsFuture<'a, ()> {
        let fs = self.clone();
        let path = path.clone();
        let user = credentials.clone();

        Box::pin(async move {
            let node = fs
                .resolve(&path, user.user_id)
                .await
                .ok_or(FsError::NotFound)?;
            if !node.kind.is_folder() {
                return Err(FsError::Forbidden);
            }
            fs.nodes
                .trash(user.user_id, node.id)
                .await
                .map_err(|e| fs_error(&e))?;
            Ok(())
        })
    }

    fn rename<'a>(
        &'a self,
        from: &'a DavPath,
        to: &'a DavPath,
        credentials: &'a DavUser,
    ) -> FsFuture<'a, ()> {
        let fs = self.clone();
        let from = from.clone();
        let to = to.clone();
        let user = credentials.clone();

        Box::pin(async move {
            let node = fs
                .resolve(&from, user.user_id)
                .await
                .ok_or(FsError::NotFound)?;
            let (parent, name) = fs
                .resolve_parent(&to, user.user_id)
                .await
                .ok_or(FsError::NotFound)?;

            // Si cambia la carpeta padre, primero se mueve el nodo.
            if node.parent_id != Some(parent.id) {
                fs.nodes
                    .move_to(user.user_id, node.id, parent.id)
                    .await
                    .map_err(|e| fs_error(&e))?;
            }
            fs.nodes
                .rename(user.user_id, node.id, &name)
                .await
                .map_err(|e| fs_error(&e))?;
            Ok(())
        })
    }

    fn copy<'a>(
        &'a self,
        from: &'a DavPath,
        to: &'a DavPath,
        credentials: &'a DavUser,
    ) -> FsFuture<'a, ()> {
        let fs = self.clone();
        let from = from.clone();
        let to = to.clone();
        let user = credentials.clone();

        Box::pin(async move {
            let node = fs
                .resolve(&from, user.user_id)
                .await
                .ok_or(FsError::NotFound)?;
            let (parent, name) = fs
                .resolve_parent(&to, user.user_id)
                .await
                .ok_or(FsError::NotFound)?;

            if node.kind.is_folder() {
                // Copia de carpetas: se crea la carpeta destino. El contenido
                // no se copia recursivamente.
                fs.nodes
                    .create_folder(user.user_id, parent.id, &name)
                    .await
                    .map_err(|e| fs_error(&e))?;
                return Ok(());
            }

            let source_key = node.blob_key.clone().ok_or(FsError::NotFound)?;
            let data = fs
                .blobs
                .read_to_vec(&source_key)
                .await
                .map_err(|_| FsError::NotFound)?;

            let new_key = BlobStore::new_key();
            fs.blobs
                .put(&new_key, &data)
                .await
                .map_err(|e| fs_error(&e))?;

            let checksum = fs.blobs.checksum(&new_key).await.unwrap_or_default();

            fs.nodes
                .create_file(
                    user.user_id,
                    NewFile {
                        parent_id: parent.id,
                        name,
                        mime_type: node.mime_type.clone(),
                        size_bytes: data.len() as i64,
                        blob_key: new_key,
                        checksum_sha256: checksum,
                    },
                )
                .await
                .map_err(|e| fs_error(&e))?;
            Ok(())
        })
    }

    fn get_quota<'a>(&'a self, credentials: &'a DavUser) -> FsFuture<'a, (u64, Option<u64>)> {
        let fs = self.clone();
        let user = credentials.clone();

        Box::pin(async move {
            let used = fs
                .nodes
                .compute_used_bytes(user.user_id)
                .await
                .unwrap_or(0)
                .max(0) as u64;

            let quota = sqlx::query_scalar::<_, i64>("SELECT quota_bytes FROM users WHERE id = $1")
                .bind(user.user_id)
                .fetch_optional(&fs.pool)
                .await
                .ok()
                .flatten()
                .unwrap_or(0)
                .max(0) as u64;

            if quota == 0 {
                Ok((used, None))
            } else {
                Ok((used, Some(quota)))
            }
        })
    }
}

/// Entrada de directorio.
struct NodeDirEntry {
    name: Vec<u8>,
    size: u64,
    is_dir: bool,
    modified: SystemTime,
}

impl DavDirEntry for NodeDirEntry {
    fn name(&self) -> Vec<u8> {
        self.name.clone()
    }

    fn metadata(&'_ self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        let meta = NodeMeta {
            size: self.size,
            is_dir: self.is_dir,
            modified: self.modified,
            created: self.modified,
        };
        Box::pin(async move { Ok(Box::new(meta) as Box<dyn DavMetaData>) })
    }

    fn is_dir(&'_ self) -> FsFuture<'_, bool> {
        Box::pin(async move { Ok(self.is_dir) })
    }
}

/// Archivo abierto en lectura o escritura.
enum DavFileImpl {
    Read {
        file: tokio::fs::File,
        meta: NodeMeta,
    },
    Write {
        writer: BlobWriter,
        blobs: BlobStore,
        nodes: NodeRepository,
        blob_key: String,
        user_id: Uuid,
        parent_id: Uuid,
        name: String,
        /// Nodo al que se sustituye el contenido, si ya existía.
        existing_id: Option<Uuid>,
        written: u64,
    },
}

impl std::fmt::Debug for DavFileImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DavFileImpl::Read { meta, .. } => f.debug_struct("Read").field("meta", meta).finish(),
            DavFileImpl::Write {
                blob_key,
                name,
                written,
                ..
            } => f
                .debug_struct("Write")
                .field("blob_key", blob_key)
                .field("name", name)
                .field("written", written)
                .finish(),
        }
    }
}

impl DavFile for DavFileImpl {
    fn metadata(&'_ mut self) -> FsFuture<'_, Box<dyn DavMetaData>> {
        let meta = match self {
            DavFileImpl::Read { meta, .. } => meta.clone(),
            DavFileImpl::Write { written, .. } => NodeMeta {
                size: *written,
                is_dir: false,
                modified: SystemTime::now(),
                created: SystemTime::now(),
            },
        };
        Box::pin(async move { Ok(Box::new(meta) as Box<dyn DavMetaData>) })
    }

    fn write_buf(&'_ mut self, mut buf: Box<dyn bytes::Buf + Send>) -> FsFuture<'_, ()> {
        // `Buf::to_bytes` ya no existe en `bytes` 1.12: se copia el contenido
        // fragmento a fragmento.
        let mut data = Vec::with_capacity(buf.remaining());
        while buf.has_remaining() {
            let chunk = buf.chunk();
            data.extend_from_slice(chunk);
            buf.advance(chunk.len());
        }
        self.write_bytes(Bytes::from(data))
    }

    fn write_bytes(&'_ mut self, buf: Bytes) -> FsFuture<'_, ()> {
        Box::pin(async move {
            let DavFileImpl::Write {
                writer, written, ..
            } = self
            else {
                return Err(FsError::Forbidden);
            };

            let new_size = *written + buf.len() as u64;
            if new_size > MAX_WRITE_SIZE {
                return Err(FsError::TooLarge);
            }

            writer
                .write_all(&buf)
                .await
                .map_err(|_| FsError::GeneralFailure)?;
            *written = new_size;
            Ok(())
        })
    }

    fn read_bytes(&'_ mut self, count: usize) -> FsFuture<'_, Bytes> {
        Box::pin(async move {
            let DavFileImpl::Read { file, .. } = self else {
                return Err(FsError::Forbidden);
            };
            let mut buf = vec![0u8; count];
            let read = file
                .read(&mut buf)
                .await
                .map_err(|_| FsError::GeneralFailure)?;
            buf.truncate(read);
            Ok(Bytes::from(buf))
        })
    }

    fn seek(&'_ mut self, pos: SeekFrom) -> FsFuture<'_, u64> {
        Box::pin(async move {
            let DavFileImpl::Read { file, .. } = self else {
                return Err(FsError::Forbidden);
            };
            file.seek(pos).await.map_err(|_| FsError::GeneralFailure)
        })
    }

    fn flush(&'_ mut self) -> FsFuture<'_, ()> {
        Box::pin(async move {
            // Al vaciar un archivo en escritura se confirma el blob y se crea
            // o se reemplaza el nodo en el árbol.
            let DavFileImpl::Write {
                writer,
                blobs,
                nodes,
                blob_key,
                user_id,
                parent_id,
                name,
                existing_id,
                written,
            } = self
            else {
                return Ok(());
            };

            let size = *written;
            writer.commit().await.map_err(|_| FsError::GeneralFailure)?;

            let checksum = blobs.checksum(blob_key).await.unwrap_or_default();

            if let Some(existing) = existing_id {
                nodes
                    .replace_file_content(*existing, blob_key, size as i64, &checksum)
                    .await
                    .map_err(|e| fs_error(&e))?;
            } else {
                nodes
                    .create_file(
                        *user_id,
                        NewFile {
                            parent_id: *parent_id,
                            name: name.clone(),
                            mime_type: guess_mime(name),
                            size_bytes: size as i64,
                            blob_key: blob_key.clone(),
                            checksum_sha256: checksum,
                        },
                    )
                    .await
                    .map_err(|e| fs_error(&e))?;
            }

            Ok(())
        })
    }
}

/// Deduce el tipo MIME a partir del nombre del archivo.
fn guess_mime(name: &str) -> Option<String> {
    mime_guess::from_path(name)
        .first_raw()
        .map(|mime| mime.to_string())
}
