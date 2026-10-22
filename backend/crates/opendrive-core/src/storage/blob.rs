//! Almacén de blobs en el sistema de archivos.
//!
//! Los blobs se guardan en un árbol balanceado de dos niveles
//! (`blobs/ab/cd/<uuid>`) para no meter miles de entradas en un mismo
//! directorio. El nombre del archivo en disco es una clave opaca generada por
//! el servidor: **nunca** deriva del nombre visible del archivo, de modo que
//! el usuario no puede influir en la ruta del sistema de archivos.
//!
//! El área de staging de las subidas en curso (`.staging/`) vive **dentro** de
//! la misma raíz que los blobs. Así, el paso de un upload terminado a blob
//! definitivo es un `rename` dentro de la misma raíz cap-std: atómico, sin
//! copiar los bytes y sin salir nunca del directorio confinado.
//!
//! Todo el acceso pasa por [`cap_std::fs::Dir`], que confina las operaciones al
//! directorio raíz y hace imposible escapar de él con `..` o rutas absolutas.

use std::path::PathBuf;
use std::sync::Arc;

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::error::{Error, Result};

/// Nombre del subdirectorio de staging dentro de la raíz de blobs.
///
/// Lleva un punto inicial para que nunca pueda coincidir con una clave de
/// blob, que solo admite 32 caracteres hexadecimales.
pub const STAGING_DIR: &str = ".staging";

/// Almacén de contenido en disco confinado a un directorio raíz.
#[derive(Debug, Clone)]
pub struct BlobStore {
    root: Arc<Dir>,
    staging: Arc<Dir>,
    root_path: PathBuf,
    staging_path: PathBuf,
}

impl BlobStore {
    /// Abre (o crea) el almacén en la ruta indicada.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root_path = root.into();
        std::fs::create_dir_all(&root_path)?;

        let staging_path = root_path.join(STAGING_DIR);
        std::fs::create_dir_all(&staging_path)?;

        let root = Dir::open_ambient_dir(&root_path, ambient_authority()).map_err(|e| {
            Error::Internal(format!("no se pudo abrir el directorio de blobs: {e}"))
        })?;
        let staging = root.open_dir(STAGING_DIR).map_err(|e| {
            Error::Internal(format!("no se pudo abrir el directorio de staging: {e}"))
        })?;

