//! Rutas de autenticación.

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use opendrive_core::auth::session::{SESSION_USER_KEY, SessionUser};
use opendrive_core::auth::{Argon2Params, hash_password, verify_password};
use opendrive_core::error::{Error, Result};
use opendrive_core::models::user::{UserProfile, UserRole};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::extract::AuthUser;
use crate::state::AppState;

/// Cuerpo de la petición de login.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct LoginRequest {
    /// Correo electrónico o nombre de usuario.
    pub identifier: String,
    /// Contraseña.
    pub password: String,
}

/// Cuerpo de la petición de registro.
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RegisterRequest {
    /// Correo electrónico.
    pub email: String,
    /// Nombre de usuario.
    pub username: String,
    /// Contraseña.
    pub password: String,
}

/// Respuesta del login: perfil del usuario y sesión iniciada.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct AuthResponse {
    /// Perfil del usuario autenticado.
    pub user: UserProfile,
    /// `true` si la sesión se ha iniciado.
    pub authenticated: bool,
}

/// Router de autenticación.
pub fn router() -> Router<AppState> {
    Router::<AppState>::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .route("/logout", post(logout))
        .route("/me", get(me))
}

/// Inicia sesión.
#[utoipa::path(
    post,
    path = "/api/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Sesión iniciada", body = AuthResponse),
        (status = 401, description = "Credenciales inválidas"),
    )
)]
async fn login(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    let user = state
        .users
        .find_by_identifier(&req.identifier)
        .await?
        .ok_or(Error::InvalidCredentials)?;

    if !user.is_active {
        return Err(Error::Forbidden);
    }

    verify_password(&req.password, &user.password_hash)?;

    // Se rota el identificador de sesión para evitar ataques de fijación de
    // sesión: tras el login, el identificador anterior deja de ser válido.
    session
        .cycle_id()
        .await
        .map_err(|e| Error::Internal(format!("no se pudo rotar la sesión: {e}")))?;

    let session_user = SessionUser {
        id: user.id,
        username: user.username.clone(),
        email: user.email.clone(),
        role: user.role,
    };

    session
        .insert(SESSION_USER_KEY, &session_user)
        .await
        .map_err(|e| Error::Internal(format!("no se pudo guardar la sesión: {e}")))?;
    session
        .save()
        .await
        .map_err(|e| Error::Internal(format!("no se pudo guardar la sesión: {e}")))?;

    Ok(Json(AuthResponse {
        user: UserProfile::from(user),
        authenticated: true,
    }))
}

/// Registra un usuario nuevo.
#[utoipa::path(
    post,
    path = "/api/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "Usuario creado", body = AuthResponse),
        (status = 400, description = "Datos inválidos"),
        (status = 409, description = "El usuario ya existe"),
    )
)]
async fn register(
    State(state): State<AppState>,
    session: Session,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>> {
    if !state.config.allow_registration {
        return Err(Error::Forbidden);
    }

    let email = req.email.trim().to_lowercase();
    let username = req.username.trim();

    if !email.contains('@') || email.len() > 255 {
        return Err(Error::Validation("correo electrónico no válido".into()));
    }
    if username.len() < 3 || username.len() > 64 {
        return Err(Error::Validation(
            "el nombre de usuario debe tener entre 3 y 64 caracteres".into(),
        ));
    }
    if req.password.len() < 8 {
        return Err(Error::Validation(
            "la contraseña debe tener al menos 8 caracteres".into(),
        ));
    }

    let params = Argon2Params {
        memory_kib: state.config.argon2_memory_kib,
        iterations: state.config.argon2_iterations,
        parallelism: 1,
    };
    let password_hash = hash_password(&req.password, params)?;

    let user = state
        .users
        .create(&opendrive_core::models::user::NewUser {
            email,
            username: username.to_string(),
            password_hash,
            role: UserRole::User,
            quota_bytes: state.config.default_quota_bytes,
        })
        .await?;

    // Cada usuario recibe su carpeta raíz ("Mi Drive").
    let root = state.nodes.create_root(user.id, "Mi Drive").await?;
    tracing::info!(user_id = %user.id, root_id = %root.id, "carpeta raíz creada");

    session
        .cycle_id()
        .await
        .map_err(|e| Error::Internal(format!("no se pudo rotar la sesión: {e}")))?;

    let session_user = SessionUser {
        id: user.id,
        username: user.username.clone(),
        email: user.email.clone(),
        role: user.role,
    };
    session
        .insert(SESSION_USER_KEY, &session_user)
        .await
        .map_err(|e| Error::Internal(format!("no se pudo guardar la sesión: {e}")))?;
    session
        .save()
        .await
        .map_err(|e| Error::Internal(format!("no se pudo guardar la sesión: {e}")))?;

    Ok(Json(AuthResponse {
        user: UserProfile::from(user),
        authenticated: true,
    }))
}

/// Cierra la sesión.
#[utoipa::path(
    post,
    path = "/api/auth/logout",
    tag = "auth",
    responses((status = 200, description = "Sesión cerrada"))
)]
async fn logout(session: Session) -> Result<Json<serde_json::Value>> {
    session
        .delete()
        .await
        .map_err(|e| Error::Internal(format!("no se pudo cerrar la sesión: {e}")))?;
    Ok(Json(serde_json::json!({ "authenticated": false })))
}

/// Devuelve el perfil del usuario autenticado.
#[utoipa::path(
    get,
    path = "/api/auth/me",
    tag = "auth",
    responses(
        (status = 200, description = "Perfil del usuario", body = UserProfile),
        (status = 401, description = "No autenticado"),
    )
)]
async fn me(user: AuthUser) -> Result<Json<UserProfile>> {
    Ok(Json(UserProfile {
        id: user.0.id,
        username: user.0.username,
        email: user.0.email,
        role: user.0.role,
        quota_bytes: 0,
        used_bytes: 0,
    }))
}
