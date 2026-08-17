use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use conductor_domain::{
    grants_for_role, role_has_permission, scope_is_role_compatible, AuthenticationKind,
    AuthorizationTarget, ConductorError, ConstraintExpr, DeclaredRequirement, PolicyDecision,
    PolicyInput, TargetConstraint, TargetType,
};
use uuid::Uuid;

use crate::core::authorization::{AuthorizationEvent, AuthorizationStage};
use crate::core::error::{ApiError, ApiResult};
use crate::core::request_context::{scope as request_scope, RequestContext};
use crate::core::state::AppState;
use crate::http::extractors::{
    authenticate_browser_user, authenticate_connection_principal, connection_principal_scope,
    mark_connection_secret_used_if_due, AuthUser, ConnectionPrincipal,
};

use super::catalog::{
    BrowserRoutePolicy, ConnectionRoutePolicy, RouteAuthentication, RouteSpec, RouteTargetSelector,
};

const X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");

#[derive(Clone)]
pub(crate) struct BoundaryState {
    pub app: AppState,
    pub spec: RouteSpec,
}

/// Typed route declaration made available to target-aware handlers.
///
/// Target-heavy handlers load authoritative facts and call one of the
/// `authorize_*_target` methods; they cannot substitute an action or
/// requirement different from the one selected by the route catalog.
#[derive(Debug, Clone)]
pub struct RouteAuthorization {
    context: RequestContext,
    spec: RouteSpec,
    target_required: bool,
    target_evaluated: Arc<AtomicBool>,
}

impl RouteAuthorization {
    pub fn request_context(&self) -> &RequestContext {
        &self.context
    }

    pub fn route_spec(&self) -> &RouteSpec {
        &self.spec
    }

    fn mark_target_evaluated(&self) {
        self.target_evaluated.store(true, Ordering::Release);
    }

    fn requires_unresolved_target(&self) -> bool {
        self.target_required && !self.target_evaluated.load(Ordering::Acquire)
    }

    pub async fn authorize_browser_target(
        &self,
        state: &AppState,
        actor: &conductor_domain::User,
        target: AuthorizationTarget,
    ) -> ApiResult<PolicyDecision> {
        let aggregate_only = match &self.spec.authentication {
            RouteAuthentication::Browser(policy) => policy
                .alternatives
                .iter()
                .any(|alternative| selector_contains_aggregate(&alternative.selector))
                .then_some(true),
            _ => None,
        };
        self.authorize_browser_target_with_facts(state, actor, target, aggregate_only)
            .await
    }

    pub async fn authorize_browser_target_with_aggregate_fact(
        &self,
        state: &AppState,
        actor: &conductor_domain::User,
        target: AuthorizationTarget,
        aggregate_only: bool,
    ) -> ApiResult<PolicyDecision> {
        self.authorize_browser_target_with_facts(state, actor, target, Some(aggregate_only))
            .await
    }

    async fn authorize_browser_target_with_facts(
        &self,
        state: &AppState,
        actor: &conductor_domain::User,
        target: AuthorizationTarget,
        aggregate_only: Option<bool>,
    ) -> ApiResult<PolicyDecision> {
        let RouteAuthentication::Browser(policy) = &self.spec.authentication else {
            return Err(ConductorError::Forbidden.into());
        };
        let actor_project_id = state
            .db
            .instance()
            .authorization_project_id()
            .await?
            .ok_or(ConductorError::SetupRequired)
            .map(Some)?;
        let input = PolicyInput {
            actor_id: actor.id,
            actor_project_id,
            role: actor.primary_role,
            status: actor.status,
            authentication_kind: AuthenticationKind::BrowserSession,
            requirement: DeclaredRequirement::Browser(policy.domain_requirement()),
            action: self.spec.action,
            expected_target_type: self.spec.target_type,
            target,
            aggregate_only,
            credential_scopes: vec![],
        };
        self.mark_target_evaluated();
        let decision =
            state
                .authorization
                .evaluate(&self.context, AuthorizationStage::Target, None, &input);
        decision_result(decision, &self.spec)
    }

