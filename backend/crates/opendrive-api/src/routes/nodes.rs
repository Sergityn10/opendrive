//! Rutas de gestión de archivos.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header::{
    ACCEPT_RANGES, CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE,
    ETAG, IF_NONE_MATCH, RANGE,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use opendrive_core::error::{Error, Result};
use opendrive_core::models::node::{NodeListQuery, NodeSummary};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::extract::AuthUser;
use crate::state::AppState;

/// Router de archivos.
pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/nodes", get(list_nodes).post(create_folder))
        .route(
            "/nodes/{id}",
            get(get_node).patch(update_node).delete(trash_node),
        )
        .route("/nodes/{id}/restore", post(restore_node))
        .route("/nodes/{id}/download", get(download_node))
        .route("/nodes/{id}/thumbnail", get(thumbnail_node))
        .route("/nodes/{id}/breadcrumb", get(breadcrumb))
        .route("/trash", get(list_trash).delete(empty_trash))
        .route("/search", get(search))
}

/// Listado de una carpeta.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct NodeListResponse {
    /// Elementos de la carpeta.
    pub items: Vec<NodeSummary>,
    /// Cursor para la siguiente página, si la hay.
    pub next_cursor: Option<Uuid>,
}

/// Crea una carpeta.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateFolderRequest {
    /// Carpeta padre. Si se omite, se usa la raíz del usuario.
    pub parent_id: Option<Uuid>,
    /// Nombre de la carpeta.
    pub name: String,
}

/// Modifica un nodo.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateNodeRequest {
    /// Nuevo nombre.
    pub name: Option<String>,
    /// Nueva carpeta padre.
    pub parent_id: Option<Uuid>,
}

/// Lista el contenido de una carpeta.
#[utoipa::path(
    get,
    path = "/api/nodes",
    tag = "nodes",
    params(NodeListQuery),
    responses(
        (status = 200, description = "Contenido de la carpeta", body = NodeListResponse),
        (status = 401, description = "No autenticado"),
    )
)]
async fn list_nodes(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<NodeListQuery>,
) -> Result<Json<NodeListResponse>> {
    let parent_id = match query.parent_id {
        Some(id) => id,
        None => {
            let root = state
                .nodes
                .get_root(user.id())
                .await?
                .ok_or_else(|| Error::NotFound("carpeta raíz".into()))?;
            root.id
        }
    };

    // Verifica que la carpeta existe y pertenece al usuario.
    state
        .nodes
        .get(user.id(), parent_id)
        .await?
        .ok_or_else(|| Error::NotFound("carpeta".into()))?;

    let items = state
        .nodes
        .list_children(user.id(), parent_id, &query)
        .await?;
    let next_cursor = items.last().map(|n| n.id);

    Ok(Json(NodeListResponse { items, next_cursor }))
}

/// Devuelve un nodo concreto.
#[utoipa::path(
    get,
    path = "/api/nodes/{id}",
    tag = "nodes",
    responses(
        (status = 200, description = "Nodo", body = opendrive_core::models::node::NodeSummary),
        (status = 404, description = "No encontrado"),
    )
)]
async fn get_node(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<NodeSummary>> {
    let node = state
        .nodes
        .get(user.id(), id)
        .await?
        .ok_or_else(|| Error::NotFound("nodo".into()))?;
    Ok(Json(NodeSummary::from(node)))
}

/// Crea una carpeta.
#[utoipa::path(
    post,
    path = "/api/nodes",
    tag = "nodes",
    request_body = CreateFolderRequest,
    responses(
        (status = 200, description = "Carpeta creada", body = opendrive_core::models::node::NodeSummary),
        (status = 400, description = "Datos inválidos"),
        (status = 409, description = "Ya existe un elemento con ese nombre"),
    )
)]
async fn create_folder(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<CreateFolderRequest>,
) -> Result<Json<NodeSummary>> {
    let parent_id = match req.parent_id {
        Some(id) => id,
        None => {
            let root = state
                .nodes
                .get_root(user.id())
                .await?
                .ok_or_else(|| Error::NotFound("carpeta raíz".into()))?;
            root.id
        }
    };

    let node = state
        .nodes
        .create_folder(user.id(), parent_id, &req.name)
        .await?;
    Ok(Json(NodeSummary::from(node)))
}

/// Renombra o mueve un nodo.
#[utoipa::path(
    patch,
    path = "/api/nodes/{id}",
    tag = "nodes",
    request_body = UpdateNodeRequest,
    responses(
        (status = 200, description = "Nodo actualizado", body = opendrive_core::models::node::NodeSummary),
        (status = 400, description = "Operación no válida"),
    )
)]
async fn update_node(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateNodeRequest>,
) -> Result<Json<NodeSummary>> {
    let node = if let Some(name) = req.name {
        state.nodes.rename(user.id(), id, &name).await?
    } else {
        state
            .nodes
            .get(user.id(), id)
            .await?
            .ok_or_else(|| Error::NotFound("nodo".into()))?
    };

    let node = if let Some(parent_id) = req.parent_id {
        state.nodes.move_to(user.id(), node.id, parent_id).await?
    } else {
        node
    };

    Ok(Json(NodeSummary::from(node)))
}

