//! Endpoint de subida resumible: protocolo tus 1.0.
//!
//! # Por qué un adaptador propio
//!
//! `fileloft-axum` ofrece un router listo, pero no permite autenticar la
//! petición ni asociar el upload a un usuario: los hooks (`pre_create`,
//! `pre_finish`) reciben únicamente el `UploadInfo`, sin las cabeceras HTTP.
//!
//! Por eso OpenDrive usa su propio adaptador, basado en el de `fileloft-axum`:
//!
//! 1. Se valida la sesión **antes** de tocar el handler de tus.
//! 2. El usuario autenticado se expone a los hooks mediante un
//!    `tokio::task_local!`. Los hooks se esperan en línea (no en un `spawn`),
//!    así que heredan el contexto de la petición.
//! 3. El identificador del upload lleva el prefijo del usuario, de modo que
//!    la URL que devuelve el servidor no es adivinable y un usuario no puede
//!    continuar la subida de otro.
//! 4. Al terminar el último `PATCH`, el adaptador mueve el archivo de staging
//!    a su blob definitivo e inserta el nodo en el árbol.

use std::io;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, post};
use fileloft_core::config::Config as TusConfig;
use fileloft_core::error::TusError;
use fileloft_core::handler::{TusBody, TusHandler, TusRequest, TusResponse};
use fileloft_core::hooks::{HookConfig, PreCreateCallback};
use fileloft_core::info::{UploadId, UploadInfo, UploadInfoChanges};
use fileloft_store_fs::{FileLocker, FileStore};
use futures_util::TryStreamExt;
use opendrive_core::db::nodes::NewFile;
use opendrive_core::storage::BlobStore;
use tokio::io::AsyncRead;
use tokio_util::io::{ReaderStream, StreamReader};
use uuid::Uuid;

use crate::extract::AuthUser;
use crate::state::AppState;

// Contexto de la petición actual, visible para los hooks de tus.
//
// Se define con `task_local!` porque los hooks se llaman en línea dentro del
// handler y no aceptan contexto de forma explícita.
tokio::task_local! {
    static UPLOAD_USER: Uuid;
}

/// Clave de metadatos tus con el nombre del archivo.
const META_FILENAME: &str = "filename";
/// Clave de metadatos tus con la carpeta de destino.
const META_PARENT: &str = "parent_id";
/// Clave de metadatos tus con el tipo MIME.
const META_MIME: &str = "filetype";

/// Ruta donde se monta el endpoint de subida. Debe coincidir con la del
/// router: se usa para construir la cabecera `Location`.
const UPLOAD_BASE_PATH: &str = "/api/uploads/";

/// Router del endpoint de subida.
pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/api/uploads", post(create_upload))
        .route("/api/uploads/{id}", any(upload_request))
}

/// Crea el handler de tus con sus hooks.
///
/// Se construye una vez y se comparte entre peticiones.
pub fn build_handler(state: &AppState) -> Arc<TusHandler<FileStore, FileLocker>> {
    let store = FileStore::new(state.blobs.staging_path().to_path_buf());
    let locker = FileLocker::new(state.blobs.staging_path().join("locks"));

    let mut config = TusConfig {
        // `base_path` es la ruta de montaje del endpoint.
        // Se deja `base_url: None` para que la cabecera `Location` devuelva una ruta
        // relativa (/api/uploads/<id>). Esto permite que las subidas funcionen a través de
        // cualquier proxy inverso (Vite, Nginx, Caddy, IP local o dominio) sin problemas de CORS.
        base_path: UPLOAD_BASE_PATH.to_string(),
        base_url: None,
        max_size: state.config.max_upload_size,
        trust_forwarded_headers: true,
        enable_download: false,
        ..TusConfig::default()
    };

    // Extensión de expiración: los uploads abandonados se limpian solos.
    config.extensions.expiration = true;
    config.extensions.expiration_ttl = Some(state.config.tus_expiry());
    // La extensión checksum permite al cliente verificar la integridad.
    config.extensions.checksum = true;

    config.hooks = HookConfig {
        channel_capacity: 64,
        pre_create: Some(pre_create_hook(state.clone())),
        ..HookConfig::default()
    };

    Arc::new(TusHandler::new(store, Some(locker), config))
}

