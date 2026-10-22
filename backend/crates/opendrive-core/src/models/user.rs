//! Modelo de usuario.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Rol de un usuario dentro de la instancia.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, utoipa::ToSchema,
)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum UserRole {
    /// Administrador de la instancia: puede gestionar usuarios y ajustes.
    Admin,
    /// Usuario normal.
    User,
}

impl UserRole {
    /// Devuelve el rol si es administrador.
    pub fn is_admin(self) -> bool {
        matches!(self, UserRole::Admin)
    }
}

/// Un usuario registrado en la instancia.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    /// Identificador único.
    pub id: Uuid,
    /// Correo electrónico (único, insensible a mayúsculas).
    pub email: String,
    /// Nombre de usuario (único, insensible a mayúsculas).
    pub username: String,
    /// Hash Argon2id de la contraseña.
    #[serde(skip_serializing)]
    pub password_hash: String,
    /// Rol dentro de la instancia.
    pub role: UserRole,
    /// Cuota de almacenamiento en bytes. 0 = sin cuota.
    pub quota_bytes: i64,
    /// Espacio usado en bytes.
    pub used_bytes: i64,
    /// Si la cuenta está activa.
    pub is_active: bool,
    /// Fecha de creación.
    pub created_at: DateTime<Utc>,
    /// Última modificación.
    pub updated_at: DateTime<Utc>,
}

/// Datos para crear un usuario nuevo.
#[derive(Debug, Clone)]
pub struct NewUser {
    /// Correo electrónico.
    pub email: String,
    /// Nombre de usuario.
    pub username: String,
    /// Hash de la contraseña ya calculado.
    pub password_hash: String,
    /// Rol.
    pub role: UserRole,
    /// Cuota en bytes.
    pub quota_bytes: i64,
}

/// Vista pública de un usuario, sin datos sensibles.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UserProfile {
    /// Identificador único.
    pub id: Uuid,
    /// Nombre visible.
    pub username: String,
    /// Correo electrónico.
    pub email: String,
    /// Rol.
    pub role: UserRole,
    /// Cuota en bytes.
    pub quota_bytes: i64,
    /// Espacio usado en bytes.
    pub used_bytes: i64,
}

impl From<User> for UserProfile {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            email: u.email,
            role: u.role,
            quota_bytes: u.quota_bytes,
            used_bytes: u.used_bytes,
        }
    }
}
