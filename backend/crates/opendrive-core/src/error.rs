//! Errores del dominio.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Error de negocio de OpenDrive.
///
/// Los errores de I/O o de base de datos se convierten en [`Error::Internal`]
/// y nunca se exponen al cliente con detalle: el detalle va al log, el
/// cliente recibe un mensaje genérico y un `trace_id`.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no autenticado")]
    Unauthorized,

    #[error("sesión inválida o expirada")]
    SessionExpired,

    #[error("credenciales inválidas")]
    InvalidCredentials,

    #[error("el usuario ya existe")]
    UserAlreadyExists,

    #[error("usuario no encontrado")]
    UserNotFound,

    #[error("permisos insuficientes")]
    Forbidden,

    #[error("recurso no encontrado: {0}")]
    NotFound(String),

    #[error("entrada inválida: {0}")]
    Validation(String),

    #[error("conflicto: {0}")]
    Conflict(String),

    #[error("la cuota de almacenamiento se ha excedido")]
    QuotaExceeded,

    #[error("el archivo es demasiado grande (máximo {max} bytes)")]
    PayloadTooLarge { max: u64 },

    #[error("error interno: {0}")]
    Internal(String),
}

impl Error {
    /// Código HTTP asociado al error.
    pub fn status(&self) -> StatusCode {
        match self {
            Error::Unauthorized | Error::SessionExpired | Error::InvalidCredentials => {
                StatusCode::UNAUTHORIZED
            }
            Error::UserAlreadyExists | Error::Conflict(_) => StatusCode::CONFLICT,
            Error::UserNotFound => StatusCode::NOT_FOUND,
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::NotFound(_) => StatusCode::NOT_FOUND,
            Error::Validation(_) => StatusCode::BAD_REQUEST,
            Error::QuotaExceeded => StatusCode::INSUFFICIENT_STORAGE,
            Error::PayloadTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            Error::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Código estable para el cliente, útil para lógica de UI.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Unauthorized => "unauthorized",
            Error::SessionExpired => "session_expired",
            Error::InvalidCredentials => "invalid_credentials",
            Error::UserAlreadyExists => "user_exists",
            Error::UserNotFound => "user_not_found",
            Error::Forbidden => "forbidden",
            Error::NotFound(_) => "not_found",
            Error::Validation(_) => "validation_error",
            Error::Conflict(_) => "conflict",
            Error::QuotaExceeded => "quota_exceeded",
            Error::PayloadTooLarge { .. } => "payload_too_large",
            Error::Internal(_) => "internal_error",
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = self.status();
        if status.is_server_error() {
            tracing::error!(error = %self, "error interno");
        } else {
            tracing::debug!(error = %self, code = self.code(), "error de cliente");
        }
        let body = Json(json!({
            "error": {
                "code": self.code(),
                "message": self.to_string(),
            }
        }));
        (status, body).into_response()
    }
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => Error::NotFound("recurso".into()),
            sqlx::Error::Database(db) => {
                if db.code().as_deref() == Some("23505") {
                    Error::Conflict("el recurso ya existe".into())
                } else {
                    Error::Internal(format!("error de base de datos: {err}"))
                }
            }
            _ => Error::Internal(format!("error de base de datos: {err}")),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Internal(format!("error de E/S: {err}"))
    }
}

impl From<axum::http::header::InvalidHeaderValue> for Error {
    fn from(err: axum::http::header::InvalidHeaderValue) -> Self {
        Error::Internal(format!("valor de cabecera no válido: {err}"))
    }
}

/// Resultado alias del crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;
