//! Servidor HTTP de OpenDrive.

mod dav;
mod extract;
mod routes;
mod state;
mod tus;

use std::net::SocketAddr;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use clap::Parser;
use opendrive_core::auth::session::PgSessionStore;
use opendrive_core::config::Config;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tower_sessions::Expiry;
use tower_sessions::SessionManagerLayer;

use crate::state::AppState;

/// Límite máximo del cuerpo de una petición, en bytes.
///
/// Las subidas grandes pasan por el endpoint tus, que no usa este extractor.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // En desarrollo se puede usar un archivo .env; en producción las variables
    // llegan del orquestador.
    let _ = dotenvy::dotenv();

    let config = Config::parse();

    // Modo de autocomprobación: el healthcheck del contenedor llama al
    // servidor por HTTP y sale con código 0 si responde.
    if config.health_check {
        return health_check(&config.bind).await;
    }

    init_tracing();

    tracing::info!(
        bind = %config.bind,
        data_dir = %config.data_dir.display(),
        "iniciando OpenDrive"
    );

    // Las migraciones se aplican al arrancar: así el despliegue es un único
    // comando y no hace falta un paso aparte de migración.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(16)
        .connect(&config.database_url)
        .await?;
    sqlx::migrate!("../opendrive-core/migrations")
        .run(&pool)
        .await
        .map_err(|e| anyhow::anyhow!("no se pudieron aplicar las migraciones: {e}"))?;
    tracing::info!("migraciones aplicadas");
    drop(pool);

    let state = AppState::new(config.clone()).await?;

    // Usuario administrador por defecto en el primer arranque.
    if !config.admin_password.is_empty() {
        let params = opendrive_core::auth::Argon2Params {
            memory_kib: config.argon2_memory_kib,
            iterations: config.argon2_iterations,
            parallelism: 1,
        };
        let hash = opendrive_core::auth::hash_password(&config.admin_password, params)?;
        if let Some(user) = state
            .users
            .ensure_admin(
                &config.admin_email,
                &config.admin_username,
                &hash,
                config.default_quota_bytes,
            )
            .await?
        {
            state.nodes.create_root(user.id, "Mi Drive").await?;
            tracing::info!(email = %user.email, "usuario administrador creado");
        }
    }

    // Tarea de fondo que limpia las sesiones expiradas.
    let store = PgSessionStore::new(state.pool.clone());
    tokio::spawn(
        store
            .clone()
            .continuously_delete_expired(std::time::Duration::from_secs(300)),
    );

    // Tarea de fondo que elimina los uploads abandonados y sus archivos de
    // staging, para que no se acumulen residuos en el disco.
    let cleanup_state = state.clone();
    let cleanup_interval = std::time::Duration::from_secs(3600);
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(cleanup_interval);
        ticker.tick().await; // la primera llamada es inmediata: se omite
        loop {
            ticker.tick().await;
            match cleanup_state
                .cleanup_expired_uploads(cleanup_interval * 2)
                .await
            {
                Ok(n) if n > 0 => {
                    tracing::info!(count = n, "uploads caducados eliminados");
                }
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!(error = %err, "no se pudieron limpiar los uploads caducados");
                }
            }
        }
    });

    let session_layer = SessionManagerLayer::new(store)
        .with_secure(!config.public_url.starts_with("http://"))
        .with_expiry(Expiry::OnInactivity(time::Duration::seconds(
            config.session_ttl_hours.max(1) * 3600,
        )));

    let app = build_router(state, session_layer);

    let addr: SocketAddr = config
        .bind
        .parse()
        .map_err(|e| anyhow::anyhow!("dirección de escucha no válida ({}): {e}", config.bind))?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::info!(%addr, "OpenDrive escuchando");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Construye el router HTTP completo.
fn build_router(state: AppState, session_layer: SessionManagerLayer<PgSessionStore>) -> Router {
    let cors = if state.config.public_url.starts_with("http://") {
        // En desarrollo se permite cualquier origen para que el frontend
        // pueda servirse desde Vite en otro puerto.
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
    };

    // La API y tus llevan compresión y límite de cuerpo. WebDAV va aparte:
    //
    // * El cuerpo de un `PUT` de WebDAV es el archivo entero: el límite
    //   de 16 MiB de la API lo rechazaría.
    // * El cuerpo de WebDAV es un flujo que `dav-server` produce con un
    //   generador asíncrono. La capa de compresión vuelve a consultar ese
    //   flujo cuando ya ha terminado, y `dav-server` no lo marca como
    //   completado: el generador entra en pánico y el proceso cae.
    //
    //   Los clientes WebDAV (rclone, Finder, Explorer) tampoco se benefician
    //   de la compresión gzip.
    let api = Router::<AppState>::new()
        .nest("/api/auth", routes::auth::router())
        .nest("/api", routes::nodes::router())
        .nest("/api", routes::health::router())
        .merge(tus::router())
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(CompressionLayer::new());

    Router::<AppState>::new()
        .merge(api)
        .merge(dav::router())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .layer(session_layer)
        // Un pánico en una petición no debe tumbar el servidor: se
        // convierte en un error 500 y el proceso sigue vivo.
        .layer(CatchPanicLayer::custom(handle_panic))
        .with_state(state)
}

/// Respuesta para una petición que ha entrado en pánico.
fn handle_panic(err: Box<dyn std::any::Any + Send + 'static>) -> Response {
    tracing::error!("la petición ha entrado en pánico: {err:?}");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        axum::Json(serde_json::json!({
            "error": { "code": "internal_error", "message": "error interno" }
        })),
    )
        .into_response()
}

/// Consulta el endpoint de salud y devuelve código de salida 0 si responde.
///
/// Se implementa con `TcpStream` y HTTP/1.1 a pelo para no añadir un cliente
/// HTTP completo a la imagen final.
async fn health_check(bind: &str) -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let addr: SocketAddr = bind
        .parse()
        .map_err(|e| anyhow::anyhow!("dirección de escucha no válida: {e}"))?;
    let host = if addr.ip().is_unspecified() {
        "127.0.0.1"
    } else {
        &addr.ip().to_string()
    };

    let mut stream = tokio::net::TcpStream::connect((host, addr.port()))
        .await
        .map_err(|e| anyhow::anyhow!("no se pudo conectar con el servidor: {e}"))?;

    let request = format!(
        "GET /api/health HTTP/1.1\r\nHost: {host}:{}\r\nConnection: close\r\nUser-Agent: opendrive-healthcheck\r\n\r\n",
        addr.port()
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|e| anyhow::anyhow!("no se pudo enviar la petición: {e}"))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .map_err(|e| anyhow::anyhow!("no se pudo leer la respuesta: {e}"))?;

    let head = String::from_utf8_lossy(&response);
    if head.starts_with("HTTP/1.1 200") {
        Ok(())
    } else {
        anyhow::bail!(
            "el servidor no está sano: {}",
            head.lines().next().unwrap_or("?")
        )
    }
}

/// Configura el subscriber de trazas.
fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,tower_http=info"));

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_target(true)
                .with_target(false)
                .with_level(true),
        )
        .with(filter)
        .init();
}