        Ok(Self {
            root: Arc::new(root),
            staging: Arc::new(staging),
            root_path,
            staging_path,
        })
    }

    /// Ruta absoluta del directorio raíz.
    pub fn root_path(&self) -> &PathBuf {
        &self.root_path
    }

    /// Ruta absoluta del directorio de staging.
    ///
    /// La usa `fileloft` como almacén de subidas en curso, que necesita una
    /// ruta real del sistema de archivos y no puede operar con `cap-std`.
    pub fn staging_path(&self) -> &PathBuf {
        &self.staging_path
    }

    /// Genera una clave opaca para un blob nuevo.
    pub fn new_key() -> String {
        Uuid::new_v4().simple().to_string()
    }

    /// Convierte una clave en la ruta relativa dentro del almacén.
    ///
    /// La clave se valida estrictamente: solo se admiten 32 caracteres
    /// hexadecimales. Cualquier otra cosa se rechaza, de modo que la clave
    /// nunca pueda contener separadores de ruta.
    fn relative_path(key: &str) -> Result<String> {
        if key.len() != 32 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::Validation("clave de blob no válida".into()));
        }
        Ok(format!("{}/{}/{}", &key[0..2], &key[2..4], key))
    }

    /// Divide una ruta relativa en sus dos primeros segmentos y el nombre.
    fn split_relative(rel: &str) -> Result<(&str, &str, &str)> {
        let (dir_part, file_name) = rel
            .rsplit_once('/')
            .ok_or_else(|| Error::Validation("clave de blob no válida".into()))?;
        let (a, b) = dir_part
            .split_once('/')
            .ok_or_else(|| Error::Validation("clave de blob no válida".into()))?;
        Ok((a, b, file_name))
    }

    /// Abre (o crea) el directorio de dos niveles que aloja un blob.
    fn parent_dir(&self, key: &str) -> Result<Dir> {
        let rel = Self::relative_path(key)?;
        let (a, b, _) = Self::split_relative(&rel)?;
        self.root.create_dir_all(format!("{a}/{b}")).map_err(|e| {
            Error::Internal(format!("no se pudo preparar el directorio del blob: {e}"))
        })?;
        let dir = self
            .root
            .open_dir(a)
            .and_then(|d| d.open_dir(b))
            .map_err(|e| {
                Error::Internal(format!("no se pudo abrir el directorio del blob: {e}"))
            })?;
        Ok(dir)
    }

    /// Abre un blob para lectura.
    pub async fn open(&self, key: &str) -> Result<tokio::fs::File> {
        let rel = Self::relative_path(key)?;
        let (a, b, file_name) = Self::split_relative(&rel)?;

        let dir = self
            .root
            .open_dir(a)
            .and_then(|d| d.open_dir(b))
            .map_err(|_| Error::NotFound("archivo".into()))?;

        let file = dir
            .open(file_name)
            .map_err(|_| Error::NotFound("archivo".into()))?;

        Ok(tokio::fs::File::from_std(file.into_std()))
    }

    /// Inicia la escritura de un blob nuevo.
    ///
    /// El contenido se escribe primero en un archivo temporal dentro del
    /// mismo directorio y se renombra al final, de modo que un fallo a mitad
    /// de la subida no deja un blob corrupto visible.
    pub async fn begin_write(&self, key: &str) -> Result<BlobWriter> {
        let dir = self.parent_dir(key)?;
        let temp_name = format!(".tmp-{}", Uuid::new_v4().simple());
        let file = dir
            .create(&temp_name)
            .map_err(|e| Error::Internal(format!("no se pudo crear el archivo temporal: {e}")))?;
        Ok(BlobWriter {
            file: tokio::fs::File::from_std(file.into_std()),
            dir,
            temp_name,
            key: key.to_string(),
        })
    }

    /// Escribe un blob completo de una vez.
    pub async fn put(&self, key: &str, data: &[u8]) -> Result<()> {
        let mut writer = self.begin_write(key).await?;
        writer.write_all(data).await?;
        writer.commit().await?;
        Ok(())
    }

    /// Elimina un blob. No falla si no existe.
    pub async fn delete(&self, key: &str) -> Result<()> {
        let Ok(rel) = Self::relative_path(key) else {
            return Ok(());
        };
        let (a, b, file_name) = Self::split_relative(&rel)?;

        let Ok(dir) = self.root.open_dir(a).and_then(|d| d.open_dir(b)) else {
            return Ok(());
        };
        let _ = dir.remove_file(file_name);
        Ok(())
    }

    /// Devuelve el tamaño en bytes de un blob.
    pub async fn size(&self, key: &str) -> Result<u64> {
        let file = self.open(key).await?;
        let meta = file.metadata().await?;
        Ok(meta.len())
    }

    /// Lee un blob completo en memoria.
    ///
    /// Solo para archivos pequeños (comprobaciones, miniaturas). Para
    /// descargas se usa [`BlobStore::open`] con streaming.
    pub async fn read_to_vec(&self, key: &str) -> Result<Vec<u8>> {
        let mut file = self.open(key).await?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;
        Ok(buf)
    }

    /// Calcula el SHA-256 de un blob.
    pub async fn checksum(&self, key: &str) -> Result<String> {
        use sha2::{Digest, Sha256};

        let mut file = self.open(key).await?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    }

    /* ---------------------------------------------------------------------
     * Área de staging
     * ------------------------------------------------------------------ */

    /// Devuelve el tamaño del archivo de staging indicado, si existe.
    pub async fn staging_size(&self, name: &str) -> Result<Option<u64>> {
        if !is_safe_staging_name(name) {
            return Err(Error::Validation("nombre de staging no válido".into()));
        }
        let Ok(file) = self.staging.open(name) else {
            return Ok(None);
        };
        let meta = file.metadata()?;
        Ok(Some(meta.len()))
    }

    /// Calcula el SHA-256 de un archivo de staging.
    pub async fn staging_checksum(&self, name: &str) -> Result<String> {
        use sha2::{Digest, Sha256};

        if !is_safe_staging_name(name) {
            return Err(Error::Validation("nombre de staging no válido".into()));
        }
        let file = self
            .staging
            .open(name)
            .map_err(|_| Error::NotFound("upload".into()))?;
        let mut file = tokio::fs::File::from_std(file.into_std());

        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(hex::encode(hasher.finalize()))
    }

    /// Mueve un archivo de staging a su posición definitiva como blob.
    ///
    /// Es un `rename` dentro de la misma raíz cap-std: atómico y sin copiar
    /// los bytes. Devuelve `false` si el archivo de staging no existe.
    pub async fn promote_staging(&self, name: &str, key: &str) -> Result<bool> {
        if !is_safe_staging_name(name) {
            return Err(Error::Validation("nombre de staging no válido".into()));
        }
        let rel = Self::relative_path(key)?;
        let (a, b, _) = Self::split_relative(&rel)?;

        self.root.create_dir_all(format!("{a}/{b}")).map_err(|e| {
            Error::Internal(format!("no se pudo preparar el directorio del blob: {e}"))
        })?;

        if !self.staging.try_exists(name).unwrap_or(false) {
            return Ok(false);
        }

        // En `cap_std`, el origen es relativo a `self` y el destino es
        // relativo a `to_dir`: aquí ambos son la raíz de los blobs.
        self.root
            .rename(format!("{STAGING_DIR}/{name}"), &self.root, &rel)
            .map_err(|e| Error::Internal(format!("no se pudo mover el blob a su destino: {e}")))?;
        Ok(true)
    }

    /// Elimina un archivo de staging y sus residuos. No falla si no existe.
    pub async fn remove_staging(&self, name: &str) -> Result<()> {
        if !is_safe_staging_name(name) {
            return Ok(());
        }
        // `fileloft` deja el archivo de datos, el de metadatos (`.info`) y los
        // fragmentos parciales (`_part_N`).
        let _ = self.staging.remove_file(name);
        let _ = self.staging.remove_file(format!("{name}.info"));
        Ok(())
    }
}

