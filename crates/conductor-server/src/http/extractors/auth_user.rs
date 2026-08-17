use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use conductor_domain::core::constants::auth::AUTH_SCHEME_BEARER;
use conductor_domain::{ConductorError, User, UserStatus};
use uuid::Uuid;

use crate::core::error::ApiError;
use crate::core::state::AppState;

#[derive(Clone)]
pub struct AuthUser(pub User);

pub async fn authenticate_browser_user(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<User, ApiError> {
    let header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(ConductorError::Unauthorized)?;

    let token = header
        .strip_prefix(AUTH_SCHEME_BEARER)
        .filter(|token| !token.starts_with("evc_"))
        .ok_or(ConductorError::Unauthorized)?;

    let jwt = state.jwt().await.ok_or(ConductorError::SetupRequired)?;
    let claims = jwt.verify(token)?;
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| ConductorError::Unauthorized)?;

    let user = state
        .db
        .users()
        .find_by_id(user_id)
        .await?
        .ok_or(ConductorError::Unauthorized)?;

    let session_version = state
        .db
        .users()
        .session_version(user_id)
        .await?
        .ok_or(ConductorError::Unauthorized)?;
    if claims.ver != session_version {
        return Err(ConductorError::Unauthorized.into());
    }

    match user.status {
        UserStatus::Active => Ok(user),
        UserStatus::Disabled | UserStatus::Pending | UserStatus::Invited => {
            Err(ConductorError::Unauthorized.into())
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if let Some(authenticated) = parts.extensions.get::<AuthUser>() {
            return Ok(authenticated.clone());
        }

        authenticate_browser_user(state, &parts.headers)
            .await
            .map(AuthUser)
    }
}
