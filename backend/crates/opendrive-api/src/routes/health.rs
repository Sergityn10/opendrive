//! Rutas de salud y estado del servicio.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::state::AppState;

/// Router de salud.
pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/health", get(health))
        .route("/health/ready", get(readiness))
}

/// Respuesta del endpoint de salud.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Estado del servicio.
    pub status: &'static str,
    /// Versión de la aplicación.
    pub version: &'static str,
}

/// Comprobación de vida: el proceso responde.
#[utoipa::path(
    get,
    path = "/api/health",
    tag = "health",
    responses((status = 200, description = "El servicio está vivo"))
)]
async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(HealthResponse {
            status: "ok",
            version: env!("CARGO_PKG_VERSION"),
        }),
    )
}

/// Comprobación de disponibilidad: la base de datos responde.
#[utoipa::path(
    get,
    path = "/api/health/ready",
    tag = "health",
    responses(
        (status = 200, description = "El servicio está listo"),
        (status = 503, description = "La base de datos no responde"),
    )
)]
async fn readiness(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(HealthResponse {
                status: "ready",
                version: env!("CARGO_PKG_VERSION"),
            }),
        ),
        Err(err) => {
            tracing::warn!(error = %err, "la base de datos no responde");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "unavailable",
                    version: env!("CARGO_PKG_VERSION"),
                }),
            )
        }
    }
}
