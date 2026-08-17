use std::sync::Arc;

use conductor_domain::{
    evaluate_policy, AuthenticationKind, AuthorizationAction, AuthorizationTargetSummary,
    DecisionReason, PermissionKey, PolicyDecision, PolicyInput, PrimaryRole, ResponseProjection,
    SecretScope, TargetConstraint, TargetType, UserStatus, V1_POLICY_REVISION,
};
use conductor_storage::MemberAccessChange;
use serde::Serialize;
use uuid::Uuid;

use crate::core::request_context::RequestContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationStage {
    RouteBoundary,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizationResult {
    Preauthorized,
    Allowed,
    Denied,
}

/// Safe, non-durable handoff for an authorization decision.
///
/// This deliberately has no email, token material, request URI/query, headers,
/// body, file data or database error. REQ-018 may compose a durable observer
/// later without changing the policy evaluation boundary.
#[derive(Debug, Clone, Serialize)]
pub struct AuthorizationEvent {
    pub request_id: Uuid,
    pub policy_revision: String,
    pub normalized_route_id: &'static str,
    pub method: &'static str,
    pub stage: AuthorizationStage,
    pub authentication_kind: AuthenticationKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_credential_id: Option<Uuid>,
    pub actor_id: Uuid,
    pub primary_role_snapshot: PrimaryRole,
    pub action: AuthorizationAction,
    pub declared_requirement_id: String,
    pub evaluated_permissions: Vec<PermissionKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_permission: Option<PermissionKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_scope: Option<SecretScope>,
    pub target_type: TargetType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
    pub authorization_result: AuthorizationResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<DecisionReason>,
    pub matched_constraints: Vec<TargetConstraint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_constraint: Option<TargetConstraint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_projection: Option<ResponseProjection>,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberAccessSnapshotEvent {
    pub primary_role: PrimaryRole,
    pub status: UserStatus,
    pub sub_role_count: usize,
    pub tag_count: usize,
    pub session_version: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CredentialPolicyEffectEvent {
    pub credential_id: Uuid,
    pub scopes: Vec<SecretScope>,
    pub reason: &'static str,
}

/// Safe, non-durable handoff for a committed access-profile mutation.
/// Assignment identifiers and member/token secrets are deliberately excluded.
#[derive(Debug, Clone, Serialize)]
pub struct MemberAccessChangeEvent {
    pub request_id: Uuid,
    pub policy_revision: &'static str,
    pub normalized_route_id: &'static str,
    pub action: AuthorizationAction,
    pub actor_id: Uuid,
    pub target_id: Uuid,
    pub before: MemberAccessSnapshotEvent,
    pub after: MemberAccessSnapshotEvent,
    pub admin_elevation: bool,
    pub audience_changed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_reason: Option<&'static str>,
    pub affected_credentials: Vec<CredentialPolicyEffectEvent>,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
}

impl MemberAccessChangeEvent {
    fn new(
        context: &RequestContext,
        action: AuthorizationAction,
        change: &MemberAccessChange,
    ) -> Self {
        let snapshot =
            |value: &conductor_storage::MemberSecuritySnapshot| MemberAccessSnapshotEvent {
                primary_role: value.primary_role,
                status: value.status,
                sub_role_count: value.sub_role_ids.len(),
                tag_count: value.tag_ids.len(),
                session_version: value.session_version,
            };
        Self {
            request_id: context.request_id,
            policy_revision: V1_POLICY_REVISION,
            normalized_route_id: context.route_id,
            action,
            actor_id: change.actor_id,
            target_id: change.target_id,
            before: snapshot(&change.before),
            after: snapshot(&change.after),
            admin_elevation: change.admin_elevation,
            audience_changed: change.audience_changed,
            status_reason: change.status_reason.map(|reason| reason.as_str()),
            affected_credentials: change
                .revoked_credentials
                .iter()
                .map(|credential| CredentialPolicyEffectEvent {
                    credential_id: credential.credential_id,
                    scopes: credential.scopes.clone(),
                    reason: credential.reason,
                })
                .collect(),
            occurred_at: context.occurred_at,
        }
    }
}

impl AuthorizationEvent {
    fn from_decision(
        context: &RequestContext,
        stage: AuthorizationStage,
        actor_id: Uuid,
        safe_credential_id: Option<Uuid>,
        decision: &PolicyDecision,
    ) -> Self {
        Self {
            request_id: context.request_id,
            policy_revision: decision.policy_revision.clone(),
            normalized_route_id: context.route_id,
            method: context.method,
            stage,
            authentication_kind: decision.authentication_kind,
            safe_credential_id,
            actor_id,
            primary_role_snapshot: decision.role_snapshot,
            action: decision.action,
            declared_requirement_id: decision.declared_requirement_id.clone(),
            evaluated_permissions: decision.evaluated_permissions.clone(),
            resolved_permission: decision.resolved_permission,
            required_scope: decision.required_scope,
            target_type: decision.target_summary.target_type,
            target_id: decision.target_summary.target_id,
            project_id: decision.target_summary.project_id,
            authorization_result: if decision.allow {
                AuthorizationResult::Allowed
            } else {
                AuthorizationResult::Denied
            },
            reason_code: Some(decision.reason_code),
            matched_constraints: decision.matched_constraints.clone(),
            failed_constraint: decision.failed_constraint.clone(),
            response_projection: decision.response_projection,
            occurred_at: context.occurred_at,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn route_precheck(
        context: &RequestContext,
        authentication_kind: AuthenticationKind,
        safe_credential_id: Option<Uuid>,
        actor_id: Uuid,
        role: PrimaryRole,
        action: AuthorizationAction,
        requirement_id: impl Into<String>,
        evaluated_permissions: Vec<PermissionKey>,
        required_scope: Option<SecretScope>,
        target_type: TargetType,
        allowed: bool,
    ) -> Self {
        Self {
            request_id: context.request_id,
            policy_revision: V1_POLICY_REVISION.to_owned(),
            normalized_route_id: context.route_id,
            method: context.method,
            stage: AuthorizationStage::RouteBoundary,
            authentication_kind,
            safe_credential_id,
            actor_id,
            primary_role_snapshot: role,
            action,
            declared_requirement_id: requirement_id.into(),
            evaluated_permissions,
            resolved_permission: None,
            required_scope,
            target_type,
            target_id: None,
            project_id: None,
            authorization_result: if allowed {
                AuthorizationResult::Preauthorized
            } else {
                AuthorizationResult::Denied
            },
            reason_code: (!allowed).then_some(if required_scope.is_some() {
                DecisionReason::DenyScope
            } else {
                DecisionReason::DenyRole
            }),
            matched_constraints: vec![],
            failed_constraint: None,
            response_projection: None,
            occurred_at: context.occurred_at,
        }
    }
}

pub trait AuthorizationDecisionObserver: Send + Sync + 'static {
    fn observe(&self, event: &AuthorizationEvent);

    fn observe_member_access_change(&self, _event: &MemberAccessChangeEvent) {}
}

#[derive(Debug, Default)]
pub struct TracingAuthorizationObserver;

impl AuthorizationDecisionObserver for TracingAuthorizationObserver {
    fn observe(&self, event: &AuthorizationEvent) {
        tracing::info!(
            request_id = %event.request_id,
            policy_revision = event.policy_revision,
            route_id = event.normalized_route_id,
            method = event.method,
            stage = ?event.stage,
            authentication_kind = %event.authentication_kind,
            safe_credential_id = ?event.safe_credential_id,
            actor_id = %event.actor_id,
            primary_role = event.primary_role_snapshot.as_str(),
            action = %event.action,
            requirement_id = event.declared_requirement_id,
            result = ?event.authorization_result,
            reason = ?event.reason_code,
            target_type = %event.target_type,
            target_id = ?event.target_id,
            project_id = ?event.project_id,
            "authorization decision"
        );
    }

    fn observe_member_access_change(&self, event: &MemberAccessChangeEvent) {
        tracing::info!(?event, "member access change committed");
    }
}

#[derive(Clone)]
pub struct AuthorizationService {
    observer: Arc<std::sync::RwLock<Arc<dyn AuthorizationDecisionObserver>>>,
}

impl Default for AuthorizationService {
    fn default() -> Self {
        Self::new(Arc::new(TracingAuthorizationObserver))
    }
}

impl AuthorizationService {
    pub fn new(observer: Arc<dyn AuthorizationDecisionObserver>) -> Self {
        Self {
            observer: Arc::new(std::sync::RwLock::new(observer)),
        }
    }

    pub fn set_observer(&self, observer: Arc<dyn AuthorizationDecisionObserver>) {
        *self.observer.write().expect("authorization observer lock") = observer;
    }

    pub fn evaluate(
        &self,
        context: &RequestContext,
        stage: AuthorizationStage,
        safe_credential_id: Option<Uuid>,
        input: &PolicyInput,
    ) -> PolicyDecision {
        let decision = evaluate_policy(input);
        let event = AuthorizationEvent::from_decision(
            context,
            stage,
            input.actor_id,
            safe_credential_id,
            &decision,
        );
        self.observer
            .read()
            .expect("authorization observer lock")
            .observe(&event);
        decision
    }

    pub fn observe_route_precheck(&self, event: AuthorizationEvent) {
        self.observer
            .read()
            .expect("authorization observer lock")
            .observe(&event);
    }

    pub fn observe_member_access_change(
        &self,
        context: &RequestContext,
        action: AuthorizationAction,
        change: &MemberAccessChange,
    ) {
        let event = MemberAccessChangeEvent::new(context, action, change);
        self.observer
            .read()
            .expect("authorization observer lock")
            .observe_member_access_change(&event);
    }
}

pub fn empty_target(target_type: TargetType) -> AuthorizationTargetSummary {
    AuthorizationTargetSummary {
        target_type,
        target_id: None,
        project_id: None,
        resource_kind: None,
        lifecycle: None,
        self_actor: None,
        owner_actor: None,
        effective_audience: None,
        same_project: None,
        aggregate_only: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precheck_event_serialization_has_no_credential_or_request_payload_fields() {
        let context = RequestContext::new("project.dashboard.read", &axum::http::Method::GET);
        let event = AuthorizationEvent::route_precheck(
            &context,
            AuthenticationKind::BrowserSession,
            None,
            Uuid::new_v4(),
            PrimaryRole::Contribute,
            AuthorizationAction::ProjectDashboardRead,
            "project.dashboard.read",
            vec![PermissionKey::ProjectDashboardRead],
            None,
            TargetType::Project,
            true,
        );
        let json = serde_json::to_string(&event).expect("serialize safe event");

        for forbidden in [
            "authorization_header",
            "raw_token",
            "email",
            "query_string",
            "request_body",
            "file_content",
        ] {
            assert!(!json.contains(forbidden), "unsafe field {forbidden}");
        }
    }
}
