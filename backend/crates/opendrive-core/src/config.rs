//! Configuración de la aplicación, leída del entorno.

use std::path::PathBuf;

use clap::Parser;

/// Configuración efectiva de OpenDrive.
///
/// Todos los campos tienen valor por defecto sensato para que el servidor
/// arranque sin configuración en un entorno de desarrollo.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "opendrive",
    version,
    about = "OpenDrive — servidor de archivos autoalojado"
)]
pub struct Config {
    /// Dirección TCP de escucha.
    #[arg(long, env = "OPENDRIVE_BIND", default_value = "0.0.0.0:8080")]
    pub bind: String,

    /// URL pública de la instancia. Se usa para los enlaces de compartir.
    #[arg(
        long,
        env = "OPENDRIVE_PUBLIC_URL",
        default_value = "http://localhost:8080"
    )]
    pub public_url: String,

    /// Directorio raíz de datos: blobs, staging y miniaturas.
    #[arg(long, env = "OPENDRIVE_DATA_DIR", default_value = "./data")]
    pub data_dir: PathBuf,

    /// Cadena de conexión a Postgres.
    #[arg(long, env = "DATABASE_URL")]
    pub database_url: String,

    /// Correo del administrador creado en el primer arranque.
    #[arg(
        long,
        env = "OPENDRIVE_ADMIN_EMAIL",
        default_value = "admin@opendrive.local"
    )]
    pub admin_email: String,

    /// Contraseña del administrador. Si se deja vacía no se crea el usuario.
    #[arg(long, env = "OPENDRIVE_ADMIN_PASSWORD", default_value = "")]
    pub admin_password: String,

    /// Nombre visible del administrador.
    #[arg(long, env = "OPENDRIVE_ADMIN_USERNAME", default_value = "admin")]
    pub admin_username: String,

    /// Vida de la sesión en horas.
    #[arg(long, env = "OPENDRIVE_SESSION_TTL_HOURS", default_value_t = 24)]
    pub session_ttl_hours: i64,

    /// Tamaño máximo por archivo en bytes. 0 desactiva el límite.
    #[arg(long, env = "OPENDRIVE_MAX_UPLOAD_SIZE", default_value_t = 0)]
    pub max_upload_size: u64,

    /// Horas hasta que se considera abandonado un upload en curso.
    #[arg(long, env = "OPENDRIVE_TUS_EXPIRY_HOURS", default_value_t = 24)]
    pub tus_expiry_hours: i64,

    /// Cuota por defecto por usuario en bytes. 0 = sin cuota.
    #[arg(long, env = "OPENDRIVE_DEFAULT_QUOTA_BYTES", default_value_t = 0)]
    pub default_quota_bytes: i64,

    /// Desactiva el registro público de usuarios.
    #[arg(long, env = "OPENDRIVE_ALLOW_REGISTRATION", default_value_t = false)]
    pub allow_registration: bool,

    /// Parámetros de Argon2id: memoria en KiB.
    #[arg(long, env = "OPENDRIVE_ARGON2_MEMORY_KIB", default_value_t = 65536)]
    pub argon2_memory_kib: u32,

    /// Parámetros de Argon2id: iteraciones.
    #[arg(long, env = "OPENDRIVE_ARGON2_ITERATIONS", default_value_t = 3)]
    pub argon2_iterations: u32,

    /// Consulta el endpoint de salud del servidor local y sale con código 0
    /// si responde. Lo usa el `HEALTHCHECK` del contenedor.
    #[arg(
        long,
        env = "OPENDRIVE_HEALTH_CHECK",
        default_value_t = false,
        hide = true
    )]
    pub health_check: bool,
}

impl Config {
    /// Ruta del directorio de blobs.
    pub fn blobs_dir(&self) -> PathBuf {
        self.data_dir.join("blobs")
    }

    /// Ruta del directorio de staging (uploads tus en curso).
    pub fn staging_dir(&self) -> PathBuf {
        self.data_dir.join("staging")
    }

    /// Ruta del directorio de miniaturas.
    pub fn thumbs_dir(&self) -> PathBuf {
        self.data_dir.join("thumbs")
    }

    /// Duración de la sesión.
    pub fn session_ttl(&self) -> std::time::Duration {
        std::time::Duration::from_secs((self.session_ttl_hours.max(1) as u64) * 3600)
    }

    /// Duración de expiración de un upload tus en curso.
    pub fn tus_expiry(&self) -> std::time::Duration {
        std::time::Duration::from_secs((self.tus_expiry_hours.max(1) as u64) * 3600)
    }
}
