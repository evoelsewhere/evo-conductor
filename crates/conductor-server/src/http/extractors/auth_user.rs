use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use conductor_domain::{ConductorError, User, UserStatus};
use uuid::Uuid;

use crate::http::error::ApiError;
use crate::http::state::AppState;

pub struct AuthUser(pub User);

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(ConductorError::Unauthorized)?;

        let token = header
            .strip_prefix("Bearer ")
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
            UserStatus::Active => Ok(AuthUser(user)),
            UserStatus::Disabled => Err(ConductorError::Forbidden.into()),
            UserStatus::Pending | UserStatus::Invited => Err(ConductorError::Forbidden.into()),
        }
    }
}