/// Mueve un nodo a la papelera.
#[utoipa::path(
    delete,
    path = "/api/nodes/{id}",
    tag = "nodes",
    responses(
        (status = 200, description = "Nodo enviado a la papelera"),
        (status = 404, description = "No encontrado"),
    )
)]
async fn trash_node(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let trashed = state.nodes.trash(user.id(), id).await?;
    Ok(Json(serde_json::json!({ "trashed": trashed.len() })))
}

/// Restaura un nodo desde la papelera.
#[utoipa::path(
    post,
    path = "/api/nodes/{id}/restore",
    tag = "nodes",
    responses((status = 200, description = "Nodo restaurado"))
)]
async fn restore_node(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let restored = state.nodes.restore(user.id(), id).await?;
    Ok(Json(serde_json::json!({ "restored": restored.len() })))
}

/// Lista la papelera.
#[utoipa::path(
    get,
    path = "/api/trash",
    tag = "nodes",
    responses((status = 200, description = "Contenido de la papelera"))
)]
async fn list_trash(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<Vec<NodeSummary>>> {
    let items = state.nodes.list_trashed(user.id(), 500).await?;
    Ok(Json(items))
}

/// Vacía la papelera.
#[utoipa::path(
    delete,
    path = "/api/trash",
    tag = "nodes",
    responses((status = 200, description = "Papelera vaciada"))
)]
async fn empty_trash(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<serde_json::Value>> {
    let deleted = state.nodes.empty_trash(user.id()).await?;
    for node in &deleted {
        if let Some(key) = &node.blob_key
            && let Err(err) = state.blobs.delete(key).await
        {
            tracing::warn!(error = %err, key = %key, "no se pudo borrar el blob");
        }
    }
    Ok(Json(serde_json::json!({ "deleted": deleted.len() })))
}

/// Descarga el contenido de un archivo, con soporte de rangos.
#[utoipa::path(
    get,
    path = "/api/nodes/{id}/download",
    tag = "nodes",
    responses(
        (status = 200, description = "Contenido del archivo"),
        (status = 206, description = "Rango parcial del archivo"),
        (status = 404, description = "No encontrado"),
    )
)]
async fn download_node(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response> {
    let node = state
        .nodes
        .get(user.id(), id)
        .await?
        .ok_or_else(|| Error::NotFound("nodo".into()))?;

    let blob_key = node
        .blob_key
        .clone()
        .ok_or_else(|| Error::Validation("el nodo no es un archivo".into()))?;

    let mut file = state.blobs.open(&blob_key).await?;
    let total = file.metadata().await?.len();

    let etag = node
        .checksum_sha256
        .as_deref()
        .map(|h| format!("\"{h}\""))
        .unwrap_or_else(|| format!("\"{blob_key}-{total}\""));

    let mime = node
        .mime_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_string());

    // Nombre de archivo saneado para la cabecera Content-Disposition.
    let filename = node
        .name
        .chars()
        .filter(|c| !c.is_control() && *c != '"' && *c != '\\')
        .collect::<String>();
    let filename = if filename.is_empty() {
        "archivo".to_string()
    } else {
        filename
    };

    let mut response_headers = HeaderMap::new();
    response_headers.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response_headers.insert(ETAG, HeaderValue::from_str(&etag)?);
    response_headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&mime)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    response_headers.insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))?,
    );

    // Peticiones condicionales: 304 si el cliente ya tiene la versión actual.
    if let Some(if_none_match) = headers.get(IF_NONE_MATCH)
        && if_none_match.to_str().ok() == Some(etag.as_str())
    {
        return Ok((StatusCode::NOT_MODIFIED, response_headers).into_response());
    }

    // Rango parcial.
    let range_header = headers.get(RANGE).and_then(|v| v.to_str().ok());
    if let Some(range) = range_header {
        if let Some((start, end)) = parse_range(range, total) {
            let length = end - start + 1;
            file.seek(std::io::SeekFrom::Start(start)).await?;

            let stream = ReaderStream::new(file.take(length));
            response_headers.insert(CONTENT_LENGTH, HeaderValue::from(length));
            response_headers.insert(
                CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes {start}-{end}/{total}"))?,
            );

            return Ok((
                StatusCode::PARTIAL_CONTENT,
                response_headers,
                Body::from_stream(stream),
            )
                .into_response());
        }

        // Rango no satisfacible.
        response_headers.insert(
            CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes */{total}"))?,
        );
        return Ok((StatusCode::RANGE_NOT_SATISFIABLE, response_headers).into_response());
    }

    response_headers.insert(CONTENT_LENGTH, HeaderValue::from(total));
    let stream = ReaderStream::new(file);
    Ok((StatusCode::OK, response_headers, Body::from_stream(stream)).into_response())
}

