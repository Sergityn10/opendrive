//! Repositorio de usuarios.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::models::user::{NewUser, User, UserRole};

/// Operaciones sobre la tabla `users`.
#[derive(Debug, Clone)]
pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    /// Crea el repositorio sobre el pool indicado.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Busca un usuario por su identificador.
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    /// Busca un usuario por correo o nombre de usuario (sin distinguir mayúsculas).
    pub async fn find_by_identifier(&self, identifier: &str) -> Result<Option<User>> {
        let ident = identifier.trim();
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users
             WHERE lower(email) = lower($1) OR lower(username) = lower($1)
             LIMIT 1",
        )
        .bind(ident)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user)
    }

    /// Busca un usuario por correo electrónico.
    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE lower(email) = lower($1)")
            .bind(email.trim())
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    /// Inserta un usuario nuevo.
    pub async fn create(&self, new_user: &NewUser) -> Result<User> {
        let user = sqlx::query_as::<_, User>(
            "INSERT INTO users (email, username, password_hash, role, quota_bytes, used_bytes, is_active)
             VALUES ($1, $2, $3, $4, $5, 0, TRUE)
             RETURNING *",
        )
        .bind(new_user.email.trim())
        .bind(new_user.username.trim())
        .bind(&new_user.password_hash)
        .bind(new_user.role)
        .bind(new_user.quota_bytes)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db) if db.code().as_deref() == Some("23505") => {
                Error::UserAlreadyExists
            }
            other => Error::from(other),
        })?;
        Ok(user)
    }

    /// Actualiza el hash de contraseña de un usuario.
    pub async fn update_password(&self, id: Uuid, password_hash: &str) -> Result<()> {
        sqlx::query("UPDATE users SET password_hash = $2, updated_at = now() WHERE id = $1")
            .bind(id)
            .bind(password_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Actualiza el espacio usado por un usuario.
    pub async fn add_used_bytes(&self, id: Uuid, delta: i64) -> Result<i64> {
        let used: i64 = sqlx::query_scalar(
            "UPDATE users SET used_bytes = GREATEST(0, used_bytes + $2), updated_at = now()
             WHERE id = $1 RETURNING used_bytes",
        )
        .bind(id)
        .bind(delta)
        .fetch_one(&self.pool)
        .await?;
        Ok(used)
    }

    /// Cuenta el total de usuarios registrados.
    pub async fn count(&self) -> Result<i64> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(n)
    }

    /// Lista todos los usuarios.
    pub async fn list(&self) -> Result<Vec<User>> {
        let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at ASC")
            .fetch_all(&self.pool)
            .await?;
        Ok(users)
    }

    /// Crea el usuario administrador por defecto si la instancia está vacía.
    ///
    /// Es idempotente: si ya existe un usuario con ese correo o nombre de
    /// usuario, no hace nada.
    pub async fn ensure_admin(
        &self,
        email: &str,
        username: &str,
        password_hash: &str,
        quota_bytes: i64,
    ) -> Result<Option<User>> {
        if self.count().await? > 0 {
            return Ok(None);
        }
        if self.find_by_identifier(email).await?.is_some()
            || self.find_by_identifier(username).await?.is_some()
        {
            return Ok(None);
        }
        let user = self
            .create(&NewUser {
                email: email.to_string(),
                username: username.to_string(),
                password_hash: password_hash.to_string(),
                role: UserRole::Admin,
                quota_bytes,
            })
            .await?;
        Ok(Some(user))
    }
}