/// Hook `pre_create`: valida la subida y la registra.
///
/// Se ejecuta al crear el upload (POST). Comprueba que la carpeta de destino
/// existe y pertenece al usuario, que hay cuota disponible y asigna un
/// identificador con el prefijo del usuario.
fn pre_create_hook(state: AppState) -> PreCreateCallback {
    Arc::new(move |info: UploadInfo| {
        let state = state.clone();
        Box::pin(async move {
            let user_id = UPLOAD_USER
                .try_with(|u| *u)
                .map_err(|_| TusError::HookRejected("petición no autenticada".into()))?;

            let filename = metadata_value(&info, META_FILENAME)
                .ok_or_else(|| TusError::InvalidMetadata("falta el nombre del archivo".into()))?;

            // Si se especifica carpeta padre se usa esa; si viene vacía o nula,
            // se sube por defecto a la carpeta raíz ("Mi Drive") del usuario.
            let parent_id = match metadata_value(&info, META_PARENT).and_then(|v| Uuid::parse_str(&v).ok()) {
                Some(id) => id,
                None => {
                    let root = state
                        .nodes
                        .get_root(user_id)
                        .await
                        .map_err(|e| TusError::HookRejected(e.to_string()))?
                        .ok_or_else(|| {
                            TusError::HookRejected("no se encontró la carpeta raíz del usuario".into())
                        })?;
                    root.id
                }
            };
            let mime = metadata_value(&info, META_MIME);

            // La carpeta de destino debe pertenecer al usuario.
            let parent = state
                .nodes
                .get(user_id, parent_id)
                .await
                .map_err(|e| TusError::HookRejected(e.to_string()))?
                .ok_or_else(|| TusError::HookRejected("la carpeta de destino no existe".into()))?;

            if !parent.kind.is_folder() {
                return Err(TusError::HookRejected(
                    "la carpeta de destino no es una carpeta".into(),
                ));
            }
            if parent.trashed_at.is_some() {
                return Err(TusError::HookRejected(
                    "la carpeta de destino está en la papelera".into(),
                ));
            }

            let size = info.size.unwrap_or(0);
            let size_i64 = i64::try_from(size).map_err(|_| TusError::EntityTooLarge {
                max: i64::MAX as u64,
            })?;

            // Límite por archivo.
            let max = state.config.max_upload_size;
            if max > 0 && size > max {
                return Err(TusError::EntityTooLarge { max });
            }

            // Cuota del usuario.
            let used = state
                .nodes
                .compute_used_bytes(user_id)
                .await
                .map_err(|e| TusError::HookRejected(e.to_string()))?;
            let quota = state
                .users
                .find_by_id(user_id)
                .await
                .map_err(|e| TusError::HookRejected(e.to_string()))?
                .map(|u| u.quota_bytes)
                .unwrap_or(0);
            if quota > 0 && used + size_i64 > quota {
                return Err(TusError::HookRejected(
                    "la cuota de almacenamiento se ha excedido".into(),
                ));
            }

            // Identificador con el prefijo del usuario: la URL que se devuelve
            // no es adivinable y no se puede reutilizar entre usuarios.
            let upload_id =
                UploadId::from(format!("{}_{}", user_id.simple(), Uuid::new_v4().simple()));

            // Se registra el upload para poder completar la subida más tarde.
            state
                .register_upload(
                    upload_id.as_str(),
                    user_id,
                    parent_id,
                    &filename,
                    mime.as_deref(),
                    size_i64,
                )
                .await
                .map_err(|e| TusError::HookRejected(e.to_string()))?;

            Ok(UploadInfoChanges {
                id: Some(upload_id),
                metadata: None,
                storage: None,
            })
        })
    })
}

/// Extrae un valor de los metadatos de la petición tus.
fn metadata_value(info: &UploadInfo, key: &str) -> Option<String> {
    info.metadata
        .0
        .get(key)
        .and_then(|v| v.clone())
        .filter(|v| !v.is_empty())
}

/// Crea un upload nuevo (POST).
async fn create_upload(State(state): State<AppState>, user: AuthUser, req: Request) -> Response {
    dispatch(state, user, None, req).await
}

/// Gestiona una petición sobre un upload existente (PATCH/HEAD/DELETE).
async fn upload_request(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<String>,
    req: Request,
) -> Response {
    dispatch(state, user, Some(id), req).await
}

/// Núcleo del endpoint: autentica, delega en el handler de tus y completa la
/// subida cuando el último fragmento ha llegado.
async fn dispatch(
    state: AppState,
    user: AuthUser,
    upload_id: Option<String>,
    req: Request,
) -> Response {
    let user_id = user.id();

    // Un upload pertenece a su creador: se comprueba antes de tocar nada.
    if let Some(id) = &upload_id
        && !state.upload_belongs_to(id, user_id).await
    {
        return tus_error_response(
            StatusCode::NOT_FOUND,
            &format!("no se encontró la subida {id}"),
        );
    }

    let (parts, body) = req.into_parts();
    let method = parts.method;
    let uri = parts.uri;
    let headers = parts.headers;

    // El cuerpo solo se transmite en los métodos que lo necesitan.
    let body = if matches!(
        method,
        Method::HEAD | Method::DELETE | Method::OPTIONS | Method::GET
    ) {
        None
    } else {
        let stream =
            TryStreamExt::map_err(body.into_data_stream(), |e| io::Error::other(e.to_string()));
        let reader: Box<dyn AsyncRead + Send + Unpin> = Box::new(StreamReader::new(stream));
        Some(reader)
    };

    let tus_request = TusRequest {
        method: method.clone(),
        uri,
        upload_id: upload_id.clone(),
        headers,
        body,
    };

    // El handler se ejecuta dentro del contexto de la petición, de modo que
    // los hooks ven el usuario autenticado.
    let handler = state.tus_handler.clone();
    let response = UPLOAD_USER
        .scope(user_id, async move { handler.handle(tus_request).await })
        .await;

    // Si el último fragmento ha llegado, el archivo ya está ensamblado en el
    // área de staging: se mueve a su blob definitivo y se inserta el nodo.
    if method == Method::PATCH
        && response.status == StatusCode::NO_CONTENT
        && let Some(id) = &upload_id
        && let Err(err) = complete_upload(&state, id, user_id).await
    {
        tracing::error!(error = %err, upload_id = %id, "no se pudo completar la subida");
    }

    map_tus_response(response)
}