/// Parámetros de la miniatura.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ThumbnailQuery {
    /// Lado del cuadrado de la miniatura, en píxeles.
    #[serde(default)]
    pub size: Option<u32>,
}

/// Devuelve la miniatura de un archivo de imagen.
///
/// La miniatura se genera bajo demanda y se cachea en disco, indexada por el
/// hash del contenido: si el archivo cambia, la miniatura se regenera sola.
#[utoipa::path(
    get,
    path = "/api/nodes/{id}/thumbnail",
    tag = "nodes",
    params(ThumbnailQuery),
    responses(
        (status = 200, description = "Miniatura en WebP"),
        (status = 404, description = "No encontrado o no es una imagen"),
    )
)]
async fn thumbnail_node(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Query(query): Query<ThumbnailQuery>,
) -> Result<Response> {
    let node = state
        .nodes
        .get(user.id(), id)
        .await?
        .ok_or_else(|| Error::NotFound("nodo".into()))?;

    let blob_key = node
        .blob_key
        .clone()
        .ok_or_else(|| Error::NotFound("miniatura".into()))?;

    let checksum = node
        .checksum_sha256
        .clone()
        .ok_or_else(|| Error::NotFound("miniatura".into()))?;

    let size = opendrive_core::storage::ThumbnailService::normalize_size(query.size);

    let path = if state.thumbnails.is_cached(&checksum, size) {
        state.thumbnails.cache_path(&checksum, size)
    } else {
        // Solo se leen los bytes originales si la miniatura no existe.
        let data = state.blobs.read_to_vec(&blob_key).await?;
        let (path, _) = state.thumbnails.ensure(&checksum, size, &data)?;
        path
    };

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| Error::NotFound("miniatura".into()))?;
    let length = file.metadata().await?.len();

    // Las imágenes con canal alfa se guardan en PNG; el resto, en JPEG.
    let content_type = if path.extension().is_some_and(|ext| ext == "png") {
        "image/png"
    } else {
        "image/jpeg"
    };

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(CONTENT_LENGTH, HeaderValue::from(length));
    // La miniatura depende del contenido, no del momento: se cachea para
    // siempre en el navegador.
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );

    let stream = ReaderStream::new(file);
    Ok((StatusCode::OK, headers, Body::from_stream(stream)).into_response())
}

/// Devuelve la ruta de ancestros de un nodo.
#[utoipa::path(
    get,
    path = "/api/nodes/{id}/breadcrumb",
    tag = "nodes",
    responses((status = 200, description = "Ruta de ancestros"))
)]
async fn breadcrumb(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<NodeSummary>>> {
    let ancestors = state.nodes.ancestors(user.id(), id).await?;
    Ok(Json(ancestors.into_iter().map(NodeSummary::from).collect()))
}

/// Parámetros de búsqueda.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SearchQuery {
    /// Texto a buscar.
    pub q: String,
    /// Buscar también en el contenido de los documentos.
    #[serde(default)]
    pub content: bool,
    /// Número máximo de resultados.
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

/// Busca archivos por nombre y contenido.
#[utoipa::path(
    get,
    path = "/api/search",
    tag = "nodes",
    params(SearchQuery),
    responses((status = 200, description = "Resultados de la búsqueda"))
)]
async fn search(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<NodeSummary>>> {
    if query.q.trim().is_empty() {
        return Ok(Json(Vec::new()));
    }
    let results = state
        .nodes
        .search(user.id(), &query.q, query.content, query.limit)
        .await?;
    Ok(Json(results))
}

/// Interpreta una cabecera `Range: bytes=start-end`.
///
/// Devuelve `None` si el rango no es válido o no es satisfacible.
fn parse_range(header: &str, total: u64) -> Option<(u64, u64)> {
    let spec = header.strip_prefix("bytes=")?.trim();
    // Solo se admite un único rango; las listas multi-rango se ignoran.
    if spec.contains(',') {
        return None;
    }
    let (start_str, end_str) = spec.split_once('-')?;

    let start = if start_str.is_empty() {
        // Sufijo: los últimos N bytes.
        let n: u64 = end_str.trim().parse().ok()?;
        if n == 0 || total == 0 {
            return None;
        }
        total.saturating_sub(n)
    } else {
        start_str.trim().parse().ok()?
    };

    if start >= total {
        return None;
    }

    let end = if end_str.is_empty() {
        total - 1
    } else {
        end_str.trim().parse::<u64>().ok()?.min(total - 1)
    };

    if end < start {
        return None;
    }

    Some((start, end))
}