    pub async fn authorize_connection_target(
        &self,
        state: &AppState,
        principal: &ConnectionPrincipal,
        target: AuthorizationTarget,
    ) -> ApiResult<PolicyDecision> {
        let RouteAuthentication::Connection(policy) = &self.spec.authentication else {
            return Err(ConductorError::Forbidden.into());
        };
        let actor_project_id = state
            .db
            .instance()
            .authorization_project_id()
            .await?
            .ok_or(ConductorError::SetupRequired)
            .map(Some)?;
        let input = PolicyInput {
            actor_id: principal.user.id,
            actor_project_id,
            role: principal.user.primary_role,
            status: principal.user.status,
            authentication_kind: AuthenticationKind::ConnectionToken,
            requirement: DeclaredRequirement::Connection(policy.domain_requirement()),
            action: self.spec.action,
            expected_target_type: self.spec.target_type,
            target,
            aggregate_only: selector_contains_aggregate(&policy.selector).then_some(true),
            credential_scopes: principal.secret.scopes.clone(),
        };
        self.mark_target_evaluated();
        let decision = state.authorization.evaluate(
            &self.context,
            AuthorizationStage::Target,
            Some(principal.secret.id),
            &input,
        );
        decision_result(decision, &self.spec)
    }
}

pub async fn authorize_current_browser_target(
    state: &AppState,
    route: &RouteAuthorization,
    actor: &conductor_domain::User,
    target: AuthorizationTarget,
) -> ApiResult<PolicyDecision> {
    route.authorize_browser_target(state, actor, target).await
}

pub async fn authorize_current_browser_target_with_aggregate_fact(
    state: &AppState,
    route: &RouteAuthorization,
    actor: &conductor_domain::User,
    target: AuthorizationTarget,
    aggregate_only: bool,
) -> ApiResult<PolicyDecision> {
    route
        .authorize_browser_target_with_aggregate_fact(state, actor, target, aggregate_only)
        .await
}

pub async fn authorize_current_connection_target(
    state: &AppState,
    route: &RouteAuthorization,
    principal: &ConnectionPrincipal,
    target: AuthorizationTarget,
) -> ApiResult<PolicyDecision> {
    route
        .authorize_connection_target(state, principal, target)
        .await
}

pub(crate) async fn enforce_route_policy(
    State(boundary): State<BoundaryState>,
    mut request: Request,
    next: Next,
) -> Response {
    let context = RequestContext::new(boundary.spec.route_id, request.method());
    request.extensions_mut().insert(context.clone());
    let route_authorization = RouteAuthorization {
        context: context.clone(),
        spec: boundary.spec.clone(),
        target_required: route_requires_target_evaluation(&boundary.spec),
        target_evaluated: Arc::new(AtomicBool::new(false)),
    };
    request.extensions_mut().insert(route_authorization.clone());

    let request_id = context.request_id;
    let completion_guard = route_authorization.clone();
    let response = request_scope(context.clone(), async move {
        let authentication = boundary.spec.authentication.clone();
        let response = match authentication {
            RouteAuthentication::ExplicitPublic => next.run(request).await,
            RouteAuthentication::Bootstrap => {
                match boundary.app.db.instance().is_setup_completed().await {
                    Ok(false) => next.run(request).await,
                    Ok(true) => {
                        ApiError::from(ConductorError::SetupAlreadyCompleted).into_response()
                    }
                    Err(error) => ApiError::from(error).into_response(),
                }
            }
            RouteAuthentication::Browser(policy) => {
                authorize_browser(
                    boundary,
                    context,
                    request,
                    next,
                    policy,
                    route_authorization,
                )
                .await
            }
            RouteAuthentication::Connection(policy) => {
                authorize_connection(
                    boundary,
                    context,
                    request,
                    next,
                    policy,
                    route_authorization,
                )
                .await
            }
        };

        enforce_target_completion(response, &completion_guard)
    })
    .await;

    with_request_id(response, request_id)
}

fn enforce_target_completion(
    response: Response,
    completion_guard: &RouteAuthorization,
) -> Response {
    if (response.status().is_success() || response.status() == StatusCode::NOT_MODIFIED)
        && completion_guard.requires_unresolved_target()
    {
        tracing::error!(
            request_id = %completion_guard.context.request_id,
            route_id = completion_guard.spec.route_id,
            "successful target-aware route completed without a policy decision"
        );
        ApiError::from(ConductorError::Internal).into_response()
    } else {
        response
    }
}