/// Valida un nombre de archivo dentro del área de staging.
///
/// Solo se admiten los caracteres que `fileloft` usa en sus identificadores:
/// alfanuméricos, `-`, `_` y `.`. Cualquier separador de ruta se rechaza.
fn is_safe_staging_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !name.contains("..")
}

/// Escritor de un blob en curso.
///
/// El archivo temporal se renombra a su clave final al confirmar con
/// [`BlobWriter::commit`]. Si se abandona el escritor, el archivo temporal
/// queda huérfano y lo limpia la tarea de mantenimiento.
#[derive(Debug)]
pub struct BlobWriter {
    file: tokio::fs::File,
    dir: Dir,
    temp_name: String,
    key: String,
}

impl BlobWriter {
    /// Escribe contenido en el blob.
    pub async fn write_all(&mut self, data: &[u8]) -> Result<()> {
        self.file.write_all(data).await?;
        Ok(())
    }

    /// Escribe contenido desde un lector asíncrono, devolviendo los bytes escritos.
    pub async fn write_from<R>(&mut self, mut reader: R) -> Result<u64>
    where
        R: AsyncRead + Unpin,
    {
        let mut buf = vec![0u8; 64 * 1024];
        let mut total = 0u64;
        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            self.file.write_all(&buf[..n]).await?;
            total += n as u64;
        }
        Ok(total)
    }

    /// Sincroniza el contenido a disco.
    pub async fn sync(&mut self) -> Result<()> {
        self.file.flush().await?;
        self.file.sync_all().await?;
        Ok(())
    }

    /// Confirma la escritura: sincroniza y renombra el temporal a su clave final.
    ///
    /// Se toma por referencia para que quien escribe pueda seguir usando el
    /// escritor después de confirmar.
    pub async fn commit(&mut self) -> Result<()> {
        self.sync().await?;
        self.file.shutdown().await?;
        let rel = BlobStore::relative_path(&self.key)?;
        let (_, _, file_name) = BlobStore::split_relative(&rel)?;
        // `cap_std` exige el directorio de origen además de la ruta.
        self.dir
            .rename(&self.temp_name, &self.dir, file_name)
            .map_err(|e| Error::Internal(format!("no se pudo confirmar el blob: {e}")))?;
        Ok(())
    }

    /// Descarta la escritura y borra el archivo temporal.
    pub async fn abort(&self) {
        let _ = self.dir.remove_file(&self.temp_name);
    }
}
