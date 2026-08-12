use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use chrono::Utc;
use conductor_auth::hash_token;
use conductor_domain::{
    ClientHeartbeatRequest, ClientHeartbeatResponse, ClientInstallationSummary, ClientMember,
    ClientPolicy, ClientProject, ClientTelemetryPolicy, CollectionLevel, ConductorError,
    RegisterClientRequest, RegisterClientResponse, RegisteredInstallation, SecretScope,
};
use conductor_storage::repos::RegisterInstallationError;
use uuid::Uuid;

use crate::core::error::{ApiError, ApiResult};
use crate::core::state::AppState;
use crate::http::extractors::{authenticate_connection_secret, AuthUser};

const CLIENT_HEARTBEAT_INTERVAL_SECONDS: u32 = 60;
const PRIVACY_NOTICE_VERSION: &str = "2026-08-10";

pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut request): Json<RegisterClientRequest>,
) -> ApiResult<Json<RegisterClientResponse>> {
    validate_registration(&mut request)?;
    let idempotency_key = parse_idempotency_key(&headers)?;
    let principal =
        authenticate_connection_secret(&state, &headers, SecretScope::SubscribeResources).await?;
    let instance = state
        .db
        .instance()
        .get()
        .await?
        .ok_or(ConductorError::SetupRequired)?;
    let request_material = serde_json::to_string(&request).map_err(|_| ConductorError::Internal)?;
    let installation = state
        .db
        .client_installations()
        .register(
            instance.id,
            principal.user.id,
            idempotency_key,
            &hash_token(&request_material),
            &request,
        )
        .await
        .map_err(|error| match error {
            RegisterInstallationError::Conflict => ApiError::from(ConductorError::Conflict(
                "registration key already used".into(),
            )),
            RegisterInstallationError::Database(error) => ApiError::from(error),
        })?;

    let sub_roles = state
        .db
        .roles()
        .list_sub_roles()
        .await?
        .into_iter()
        .filter(|role| principal.user.sub_role_ids.contains(&role.id))
        .collect();
    let tags = state
        .db
        .roles()
        .list_tags()
        .await?
        .into_iter()
        .filter(|tag| principal.user.tag_ids.contains(&tag.id))
        .collect();
    let collection_level = CollectionLevel::parse(&state.db.instance().collection_level().await?);

    Ok(Json(RegisterClientResponse {
        installation: RegisteredInstallation {
            id: installation.id,
            display_name: installation.display_name,
            heartbeat_interval_seconds: CLIENT_HEARTBEAT_INTERVAL_SECONDS,
        },
        project: ClientProject {
            id: instance.id,
            name: instance.project_name,
            display_name: instance.display_name,
            description: instance.description,
            logo_url: instance.logo_url,
        },
        member: ClientMember {
            id: principal.user.id,
            display_name: principal.user.display_name,
            primary_role: principal.user.primary_role,
            sub_roles,
            tags,
        },
        policy: ClientPolicy {
            collection_level,
            telemetry: ClientTelemetryPolicy {
                enabled: collection_level.telemetry_enabled(),
            },
            privacy_notice_version: PRIVACY_NOTICE_VERSION.to_string(),
        },
    }))
}

pub async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClientHeartbeatRequest>,
) -> ApiResult<Json<ClientHeartbeatResponse>> {
    let principal =
        authenticate_connection_secret(&state, &headers, SecretScope::SubscribeResources).await?;
    let instance = state
        .db
        .instance()
        .get()
        .await?
        .ok_or(ConductorError::SetupRequired)?;
    state
        .db
        .client_installations()
        .heartbeat(request.installation_id, instance.id, principal.user.id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("client installation".into()))?;

    Ok(Json(ClientHeartbeatResponse {
        server_time: Utc::now(),
        heartbeat_interval_seconds: CLIENT_HEARTBEAT_INTERVAL_SECONDS,
        connection_state: "active".to_string(),
    }))
}

pub async fn list_member_installations(
    State(state): State<AppState>,
    AuthUser(actor): AuthUser,
    Path(user_id): Path<Uuid>,
) -> ApiResult<Json<Vec<ClientInstallationSummary>>> {
    if actor.id != user_id && !actor.primary_role.can_view_telemetry() {
        return Err(ConductorError::Forbidden.into());
    }
    state
        .db
        .users()
        .find_by_id(user_id)
        .await?
        .ok_or_else(|| ConductorError::NotFound("member".into()))?;
    Ok(Json(
        state
            .db
            .client_installations()
            .list_for_user(user_id)
            .await?
            .into_iter()
            .map(ClientInstallationSummary::from)
            .collect(),
    ))
}

fn parse_idempotency_key(headers: &HeaderMap) -> ApiResult<Uuid> {
    let raw = headers
        .get("Idempotency-Key")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ConductorError::msg("Idempotency-Key header is required"))?;
    Uuid::parse_str(raw).map_err(|_| ConductorError::msg("Idempotency-Key must be a UUID").into())
}

fn validate_registration(request: &mut RegisterClientRequest) -> ApiResult<()> {
    request.display_name = request.display_name.trim().to_string();
    request.evoflux_version = request.evoflux_version.trim().to_string();
    request.workspace_association = request.workspace_association.take().and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    });

    if request.display_name.is_empty() || request.display_name.len() > 120 {
        return Err(ConductorError::msg("display_name must be 1–120 characters").into());
    }
    if request.evoflux_version.is_empty() || request.evoflux_version.len() > 64 {
        return Err(ConductorError::msg("evoflux_version must be 1–64 characters").into());
    }
    if let Some(value) = request.workspace_association.as_deref() {
        if value.len() > 120
            || value.contains('/')
            || value.contains('\\')
            || value == "."
            || value == ".."
        {
            return Err(ConductorError::msg(
                "workspace_association must be a short label, not a local path",
            )
            .into());
        }
    }
    Ok(())
}
