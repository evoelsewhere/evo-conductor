use axum::{
    extract::{Path, State},
    Extension, Json,
};
use conductor_auth::generate_connection_token;
use conductor_domain::{
    AuthorizationTarget, ConductorError, ConnectionSecret, CreateSecretRequest, CreatedSecret,
    TargetType,
};
use std::collections::HashSet;
use uuid::Uuid;

use crate::core::error::ApiResult;
use crate::core::state::AppState;
use crate::http::authorization::{authorize_current_browser_target, RouteAuthorization};
use crate::http::extractors::AuthUser;

pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Vec<ConnectionSecret>>> {
    Ok(Json(state.db.secrets().list_for_user(user.id).await?))
}

pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Json(mut req): Json<CreateSecretRequest>,
) -> ApiResult<Json<CreatedSecret>> {
    validate_create_request(&mut req)?;
    issue_secret(&state, user.id, req).await.map(Json)
}

pub async fn revoke(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(user): AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let secret = state
        .db
        .secrets()
        .find_by_id(id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("secret".into()))?;
    let target = AuthorizationTarget {
        project_id: Some(project_id(&state).await?),
        target_type: TargetType::ConnectionToken,
        target_id: Some(secret.id),
        owner_id: Some(secret.owner_user_id),
        resource_kind: None,
        lifecycle: None,
        effective_audience: None,
    };
    if let Err(error) = authorize_current_browser_target(&state, &route, &user, target).await {
        if secret.owner_user_id != user.id {
            return Err(ConductorError::NotFound("secret".into()).into());
        }
        return Err(error);
    }
    let ok = state.db.secrets().revoke(id, user.id).await?;
    if !ok {
        return Err(ConductorError::NotFound("secret".into()).into());
    }
    state.realtime.disconnect_secret(id, "secret_revoked");
    Ok(Json(serde_json::json!({ "revoked": true })))
}

pub async fn list_for_member(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(member_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ConnectionSecret>>> {
    let member = state
        .db
        .users()
        .find_by_id(member_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
    authorize_member_secret_target(&state, &route, &actor, member.id, true).await?;
    Ok(Json(state.db.secrets().list_for_user(member_id).await?))
}

pub async fn create_for_member(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path(member_id): Path<Uuid>,
    Json(mut req): Json<CreateSecretRequest>,
) -> ApiResult<Json<CreatedSecret>> {
    // Raw credentials are deliberately self-issued. Administrative authority
    // permits metadata inspection and revocation, never impersonating another
    // member to mint a credential whose raw value the member did not receive.
    let member = state
        .db
        .users()
        .find_by_id(member_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
    authorize_member_secret_target(&state, &route, &actor, member.id, true).await?;
    validate_create_request(&mut req)?;
    issue_secret(&state, member_id, req).await.map(Json)
}

fn validate_create_request(req: &mut CreateSecretRequest) -> ApiResult<()> {
    req.name = req.name.trim().to_string();
    if req.name.is_empty() || req.name.len() > 120 {
        return Err(ConductorError::msg("name is required").into());
    }
    if req.scopes.is_empty() {
        return Err(ConductorError::msg("at least one scope is required").into());
    }
    let unique = req
        .scopes
        .iter()
        .map(|scope| scope.as_str())
        .collect::<HashSet<_>>();
    if unique.len() != req.scopes.len() {
        return Err(ConductorError::msg("scopes cannot contain duplicates").into());
    }
    if req
        .expires_at
        .is_some_and(|expires| expires <= chrono::Utc::now())
    {
        return Err(ConductorError::msg("expires_at must be in the future").into());
    }
    Ok(())
}

async fn issue_secret(
    state: &AppState,
    owner_user_id: Uuid,
    req: CreateSecretRequest,
) -> ApiResult<CreatedSecret> {
    let (token, prefix, hash) = generate_connection_token();
    let secret = state
        .db
        .secrets()
        .insert(
            owner_user_id,
            &req.name,
            &prefix,
            &hash,
            &req.scopes,
            req.expires_at,
        )
        .await?;
    Ok(CreatedSecret { secret, token })
}

pub async fn revoke_for_member(
    State(state): State<AppState>,
    Extension(route): Extension<RouteAuthorization>,
    AuthUser(actor): AuthUser,
    Path((member_id, secret_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<serde_json::Value>> {
    let member = state
        .db
        .users()
        .find_by_id(member_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
    let secret = state
        .db
        .secrets()
        .find_by_id(secret_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("secret".into()))?;
    let belongs_to_member = secret.owner_user_id == member.id;
    if let Err(error) =
        authorize_member_secret_target(&state, &route, &actor, member.id, belongs_to_member).await
    {
        if !belongs_to_member {
            return Err(ConductorError::NotFound("secret".into()).into());
        }
        return Err(error);
    }
    if !state.db.secrets().revoke(secret_id, member_id).await? {
        return Err(ConductorError::NotFound("secret".into()).into());
    }
    state
        .realtime
        .disconnect_secret(secret_id, "secret_revoked");
    Ok(Json(serde_json::json!({ "revoked": true })))
}

async fn authorize_member_secret_target(
    state: &AppState,
    route: &RouteAuthorization,
    actor: &conductor_domain::User,
    member_id: Uuid,
    credential_belongs_to_member: bool,
) -> ApiResult<()> {
    let project_id = project_id(state).await?;
    authorize_current_browser_target(
        state,
        route,
        actor,
        AuthorizationTarget {
            // A member/credential mismatch deliberately withholds the
            // same-project fact so the catalog's MemberSecretPath constraint
            // records a target-stage denial before the handler returns 404.
            project_id: credential_belongs_to_member.then_some(project_id),
            target_type: TargetType::ConnectionToken,
            target_id: Some(member_id),
            owner_id: Some(member_id),
            resource_kind: None,
            lifecycle: None,
            effective_audience: None,
        },
    )
    .await?;
    Ok(())
}

async fn project_id(state: &AppState) -> ApiResult<Uuid> {
    state
        .db
        .instance()
        .authorization_project_id()
        .await?
        .ok_or_else(|| ConductorError::SetupRequired.into())
}