async fn authorize_browser(
    boundary: BoundaryState,
    context: RequestContext,
    mut request: Request,
    next: Next,
    policy: BrowserRoutePolicy,
    route_authorization: RouteAuthorization,
) -> Response {
    let actor = match authenticate_browser_user(&boundary.app, request.headers()).await {
        Ok(actor) => actor,
        Err(error) => return error.into_response(),
    };

    let evaluated_permissions = policy
        .alternatives
        .iter()
        .map(|alternative| alternative.permission)
        .collect::<Vec<_>>();
    let role_allowed = evaluated_permissions
        .iter()
        .any(|permission| role_has_permission(actor.primary_role, *permission));
    if !role_allowed {
        boundary
            .app
            .authorization
            .observe_route_precheck(AuthorizationEvent::route_precheck(
                &context,
                AuthenticationKind::BrowserSession,
                None,
                actor.id,
                actor.primary_role,
                boundary.spec.action,
                policy.requirement_id,
                evaluated_permissions,
                None,
                boundary.spec.target_type,
                false,
            ));
        return ApiError::from(ConductorError::Forbidden).into_response();
    }

    if let Some(alternative) = policy.alternatives.iter().find(|alternative| {
        role_has_permission(actor.primary_role, alternative.permission)
            && alternative.selector.can_resolve_at_route_boundary()
    }) {
        let project_id = match boundary.app.db.instance().authorization_project_id().await {
            Ok(project_id) => project_id,
            Err(error) => return ApiError::from(error).into_response(),
        };
        let grant_requires_project = grants_for_role(actor.primary_role)
            .into_iter()
            .find(|grant| grant.permission == alternative.permission)
            .is_some_and(|grant| constraint_requires_project(&grant.constraints));
        if project_id.is_none()
            && (constraint_requires_project(&alternative.selector.to_constraint())
                || grant_requires_project)
        {
            return ApiError::from(ConductorError::SetupRequired).into_response();
        }
        let target = synthetic_target(
            boundary.spec.target_type,
            &alternative.selector,
            actor.id,
            project_id,
        );
        let input = PolicyInput {
            actor_id: actor.id,
            actor_project_id: project_id,
            role: actor.primary_role,
            status: actor.status,
            authentication_kind: AuthenticationKind::BrowserSession,
            requirement: DeclaredRequirement::Browser(policy.domain_requirement()),
            action: boundary.spec.action,
            expected_target_type: boundary.spec.target_type,
            target,
            aggregate_only: selector_contains_aggregate(&alternative.selector).then_some(true),
            credential_scopes: vec![],
        };
        let decision = boundary.app.authorization.evaluate(
            &context,
            AuthorizationStage::RouteBoundary,
            None,
            &input,
        );
        route_authorization.mark_target_evaluated();
        if !decision.allow {
            return decision_error(&decision, &boundary.spec).into_response();
        }
    } else {
        boundary
            .app
            .authorization
            .observe_route_precheck(AuthorizationEvent::route_precheck(
                &context,
                AuthenticationKind::BrowserSession,
                None,
                actor.id,
                actor.primary_role,
                boundary.spec.action,
                policy.requirement_id,
                evaluated_permissions,
                None,
                boundary.spec.target_type,
                true,
            ));
    }

    request.extensions_mut().insert(AuthUser(actor));
    next.run(request).await
}

