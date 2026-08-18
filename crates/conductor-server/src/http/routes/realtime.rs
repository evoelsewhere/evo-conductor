use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use conductor_domain::{
    scope_is_role_compatible, AuthorizationTarget, SecretScope, TargetType, User, UserStatus,
};
use serde_json::{json, Value};
use tokio::sync::broadcast::error::RecvError;
use tokio::time::{interval_at, Instant, MissedTickBehavior};
use uuid::Uuid;

use crate::core::state::AppState;
use crate::http::authorization::{authorize_current_connection_target, RouteAuthorization};
use crate::http::extractors::authenticate_connection_secret;
use crate::http::realtime::{capacity_response, RealtimeSignal, PROTOCOL_NAME};

pub async fn events(
    State(state): State<AppState>,
    axum::Extension(route): axum::Extension<RouteAuthorization>,
    headers: HeaderMap,
) -> Response {
    let principal =
        match authenticate_connection_secret(&state, &headers, SecretScope::SubscribeResources)
            .await
        {
            Ok(principal) => principal,
            Err(error) => return error.into_response(),
        };
    if let Err(error) = authorize_current_connection_target(
        &state,
        &route,
        &principal,
        AuthorizationTarget {
            project_id: None,
            target_type: TargetType::Resource,
            target_id: None,
            owner_id: None,
            resource_kind: None,
            lifecycle: None,
            // The stream transports invalidations only; every emitted resource
            // signal is still filtered against the refreshed owner profile.
            effective_audience: Some(true),
        },
    )
    .await
    {
        return error.into_response();
    }

    let permit = match state
        .realtime
        .try_connect(principal.secret.id, principal.secret.owner_user_id)
    {
        Ok(permit) => permit,
        Err(error) => return capacity_response(error),
    };

    // The stream is an invalidation channel, not a data plane. Subscribe before
    // advertising the fetch endpoint so a concurrent head change cannot be missed.
    let mut receiver = state.realtime.subscribe();
    let connection_id = Uuid::new_v4();
    let heartbeat_seconds = state.realtime.heartbeat_seconds();
    let secret_id = principal.secret.id;
    let mut owner = principal.user;
    let owner_user_id = owner.id;
    let stream_state = state.clone();

    let stream = async_stream::stream! {
        let _permit = permit;
        let hello_sequence = stream_state.realtime.next_sequence();
        yield Ok::<Event, Infallible>(protocol_event(
            "control.hello",
            hello_sequence,
            Utc::now(),
            json!({
                "connection_id": connection_id,
                "heartbeat_seconds": heartbeat_seconds,
                "snapshot_mode": "smart_fetch",
                "capabilities": ["resources.fetch", "resources.changed", "access.revoke"],
            }),
        ).retry(Duration::from_secs(2)));

        let head_sequence = stream_state.realtime.next_sequence();
        yield Ok(protocol_event(
            "resources.head",
            head_sequence,
            Utc::now(),
            json!({
                "reason": "initial",
                "fetch_url": "/api/v1/resources/fetch",
            }),
        ));

        let heartbeat_duration = Duration::from_secs(heartbeat_seconds);
        let mut heartbeat = interval_at(Instant::now() + heartbeat_duration, heartbeat_duration);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    match current_connection_owner(
                        &stream_state,
                        secret_id,
                        owner_user_id,
                        SecretScope::SubscribeResources,
                    )
                    .await
                    {
                        Ok(Some(current_owner)) => owner = current_owner,
                        Ok(None) => {
                            let sequence = stream_state.realtime.next_sequence();
                            yield Ok(protocol_event(
                                "control.access_revoked",
                                sequence,
                                Utc::now(),
                                json!({ "reason": "access_no_longer_valid" }),
                            ));
                            break;
                        }
                        Err(error) => {
                            tracing::error!(%error, %connection_id, "realtime access revalidation failed");
                            let sequence = stream_state.realtime.next_sequence();
                            yield Ok(protocol_event(
                                "control.resync_required",
                                sequence,
                                Utc::now(),
                                json!({ "fetch_url": "/api/v1/resources/fetch" }),
                            ));
                            break;
                        }
                    }

                    let sequence = stream_state.realtime.next_sequence();
                    yield Ok(protocol_event(
                        "control.heartbeat",
                        sequence,
                        Utc::now(),
                        json!({ "connection_id": connection_id }),
                    ));
                }
                message = receiver.recv() => {
                    match message {
                        Ok(message) => match message.signal {
                            RealtimeSignal::ResourceUpsert { audience, resource }
                                if audience.includes(&owner) =>
                            {
                                yield Ok(protocol_event(
                                    "resources.changed",
                                    message.sequence,
                                    message.emitted_at,
                                    json!({
                                        "reason": "upsert",
                                        "resource_id": resource.id,
                                        "fetch_url": "/api/v1/resources/fetch",
                                    }),
                                ));
                            }
                            RealtimeSignal::ResourceDelete { audience, resource_id }
                                if audience.includes(&owner) =>
                            {
                                yield Ok(protocol_event(
                                    "resources.changed",
                                    message.sequence,
                                    message.emitted_at,
                                    json!({
                                        "reason": "delete",
                                        "resource_id": resource_id,
                                        "fetch_url": "/api/v1/resources/fetch",
                                    }),
                                ));
                            }
                            RealtimeSignal::ResourceAudienceChanged {
                                owner_user_id: changed_owner_user_id,
                            } if changed_owner_user_id == owner_user_id => {
                                yield Ok(protocol_event(
                                    "resources.changed",
                                    message.sequence,
                                    message.emitted_at,
                                    json!({
                                        "reason": "member_access_profile",
                                        "fetch_url": "/api/v1/resources/fetch",
                                    }),
                                ));
                            }
                            RealtimeSignal::AccessRevoked {
                                secret_id: revoked_secret_id,
                                owner_user_id: revoked_owner_user_id,
                                reason,
                            } if revoked_secret_id == Some(secret_id)
                                || revoked_owner_user_id == Some(owner_user_id) =>
                            {
                                yield Ok(protocol_event(
                                    "control.access_revoked",
                                    message.sequence,
                                    message.emitted_at,
                                    json!({ "reason": reason }),
                                ));
                                break;
                            }
                            RealtimeSignal::ServerDrain { retry_after_ms } => {
                                yield Ok(protocol_event(
                                    "control.server_drain",
                                    message.sequence,
                                    message.emitted_at,
                                    json!({ "retry_after_ms": retry_after_ms }),
                                ));
                                break;
                            }
                            _ => {}
                        },
                        Err(RecvError::Lagged(skipped)) => {
                            match current_connection_owner(
                                &stream_state,
                                secret_id,
                                owner_user_id,
                                SecretScope::SubscribeResources,
                            )
                            .await
                            {
                                Ok(Some(current_owner)) => owner = current_owner,
                                Ok(None) => {
                                    let sequence = stream_state.realtime.next_sequence();
                                    yield Ok(protocol_event(
                                        "control.access_revoked",
                                        sequence,
                                        Utc::now(),
                                        json!({ "reason": "access_no_longer_valid" }),
                                    ));
                                    break;
                                }
                                Err(error) => {
                                    tracing::error!(%error, %connection_id, "realtime access revalidation failed");
                                    let sequence = stream_state.realtime.next_sequence();
                                    yield Ok(protocol_event(
                                        "control.resync_required",
                                        sequence,
                                        Utc::now(),
                                        json!({ "fetch_url": "/api/v1/resources/fetch" }),
                                    ));
                                    break;
                                }
                            }

                            let sequence = stream_state.realtime.next_sequence();
                            yield Ok(protocol_event(
                                "control.resync_required",
                                sequence,
                                Utc::now(),
                                json!({
                                    "reason": "subscriber_lagged",
                                    "skipped_events": skipped,
                                    "fetch_url": "/api/v1/resources/fetch",
                                }),
                            ));
                        }
                        Err(RecvError::Closed) => break,
                    }
                }
            }
        }
    };

    let mut response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(10))
                .text("keep-alive"),
        )
        .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-transform"),
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
    response
}

