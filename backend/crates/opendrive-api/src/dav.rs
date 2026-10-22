//! Endpoint WebDAV.
//!
//! El servidor WebDAV se monta en `/dav` y comparte la base de datos con la
//! API REST: las operaciones WebDAV y las de la aplicación web operan sobre el
//! mismo árbol de archivos.

use axum::Router;
use axum::body::Body;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use dav_server::DavHandler;
use dav_server::memls::MemLs;
use opendrive_core::dav::{DavFs, DavUser};
use opendrive_core::error::Error;
use tower_sessions::Session;

use crate::state::AppState;

/// Prefijo de montaje del endpoint WebDAV.
const DAV_PREFIX: &str = "/dav";

/// Router del endpoint WebDAV.
pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/dav", any(handle))
        .route("/dav/", any(handle))
        .route("/dav/{*path}", any(handle))
}

/// Construye el handler de WebDAV con el sistema de archivos de OpenDrive.
pub fn build_handler(state: &AppState) -> DavHandler<DavUser> {
    let fs = DavFs::new(state.pool.clone(), state.nodes.clone(), state.blobs.clone());

    // El sistema de bloqueos vive en memoria: suficiente para una instancia
    // de un solo nodo, que es el caso de uso habitual de un servidor
    // autoalojado.
    let locks: Box<dyn dav_server::ls::DavLockSystem> = MemLs::new();

    DavHandler::builder()
        .filesystem(Box::new(fs))
        .locksystem(locks)
        .strip_prefix(DAV_PREFIX)
        .principal("opendrive")
        .hide_symlinks(true)
        .autoindex(false)
        .build_handler()
}

/// Gestiona una petición WebDAV.
///
/// La autenticación usa el mismo sistema de sesión que el resto de la API.
/// Los clientes WebDAV (rclone, Finder, Explorer) no gestionan cookies de
/// sesión, así que también se acepta autenticación básica con el correo y la
/// contraseña del usuario.
async fn handle(State(state): State<AppState>, req: Request) -> Response {
    let (mut parts, body) = req.into_parts();

    let user = match authenticate(&state, &mut parts).await {
        Ok(user) => user,
        Err(error) => return error_response(&error),
    };

    let handler = state.dav_handler.clone();
    let request = axum::http::Request::from_parts(parts, body);

    let response = handler
        .handle_guarded(request, user.username.clone(), user)
        .await;

    // `dav-server` devuelve su propio tipo de cuerpo: se convierte al de
    // axum, que implementa `http_body::Body`.
    let (parts, body) = response.into_parts();
    let mut builder = Response::builder().status(parts.status);
    for (name, value) in parts.headers.iter() {
        builder = builder.header(name, value);
    }
    builder
        .body(Body::new(body))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Autentica una petición WebDAV.
async fn authenticate(state: &AppState, parts: &mut Parts) -> Result<DavUser, Error> {
    // 1. Cabecera de autorización básica: correo o usuario + contraseña.
    if let Some(value) = parts.headers.get(axum::http::header::AUTHORIZATION) {
        let value = value.to_str().unwrap_or_default();
        let Some(encoded) = value.strip_prefix("Basic ") else {
            return Err(Error::InvalidCredentials);
        };
        let Ok(decoded) = base64_decode(encoded) else {
            return Err(Error::InvalidCredentials);
        };
        let Ok(credentials) = String::from_utf8(decoded) else {
            return Err(Error::InvalidCredentials);
        };
        let Some((identifier, password)) = credentials.split_once(':') else {
            return Err(Error::InvalidCredentials);
        };
        return verify_credentials(state, identifier, password).await;
    }

    // 2. Cookie de sesión: la usan los navegadores.
    if let Ok(session) = Session::from_request_parts(parts, &()).await
        && let Some(user) = session
            .get::<opendrive_core::auth::session::SessionUser>(
                opendrive_core::auth::session::SESSION_USER_KEY,
            )
            .await
            .ok()
            .flatten()
    {
        return Ok(DavUser {
            user_id: user.id,
            username: user.username,
        });
    }

    Err(Error::Unauthorized)
}

/// Verifica las credenciales de un usuario.
async fn verify_credentials(
    state: &AppState,
    identifier: &str,
    password: &str,
) -> Result<DavUser, Error> {
    let Some(user) = state.users.find_by_identifier(identifier).await? else {
        return Err(Error::InvalidCredentials);
    };
    if !user.is_active {
        return Err(Error::Forbidden);
    }
    opendrive_core::auth::verify_password(password, &user.password_hash)?;
    Ok(DavUser {
        user_id: user.id,
        username: user.username,
    })
}

/// Decodifica una cadena en base64 estándar.
fn base64_decode(input: &str) -> Result<Vec<u8>, ()> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input.trim())
        .map_err(|_| ())
}

/// Construye la respuesta de error de WebDAV.
fn error_response(error: &Error) -> Response {
    let status = match error {
        Error::Unauthorized | Error::InvalidCredentials => StatusCode::UNAUTHORIZED,
        Error::Forbidden => StatusCode::FORBIDDEN,
        Error::NotFound(_) => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    let mut response = (
        status,
        axum::Json(serde_json::json!({
            "error": { "code": error.code(), "message": error.to_string() }
        })),
    )
        .into_response();

    if status == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(
            axum::http::header::WWW_AUTHENTICATE,
            axum::http::HeaderValue::from_static("Basic realm=\"OpenDrive\""),
        );
    }

    response
}
