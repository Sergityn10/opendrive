//! Estado compartido de la aplicación.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use fileloft_core::handler::TusHandler;
use fileloft_store_fs::{FileLocker, FileStore};
use opendrive_core::config::Config;
use opendrive_core::db::{NodeRepository, UserRepository};
use opendrive_core::error::Result;
use opendrive_core::storage::{BlobStore, TextExtractor, ThumbnailService};
use sqlx::PgPool;
use uuid::Uuid;

/// Un upload en curso.
#[derive(Debug, Clone)]
pub struct UploadRecord {
    /// Identificador del upload.
    #[allow(dead_code)]
    pub id: String,
    /// Usuario propietario.
    pub user_id: Uuid,
    /// Carpeta de destino.
    pub parent_id: Option<Uuid>,
    /// Nombre del archivo.
    pub filename: String,
    /// Tipo MIME detectado.
    pub mime_type: Option<String>,
    /// Tamaño total declarado.
    pub size_bytes: i64,
    /// Fecha de creación.
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
}

/// Estado compartido entre todos los handlers.
#[derive(Clone)]
pub struct AppState {
    /// Pool de conexiones a Postgres.
    pub pool: PgPool,
    /// Repositorio de usuarios.
    pub users: UserRepository,
    /// Repositorio de nodos del árbol de archivos.
    pub nodes: NodeRepository,
    /// Almacén de blobs en disco.
    pub blobs: BlobStore,
    /// Generador de miniaturas con cache en disco.
    pub thumbnails: ThumbnailService,
    /// Extractor de texto para la búsqueda por contenido.
    pub text: TextExtractor,
    /// Handler del protocolo tus.
    pub tus_handler: Arc<TusHandler<FileStore, FileLocker>>,
    /// Handler de WebDAV.
    pub dav_handler: Arc<dav_server::DavHandler<opendrive_core::dav::DavUser>>,
    /// Configuración efectiva.
    pub config: Arc<Config>,
}

impl AppState {
    /// Construye el estado a partir de la configuración.
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let pool = build_pool(&config.database_url).await?;

        let blobs = BlobStore::new(config.blobs_dir())?;
        tracing::info!(path = %config.blobs_dir().display(), "almacén de blobs listo");

        let thumbnails = ThumbnailService::new(config.thumbs_dir())?;
        tracing::info!(path = %config.thumbs_dir().display(), "cache de miniaturas lista");

        let mut state = Self {
            users: UserRepository::new(pool.clone()),
            nodes: NodeRepository::new(pool.clone()),
            blobs,
            thumbnails,
            text: TextExtractor::new(),
            tus_handler: Arc::new(TusHandler::new(
                FileStore::new(config.staging_dir()),
                Some(FileLocker::new(config.staging_dir().join("locks"))),
                Default::default(),
            )),
            // Valores provisionales: los handlers definitivos se construyen
            // después, cuando ya existe el estado completo.
            dav_handler: Arc::new(
                dav_server::DavHandler::<opendrive_core::dav::DavUser>::builder().build_handler(),
            ),
            config: Arc::new(config),
            pool,
        };

        // El handler de tus necesita el estado para validar cuota y carpeta,
        // así que se construye después y se vuelve a guardar.
        state.tus_handler = crate::tus::build_handler(&state);
        state.dav_handler = Arc::new(crate::dav::build_handler(&state));

        Ok(state)
    }

    /* ---------------------------------------------------------------------
     * Uploads en curso
     * ------------------------------------------------------------------ */

    /// Registra un upload en curso.
    pub async fn register_upload(
        &self,
        id: &str,
        user_id: Uuid,
        parent_id: Uuid,
        filename: &str,
        mime_type: Option<&str>,
        size_bytes: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO uploads (id, user_id, parent_id, filename, mime_type, size_bytes, state)
             VALUES ($1, $2, $3, $4, $5, $6, 'pending')
             ON CONFLICT (id) DO NOTHING",
        )
        .bind(id)
        .bind(user_id)
        .bind(parent_id)
        .bind(filename)
        .bind(mime_type)
        .bind(size_bytes)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Busca un upload en curso por identificador.
    pub async fn find_upload(&self, id: &str) -> Result<Option<UploadRecord>> {
        let record = sqlx::query_as::<
            _,
            (
                String,
                Uuid,
                Option<Uuid>,
                String,
                Option<String>,
                i64,
                DateTime<Utc>,
            ),
        >(
            "SELECT id, user_id, parent_id, filename, mime_type, size_bytes, created_at
             FROM uploads WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(record.map(
            |(id, user_id, parent_id, filename, mime_type, size_bytes, created_at)| UploadRecord {
                id,
                user_id,
                parent_id,
                filename,
                mime_type,
                size_bytes,
                created_at,
            },
        ))
    }

    /// `true` si el upload existe y pertenece al usuario indicado.
    pub async fn upload_belongs_to(&self, id: &str, user_id: Uuid) -> bool {
        let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM uploads WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();
        owner == Some(user_id)
    }

    /// Elimina el registro de un upload.
    pub async fn delete_upload(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM uploads WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Elimina los uploads caducados y sus archivos de staging.
    ///
    /// Se ejecuta periódicamente para no dejar residuos de subidas
    /// abandonadas.
    pub async fn cleanup_expired_uploads(&self, older_than: std::time::Duration) -> Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::from_std(older_than).unwrap_or_default();

        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT id, filename FROM uploads WHERE updated_at < $1 AND state = 'pending'",
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await?;

        for (id, _) in &rows {
            let _ = self.blobs.remove_staging(id).await;
        }

        if rows.is_empty() {
            return Ok(0);
        }

        let result = sqlx::query("DELETE FROM uploads WHERE updated_at < $1 AND state = 'pending'")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}

/// Construye el pool de conexiones y se asegura de que existan las
/// extensiones de Postgres que usa el esquema.
async fn build_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(16)
        .min_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .idle_timeout(std::time::Duration::from_secs(600))
        .max_lifetime(std::time::Duration::from_secs(1800))
        .connect(database_url)
        .await?;

    for extension in ["ltree", "pg_trgm", "citext"] {
        // El nombre de la extensión viene de una lista literal de este
        // archivo, no de entrada de usuario: es seguro interpolarlo.
        let statement = format!("CREATE EXTENSION IF NOT EXISTS {extension}");
        if let Err(err) = sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&pool)
            .await
        {
            tracing::warn!(%err, %extension, "no se pudo crear la extensión");
        }
    }

    Ok(pool)
}