async fn authorize_connection(
    boundary: BoundaryState,
    context: RequestContext,
    mut request: Request,
    next: Next,
    policy: ConnectionRoutePolicy,
    route_authorization: RouteAuthorization,
) -> Response {
    let principal = match authenticate_connection_principal(&boundary.app, request.headers()).await
    {
        Ok(principal) => principal,
        Err(error) => return error.into_response(),
    };

    let scope_allowed = principal.secret.scopes.contains(&policy.required_scope)
        && scope_is_role_compatible(principal.user.primary_role, policy.required_scope);

    if !scope_allowed {
        boundary
            .app
            .authorization
            .observe_route_precheck(AuthorizationEvent::route_precheck(
                &context,
                AuthenticationKind::ConnectionToken,
                Some(principal.secret.id),
                principal.user.id,
                principal.user.primary_role,
                boundary.spec.action,
                policy.requirement_id,
                vec![],
                Some(policy.required_scope),
                boundary.spec.target_type,
                false,
            ));
        return ApiError::scope_denied().into_response();
    }

    if policy.selector.can_resolve_at_route_boundary() {
        let project_id = match boundary.app.db.instance().authorization_project_id().await {
            Ok(project_id) => project_id,
            Err(error) => return ApiError::from(error).into_response(),
        };
        if project_id.is_none() && constraint_requires_project(&policy.selector.to_constraint()) {
            return ApiError::from(ConductorError::SetupRequired).into_response();
        }
        let target = synthetic_target(
            boundary.spec.target_type,
            &policy.selector,
            principal.user.id,
            project_id,
        );
        let input = PolicyInput {
            actor_id: principal.user.id,
            actor_project_id: project_id,
            role: principal.user.primary_role,
            status: principal.user.status,
            authentication_kind: AuthenticationKind::ConnectionToken,
            requirement: DeclaredRequirement::Connection(policy.domain_requirement()),
            action: boundary.spec.action,
            expected_target_type: boundary.spec.target_type,
            target,
            aggregate_only: selector_contains_aggregate(&policy.selector).then_some(true),
            credential_scopes: principal.secret.scopes.clone(),
        };
        let decision = boundary.app.authorization.evaluate(
            &context,
            AuthorizationStage::RouteBoundary,
            Some(principal.secret.id),
            &input,
        );
        route_authorization.mark_target_evaluated();
        if !decision.allow {
            return decision_error(&decision, &boundary.spec).into_response();
        }
    } else {
        boundary
            .app
            .authorization
            .observe_route_precheck(AuthorizationEvent::route_precheck(
                &context,
                AuthenticationKind::ConnectionToken,
                Some(principal.secret.id),
                principal.user.id,
                principal.user.primary_role,
                boundary.spec.action,
                policy.requirement_id,
                vec![],
                Some(policy.required_scope),
                boundary.spec.target_type,
                true,
            ));
    }

    request.extensions_mut().insert(principal.clone());
    let response = connection_principal_scope(principal.clone(), next.run(request)).await;
    if response.status().is_success() || response.status() == StatusCode::NOT_MODIFIED {
        if let Err(error) = mark_connection_secret_used_if_due(&boundary.app, &principal).await {
            tracing::error!(
                request_id = %context.request_id,
                credential_id = %principal.secret.id,
                error = %error.error,
                "could not update authorized connection-token usage metadata"
            );
        }
    }
    response
}

fn route_requires_target_evaluation(spec: &RouteSpec) -> bool {
    match &spec.authentication {
        RouteAuthentication::Browser(policy) => policy
            .alternatives
            .iter()
            .any(|alternative| !alternative.selector.can_resolve_at_route_boundary()),
        RouteAuthentication::Connection(policy) => !policy.selector.can_resolve_at_route_boundary(),
        RouteAuthentication::ExplicitPublic | RouteAuthentication::Bootstrap => false,
    }
}

fn selector_contains_aggregate(selector: &RouteTargetSelector) -> bool {
    match selector {
        RouteTargetSelector::AggregateOnly => true,
        RouteTargetSelector::AllOf(items) | RouteTargetSelector::AnyOf(items) => {
            items.iter().any(selector_contains_aggregate)
        }
        _ => false,
    }
}

fn constraint_requires_project(constraint: &ConstraintExpr) -> bool {
    match constraint {
        ConstraintExpr::Atom(TargetConstraint::SameProject) => true,
        ConstraintExpr::AllOf(items) | ConstraintExpr::AnyOf(items) => {
            items.iter().any(constraint_requires_project)
        }
        _ => false,
    }
}

fn synthetic_target(
    target_type: TargetType,
    selector: &RouteTargetSelector,
    actor_id: Uuid,
    project_id: Option<Uuid>,
) -> AuthorizationTarget {
    let mut target = AuthorizationTarget {
        project_id,
        target_type,
        target_id: None,
        owner_id: None,
        resource_kind: None,
        lifecycle: None,
        effective_audience: None,
    };
    apply_synthetic_target(&mut target, selector, actor_id);
    target
}

fn apply_synthetic_target(
    target: &mut AuthorizationTarget,
    selector: &RouteTargetSelector,
    actor_id: Uuid,
) {
    match selector {
        RouteTargetSelector::SelfActor => {
            target.target_id = Some(actor_id);
            target.owner_id = Some(actor_id);
        }
        RouteTargetSelector::NewResourceOwnerActor
        | RouteTargetSelector::NewInstallationOwnerActor => target.owner_id = Some(actor_id),
        RouteTargetSelector::KindPlugin => {
            target.resource_kind = Some(conductor_domain::ResourceKind::Plugin)
        }
        RouteTargetSelector::AllOf(items) | RouteTargetSelector::AnyOf(items) => {
            for item in items {
                apply_synthetic_target(target, item, actor_id);
            }
        }
        RouteTargetSelector::ProjectMember
        | RouteTargetSelector::AggregateOnly
        | RouteTargetSelector::CurrentScopePolicy => {}
        _ => {}
    }
}