/// Convierte la respuesta del handler de tus en una respuesta de axum.
fn map_tus_response(response: TusResponse) -> Response {
    let body = match response.body {
        TusBody::Bytes(bytes) => Body::from(bytes),
        TusBody::Reader(reader) => Body::from_stream(ReaderStream::new(reader)),
    };

    let mut builder = Response::builder().status(response.status);
    for (name, value) in response.headers.iter() {
        if name.as_str().eq_ignore_ascii_case("location") {
            let loc_str = value.to_str().unwrap_or_default();
            // Si la cabecera Location viene como URL absoluta (ej: http://localhost:8080/api/uploads/xyz),
            // la convertimos a ruta relativa (/api/uploads/xyz).
            // De este modo, el navegador siempre envía los PATCH al mismo host y puerto
            // desde donde se cargó la aplicación web o proxy.
            if let Some(pos) = loc_str.find("/api/uploads/") {
                let relative_loc = &loc_str[pos..];
                if let Ok(new_val) = axum::http::HeaderValue::from_str(relative_loc) {
                    builder = builder.header(name, new_val);
                    continue;
                }
            }
        }
        builder = builder.header(name, value);
    }

    // Cabeceras CORS indispensables para que el cliente tus (Uppy) en el navegador
    // pueda leer Location y Upload-Offset sin ser bloqueado.
    builder = builder.header(
        "Access-Control-Expose-Headers",
        "Location, Upload-Offset, Upload-Length, Tus-Version, Tus-Resumable, Tus-Max-Size, Tus-Extension",
    );

    builder.body(body).unwrap_or_else(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .expect("respuesta vacía válida")
    })
}

/// Construye una respuesta de error con el formato de la API.
fn tus_error_response(status: StatusCode, message: &str) -> Response {
    (
        status,
        axum::Json(serde_json::json!({
            "error": { "code": "upload_error", "message": message }
        })),
    )
        .into_response()
}

/// Finaliza un upload: mueve el archivo a su blob e inserta el nodo.
///
/// Es idempotente: si el upload ya estaba completado, no hace nada.
async fn complete_upload(
    state: &AppState,
    upload_id: &str,
    user_id: Uuid,
) -> opendrive_core::Result<()> {
    let Some(upload) = state.find_upload(upload_id).await? else {
        return Ok(());
    };

    // El upload debe pertenecer al usuario que lo completa.
    if upload.user_id != user_id {
        return Err(opendrive_core::Error::Forbidden);
    }

    // El archivo ensamblado solo existe cuando la subida está completa.
    let Some(size) = state.blobs.staging_size(upload_id).await? else {
        return Ok(());
    };

    if size as i64 != upload.size_bytes {
        // La subida aún no ha terminado: los fragmentos siguen en staging.
        return Ok(());
    }

    let checksum = state.blobs.staging_checksum(upload_id).await?;
    let blob_key = BlobStore::new_key();

    state.blobs.promote_staging(upload_id, &blob_key).await?;

    let parent_id = upload.parent_id.ok_or_else(|| {
        opendrive_core::Error::Internal("el upload no tiene carpeta de destino".into())
    })?;

    let node = state
        .nodes
        .create_file(
            user_id,
            NewFile {
                parent_id,
                name: upload.filename.clone(),
                mime_type: upload.mime_type.clone(),
                size_bytes: size as i64,
                blob_key: blob_key.clone(),
                checksum_sha256: checksum,
            },
        )
        .await?;

    state.users.add_used_bytes(user_id, size as i64).await?;

    // Si el formato es indexable, se extrae el texto para la búsqueda por
    // contenido. Un fallo aquí no invalida la subida.
    if let Some(text) = indexable_text(state, &node).await {
        state.nodes.set_content(node.id, &text).await?;
        tracing::debug!(node_id = %node.id, "texto indexado para la búsqueda");
    }

    state.delete_upload(upload_id).await?;
    state.blobs.remove_staging(upload_id).await?;

    tracing::info!(
        upload_id,
        node_id = %node.id,
        size,
        "subida completada"
    );

    Ok(())
}

/// Extrae el texto indexable de un nodo, si su formato lo permite.
async fn indexable_text(
    state: &AppState,
    node: &opendrive_core::models::node::Node,
) -> Option<String> {
    let blob_key = node.blob_key.as_ref()?;
    let data = state.blobs.read_to_vec(blob_key).await.ok()?;
    let mime = node.mime_type.as_deref();

    match state.text.extract(&node.name, mime, &data).await {
        Ok(Some(text)) => Some(text),
        Ok(None) => None,
        Err(err) => {
            tracing::warn!(error = %err, "no se pudo extraer el texto del archivo");
            None
        }
    }
}
