//! Extracción del usuario autenticado a partir de la sesión.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use opendrive_core::auth::session::{SESSION_USER_KEY, SessionUser};
use opendrive_core::error::Error;
use tower_sessions::Session;
use uuid::Uuid;

/// Usuario autenticado extraído de la sesión.
#[derive(Debug, Clone)]
pub struct AuthUser(pub SessionUser);

impl AuthUser {
    /// Identificador del usuario.
    pub fn id(&self) -> Uuid {
        self.0.id
    }

    /// `true` si el usuario es administrador de la instancia.
    ///
    /// Se usa en las rutas de administración, que llegan en la Fase 7.
    #[allow(dead_code)]
    pub fn is_admin(&self) -> bool {
        self.0.role.is_admin()
    }
}

/// Implementación genérica sobre el estado del router: el extractor solo
/// necesita la sesión, que el middleware de `tower-sessions` inyecta en las
/// extensiones de la petición.
impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, &())
            .await
            .map_err(|_| Error::SessionExpired)?;

        let user: Option<SessionUser> = session
            .get(SESSION_USER_KEY)
            .await
            .map_err(|_| Error::SessionExpired)?;

        user.map(AuthUser).ok_or(Error::Unauthorized)
    }
}

/// Usuario autenticado opcional: no falla si no hay sesión.
///
/// Se usa en las rutas públicas de enlace compartido, que llegan en la Fase 7.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct MaybeAuthUser(pub Option<SessionUser>);

impl<S: Send + Sync> FromRequestParts<S> for MaybeAuthUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let session = match Session::from_request_parts(parts, &()).await {
            Ok(s) => s,
            Err(_) => return Ok(Self(None)),
        };
        let user = session
            .get::<SessionUser>(SESSION_USER_KEY)
            .await
            .unwrap_or(None);
        Ok(Self(user))
    }
}