fn protocol_event(
    event_name: &'static str,
    sequence: u64,
    emitted_at: DateTime<Utc>,
    data: Value,
) -> Event {
    Event::default()
        .event(event_name)
        .id(sequence.to_string())
        .data(
            json!({
                "protocol": PROTOCOL_NAME,
                "sequence": sequence.to_string(),
                "emitted_at": emitted_at,
                "data": data,
            })
            .to_string(),
        )
}

async fn current_connection_owner(
    state: &AppState,
    secret_id: Uuid,
    owner_user_id: Uuid,
    required_scope: SecretScope,
) -> Result<Option<User>, conductor_storage::StorageError> {
    let Some(secret) = state.db.secrets().find_by_id(secret_id).await? else {
        return Ok(None);
    };
    if secret.owner_user_id != owner_user_id
        || secret.revoked_at.is_some()
        || secret
            .expires_at
            .is_some_and(|expires_at| expires_at <= Utc::now())
        || !secret.scopes.contains(&required_scope)
    {
        return Ok(None);
    }

    let Some(owner) = state.db.users().find_by_id(owner_user_id).await? else {
        return Ok(None);
    };
    if owner.status != UserStatus::Active
        || !scope_is_role_compatible(owner.primary_role, required_scope)
    {
        return Ok(None);
    }

    Ok(Some(owner))
}
