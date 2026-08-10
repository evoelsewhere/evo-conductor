use std::time::Duration;

use axum::http::HeaderMap;
use conductor_auth::hash_token;
use conductor_domain::{ConductorError, ConnectionSecret, SecretScope, User, UserStatus};

use crate::core::error::ApiResult;
use crate::core::state::AppState;

pub struct ConnectionPrincipal {
    pub secret: ConnectionSecret,
    pub user: User,
}

pub async fn authenticate_connection_secret(
    state: &AppState,
    headers: &HeaderMap,
    required_scope: SecretScope,
) -> ApiResult<ConnectionPrincipal> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| value.starts_with("evc_"))
        .ok_or(ConductorError::Unauthorized)?;

    let secret = state
        .db
        .secrets()
        .find_by_hash(&hash_token(token))
        .await?
        .ok_or(ConductorError::Unauthorized)?;

    if secret
        .expires_at
        .is_some_and(|expires_at| expires_at <= chrono::Utc::now())
    {
        return Err(ConductorError::Unauthorized.into());
    }
    if !secret.scopes.contains(&required_scope) {
        return Err(ConductorError::Forbidden.into());
    }

    let owner = state
        .db
        .users()
        .find_by_id(secret.owner_user_id)
        .await?
        .ok_or(ConductorError::Unauthorized)?;
    if owner.status != UserStatus::Active {
        return Err(ConductorError::Unauthorized.into());
    }

    // Avoid a write on every reconnect during a network flap.
    let should_mark_used = secret.last_used_at.is_none_or(|last_used_at| {
        chrono::Utc::now()
            .signed_duration_since(last_used_at)
            .to_std()
            .unwrap_or_default()
            >= Duration::from_secs(300)
    });
    if should_mark_used {
        state.db.secrets().mark_used(secret.id).await?;
    }

    Ok(ConnectionPrincipal {
        secret,
        user: owner,
    })
}