fn decision_result(decision: PolicyDecision, spec: &RouteSpec) -> ApiResult<PolicyDecision> {
    if decision.allow {
        Ok(decision)
    } else {
        Err(decision_error(&decision, spec))
    }
}

fn decision_error(decision: &PolicyDecision, spec: &RouteSpec) -> ApiError {
    if matches!(
        decision.reason_code,
        conductor_domain::DecisionReason::DenyCrossProject
            | conductor_domain::DecisionReason::DenyOutsideAudience
    ) || spec_hides_target(spec)
    {
        ConductorError::NotFound(safe_target_name(spec.target_type)).into()
    } else if decision.reason_code == conductor_domain::DecisionReason::DenyScope {
        ApiError::scope_denied()
    } else {
        ConductorError::Forbidden.into()
    }
}

fn spec_hides_target(spec: &RouteSpec) -> bool {
    fn selector_hides(selector: &RouteTargetSelector) -> bool {
        match selector {
            RouteTargetSelector::EffectiveAudienceList
            | RouteTargetSelector::EffectiveVersionPath
            | RouteTargetSelector::VisibleResourcePath
            | RouteTargetSelector::VisibleAnalyticsViewPath
            | RouteTargetSelector::InventoryItemsVisibleBody => true,
            RouteTargetSelector::AllOf(items) | RouteTargetSelector::AnyOf(items) => {
                items.iter().any(selector_hides)
            }
            _ => false,
        }
    }

    match &spec.authentication {
        RouteAuthentication::Browser(policy) => policy
            .alternatives
            .iter()
            .any(|alternative| selector_hides(&alternative.selector)),
        RouteAuthentication::Connection(policy) => selector_hides(&policy.selector),
        RouteAuthentication::ExplicitPublic | RouteAuthentication::Bootstrap => false,
    }
}

fn safe_target_name(target_type: TargetType) -> String {
    match target_type {
        TargetType::Member => "member",
        TargetType::Resource => "resource",
        TargetType::AnalyticsView => "analytics view",
        TargetType::ConnectionToken => "connection token",
        TargetType::ClientInstallation => "client installation",
        TargetType::Project => "project",
        TargetType::Session => "session",
        TargetType::Taxonomy => "taxonomy",
        TargetType::Audit => "audit",
    }
    .to_owned()
}

fn with_request_id(mut response: Response, request_id: Uuid) -> Response {
    let value =
        HeaderValue::from_str(&request_id.to_string()).expect("UUID is a valid header value");
    response.headers_mut().insert(X_REQUEST_ID, value);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Method;
    use conductor_domain::{AuthorizationAction, PermissionKey};

    fn route_authorization(target_required: bool) -> RouteAuthorization {
        let spec = RouteSpec::new(
            super::super::catalog::RouteMethod::Patch,
            "/resources/{id}",
            super::super::catalog::RouteDefinition::browser(
                AuthorizationAction::ResourceUpdate,
                TargetType::Resource,
                PermissionKey::ResourceAuthor,
                RouteTargetSelector::InProjectResourcePath,
            ),
        );
        RouteAuthorization {
            context: RequestContext::new(spec.route_id, &Method::PATCH),
            spec,
            target_required,
            target_evaluated: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn unresolved_target_success_is_rejected_but_completed_or_static_success_is_preserved() {
        let unresolved = route_authorization(true);
        let response = enforce_target_completion(StatusCode::OK.into_response(), &unresolved);
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let evaluated = route_authorization(true);
        evaluated.mark_target_evaluated();
        let response = enforce_target_completion(StatusCode::OK.into_response(), &evaluated);
        assert_eq!(response.status(), StatusCode::OK);

        let static_route = route_authorization(false);
        let response = enforce_target_completion(StatusCode::OK.into_response(), &static_route);
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn synthetic_boundary_target_uses_the_authoritative_project_fact() {
        let project_id = Uuid::new_v4();
        let target = synthetic_target(
            TargetType::Project,
            &RouteTargetSelector::ProjectMember,
            Uuid::new_v4(),
            Some(project_id),
        );
        assert_eq!(target.project_id, Some(project_id));
        assert_ne!(target.project_id, Some(Uuid::nil()));
    }
}
