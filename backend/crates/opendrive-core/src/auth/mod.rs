//! Autenticación: hashing de contraseñas y gestión de sesiones.

pub mod password;
pub mod session;

pub use password::{Argon2Params, hash_password, verify_password};
pub use session::{PgSessionStore, SessionUser};
