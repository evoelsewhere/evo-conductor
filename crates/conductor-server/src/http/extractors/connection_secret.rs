use std::time::Duration;

use axum::http::HeaderMap;
use conductor_auth::hash_token;
use conductor_domain::{
    scope_is_role_compatible, ConductorError, ConnectionSecret, SecretScope, User, UserStatus,
};

use crate::core::error::ApiResult;
use crate::core::state::AppState;

#[derive(Clone)]
pub struct ConnectionPrincipal {
    pub secret: ConnectionSecret,
    pub user: User,
}

tokio::task_local! {
    static CURRENT_CONNECTION_PRINCIPAL: ConnectionPrincipal;
}

pub(crate) async fn connection_principal_scope<F, T>(principal: ConnectionPrincipal, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    CURRENT_CONNECTION_PRINCIPAL.scope(principal, future).await
}

pub async fn authenticate_connection_secret(
    state: &AppState,
    headers: &HeaderMap,
    required_scope: SecretScope,
) -> ApiResult<ConnectionPrincipal> {
    let principal = authenticate_connection_principal(state, headers).await?;
    if !principal.secret.scopes.contains(&required_scope)
        || !scope_is_role_compatible(principal.user.primary_role, required_scope)
    {
        return Err(ConductorError::Forbidden.into());
    }
    Ok(principal)
}

pub(crate) async fn authenticate_connection_principal(
    state: &AppState,
    headers: &HeaderMap,
) -> ApiResult<ConnectionPrincipal> {
    if let Ok(principal) = CURRENT_CONNECTION_PRINCIPAL.try_with(Clone::clone) {
        return Ok(principal);
    }

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
    let owner = state
        .db
        .users()
        .find_by_id(secret.owner_user_id)
        .await?
        .ok_or(ConductorError::Unauthorized)?;
    if owner.status != UserStatus::Active {
        return Err(ConductorError::Unauthorized.into());
    }
    Ok(ConnectionPrincipal {
        secret,
        user: owner,
    })
}

/// Update usage only after the complete route authorization/handler succeeds.
pub(crate) async fn mark_connection_secret_used_if_due(
    state: &AppState,
    principal: &ConnectionPrincipal,
) -> ApiResult<()> {
    // Avoid a write on every reconnect during a network flap.
    let should_mark_used = principal.secret.last_used_at.is_none_or(|last_used_at| {
        chrono::Utc::now()
            .signed_duration_since(last_used_at)
            .to_std()
            .unwrap_or_default()
            >= Duration::from_secs(300)
    });
    if should_mark_used {
        state.db.secrets().mark_used(principal.secret.id).await?;
    }
    Ok(())
}
