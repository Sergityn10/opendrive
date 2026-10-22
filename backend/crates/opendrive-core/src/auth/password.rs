//! Hashing de contraseñas con Argon2id.

use argon2::password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash};
use argon2::{Algorithm, Argon2, Params, Version};

use crate::error::{Error, Result};

/// Parámetros de coste de Argon2id.
///
/// Los valores por defecto (64 MiB de memoria, 3 iteraciones, 1 hilo) siguen la
/// recomendación OWASP para Argon2id.
#[derive(Debug, Clone, Copy)]
pub struct Argon2Params {
    /// Memoria en KiB.
    pub memory_kib: u32,
    /// Iteraciones.
    pub iterations: u32,
    /// Paralelismo.
    pub parallelism: u32,
}

impl Default for Argon2Params {
    fn default() -> Self {
        Self {
            memory_kib: 65536,
            iterations: 3,
            parallelism: 1,
        }
    }
}

impl Argon2Params {
    /// Construye la instancia de `Argon2` con estos parámetros.
    fn argon2(&self) -> Result<Argon2<'static>> {
        let params = Params::new(self.memory_kib, self.iterations, self.parallelism, None)
            .map_err(|e| Error::Internal(format!("parámetros de argon2 inválidos: {e}")))?;
        Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
    }
}

/// Genera un hash Argon2id de la contraseña.
///
/// El resultado usa el formato PHC estándar (`$argon2id$v=19$m=...`), por lo
/// que es verificable con cualquier implementación de Argon2id.
pub fn hash_password(password: &str, params: Argon2Params) -> Result<String> {
    let hash = params
        .argon2()?
        .hash_password(password.as_bytes())
        .map_err(|e| Error::Internal(format!("fallo al hashear la contraseña: {e}")))?;
    Ok(hash.to_string())
}

/// Verifica una contraseña contra un hash PHC.
///
/// Los parámetros de coste se leen del propio hash almacenado, no de la
/// configuración actual: así, cambiar la política de contraseñas no invalida
/// los hashes existentes.
///
/// Devuelve `Error::InvalidCredentials` si no coincide. El error es
/// intencionadamente indistinguible del caso "usuario inexistente", para no
/// filtrar qué cuentas existen.
pub fn verify_password(password: &str, hash: &str) -> Result<()> {
    let parsed = PasswordHash::new(hash).map_err(|_| Error::InvalidCredentials)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| Error::InvalidCredentials)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parámetros baratos para que los tests no tarden un minuto.
    const TEST_PARAMS: Argon2Params = Argon2Params {
        memory_kib: 8192,
        iterations: 1,
        parallelism: 1,
    };

    #[test]
    fn hash_and_verify_roundtrip() {
        let hash = hash_password("contraseña-correcta", TEST_PARAMS).unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password("contraseña-correcta", &hash).is_ok());
    }

    #[test]
    fn wrong_password_is_rejected() {
        let hash = hash_password("contraseña-correcta", TEST_PARAMS).unwrap();
        let err = verify_password("contraseña-incorrecta", &hash).unwrap_err();
        assert!(matches!(err, Error::InvalidCredentials));
    }

    #[test]
    fn malformed_hash_is_rejected() {
        let err = verify_password("cualquiera", "no-es-un-hash").unwrap_err();
        assert!(matches!(err, Error::InvalidCredentials));
    }

    #[test]
    fn same_password_produces_different_hashes() {
        // La sal aleatoria hace que dos hashes del mismo contraseña difieran.
        let a = hash_password("misma", TEST_PARAMS).unwrap();
        let b = hash_password("misma", TEST_PARAMS).unwrap();
        assert_ne!(a, b);
        assert!(verify_password("misma", &a).is_ok());
        assert!(verify_password("misma", &b).is_ok());
    }
}
