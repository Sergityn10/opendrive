//! Implementación de `SessionStore` sobre Postgres.
//!
//! Se implementa a mano en lugar de usar `tower-sessions-sqlx-store` porque
//! esa versión publicada fija `sqlx ^0.8`, lo que provocaría que el árbol de
//! dependencias contenga dos versiones incompatibles de sqlx (0.8 y 0.9) y que
//! los `PgPool` de ambas no sean intercambiables.
//!
//! El formato de la sesión se serializa como JSON en la columna `data`. El
//! identificador de sesión (`i128`) se almacena como texto para no depender
//! del rango de `bigint` de Postgres.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use time::OffsetDateTime;
use tower_sessions::session::{Id, Record};
use tower_sessions::session_store::{Error as StoreError, Result as StoreResult, SessionStore};

use crate::models::user::UserRole;

/// Convierte un `OffsetDateTime` de `time` al `DateTime<Utc>` de chrono.
///
/// `tower-sessions` trabaja con `time::OffsetDateTime`, pero sqlx no sabe
/// decodificar ese tipo en Postgres. La conversión se hace en el borde.
fn to_chrono(odt: OffsetDateTime) -> DateTime<Utc> {
    DateTime::from_timestamp(odt.unix_timestamp(), odt.nanosecond())
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp(0, 0).expect("epoch válido"))
}

/// Convierte un `DateTime<Utc>` de chrono al `OffsetDateTime` de `time`.
fn to_offset(dt: DateTime<Utc>) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(dt.timestamp())
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .replace_nanosecond(dt.timestamp_subsec_nanos())
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

/// Identificador del usuario dentro de la sesión.
pub const SESSION_USER_KEY: &str = "user_id";

/// Datos del usuario autenticado guardados en la sesión.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionUser {
    /// Identificador del usuario.
    pub id: uuid::Uuid,
    /// Nombre de usuario.
    pub username: String,
    /// Correo electrónico.
    pub email: String,
    /// Rol en la instancia.
    pub role: UserRole,
}

/// `SessionStore` respaldado por Postgres.
#[derive(Debug, Clone)]
pub struct PgSessionStore {
    pool: PgPool,
}

impl PgSessionStore {
    /// Crea un store sobre el pool indicado.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Elimina periódicamente las sesiones expiradas.
    ///
    /// Se lanza como tarea de fondo al arrancar el servidor. Sin esto, la tabla
    /// `sessions` crecería indefinidamente.
    pub async fn continuously_delete_expired(self, interval: std::time::Duration) {
        let mut ticker = tokio::time::interval(interval);
        // La primera llamada del intervalo se dispara inmediatamente; la
        // saltamos para no borrar nada justo al arrancar.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match self.delete_expired().await {
                Ok(n) => {
                    if n > 0 {
                        tracing::debug!(count = n, "sesiones expiradas eliminadas");
                    }
                }
                Err(err) => {
                    tracing::warn!(error = %err, "no se pudieron eliminar las sesiones expiradas");
                }
            }
        }
    }

    /// Elimina las sesiones cuya fecha de expiración ya pasó.
    pub async fn delete_expired(&self) -> StoreResult<u64> {
        let now = Utc::now();
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= $1")
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Backend(format!("no se pudieron borrar las sesiones: {e}")))?;
        Ok(result.rows_affected())
    }
}

#[async_trait]
impl SessionStore for PgSessionStore {
    /// Crea una sesión nueva.
    ///
    /// Mitiga colisiones de identificador reintentando con un id nuevo, igual
    /// que hace el `MemoryStore` de referencia.
    async fn create(&self, record: &mut Record) -> StoreResult<()> {
        const MAX_ATTEMPTS: u32 = 8;

        for attempt in 0..MAX_ATTEMPTS {
            let id = record.id.to_string();
            let data = serde_json::to_value(&record.data)
                .map_err(|e| StoreError::Encode(format!("no se pudo serializar la sesión: {e}")))?;
            let expires_at = to_chrono(record.expiry_date);

            let inserted = sqlx::query(
                "INSERT INTO sessions (id, data, expires_at) VALUES ($1, $2, $3)
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(&id)
            .bind(&data)
            .bind(expires_at)
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Backend(format!("no se pudo crear la sesión: {e}")))?;

            if inserted.rows_affected() == 1 {
                return Ok(());
            }

            // El identificador ya existe: generamos otro y reintentamos.
            if attempt + 1 < MAX_ATTEMPTS {
                record.id = Id::default();
            }
        }

        Err(StoreError::Backend(
            "no se pudo asignar un identificador de sesión único".into(),
        ))
    }

    /// Actualiza una sesión existente.
    async fn save(&self, record: &Record) -> StoreResult<()> {
        let id = record.id.to_string();
        let data = serde_json::to_value(&record.data)
            .map_err(|e| StoreError::Encode(format!("no se pudo serializar la sesión: {e}")))?;
        let expires_at = to_chrono(record.expiry_date);

        sqlx::query(
            "INSERT INTO sessions (id, data, expires_at) VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE SET data = EXCLUDED.data, expires_at = EXCLUDED.expires_at",
        )
        .bind(&id)
        .bind(&data)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Backend(format!("no se pudo guardar la sesión: {e}")))?;

        Ok(())
    }

    /// Carga una sesión. Devuelve `None` si no existe o ha expirado.
    async fn load(&self, session_id: &Id) -> StoreResult<Option<Record>> {
        let id = session_id.to_string();
        let row: Option<(serde_json::Value, DateTime<Utc>)> =
            sqlx::query_as("SELECT data, expires_at FROM sessions WHERE id = $1")
                .bind(&id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| StoreError::Backend(format!("no se pudo cargar la sesión: {e}")))?;

        let Some((data, expires_at)) = row else {
            return Ok(None);
        };

        if expires_at <= Utc::now() {
            // Sesión expirada: la eliminamos y la tratamos como ausente.
            let _ = self.delete(session_id).await;
            return Ok(None);
        }

        let data = serde_json::from_value(data)
            .map_err(|e| StoreError::Decode(format!("no se pudo deserializar la sesión: {e}")))?;

        Ok(Some(Record {
            id: *session_id,
            data,
            expiry_date: to_offset(expires_at),
        }))
    }

    /// Elimina una sesión.
    async fn delete(&self, session_id: &Id) -> StoreResult<()> {
        let id = session_id.to_string();
        sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(&id)
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Backend(format!("no se pudo borrar la sesión: {e}")))?;
        Ok(())
    }
}
