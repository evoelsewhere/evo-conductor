//! Canonical manifest-driven authorization proof shared with TSK-020-02.
//!
//! Focused HTTP fixtures remain beside their feature. This suite proves that
//! every sealed manifest action is represented in the current role/scope
//! policy and that the generated review snapshot cannot drift silently.

mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use conductor_auth::hash_token;
use conductor_domain::{
    evaluate_policy, grants_for_role, role_has_permission, scope_is_role_compatible,
    AuthenticationKind, AuthorizationAction, AuthorizationTarget, ConductorError, ConstraintExpr,
    DeclaredRequirement, LifecycleState, PermissionAlternative, PolicyInput, PolicyRequirement,
    PrimaryRole, ResourceKind, ResponseProjection, SecretScope, SetupRequest, TargetConstraint,
    UserStatus,
};
use conductor_server::core::authorization::{AuthorizationDecisionObserver, AuthorizationEvent};
use conductor_server::http::authorization::{
    route_manifest, RouteAuthentication, RouteTargetSelector, EXPECTED_ROUTE_ACTIONS,
};
use conductor_server::route_inventory::{
    build_route_inventory, render_route_inventory, BaselineOutcome,
};
use http_body_util::BodyExt;
use serde_json::Value;
use support::test_app;
use uuid::Uuid;

const RESOURCE_KINDS: [ResourceKind; 5] = [
    ResourceKind::Agent,
    ResourceKind::Skill,
    ResourceKind::Plugin,
    ResourceKind::Workflow,
    ResourceKind::Command,
];

#[test]
fn every_declared_action_and_current_role_grant_is_covered_by_the_manifest() {
    let manifest = route_manifest();
    manifest.validate().expect("valid classified manifest");
    assert_eq!(manifest.routes.len(), EXPECTED_ROUTE_ACTIONS);

    let manifest_actions = manifest
        .routes
        .iter()
        .map(|route| route.action.as_str())
        .collect::<BTreeSet<_>>();
    let declared_actions = AuthorizationAction::ALL
        .iter()
        .map(|action| action.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(manifest_actions, declared_actions);

    for role in PrimaryRole::ALL {
        let grants = grants_for_role(role);
        assert_eq!(
            grants
                .iter()
                .map(|grant| grant.permission.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            grants.len(),
            "duplicate grant for {}",
            role.as_str()
        );
        for permission in conductor_domain::PermissionKey::ALL {
            assert_eq!(
                grants.iter().any(|grant| grant.permission == *permission),
                role_has_permission(role, *permission),
                "{} for {}",
                permission,
                role.as_str()
            );
        }
    }
}

#[test]
fn every_browser_action_resolves_each_current_role_and_eligible_alternative() {
    let manifest = route_manifest();
    let actor_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let mut browser_actions = 0usize;
    let mut role_cases = 0usize;

    for route in &manifest.routes {
        let RouteAuthentication::Browser(policy) = &route.authentication else {
            continue;
        };
        browser_actions += 1;

        for role in PrimaryRole::ALL {
            role_cases += 1;
            let grants = grants_for_role(role);
            let eligible = policy
                .alternatives
                .iter()
                .filter_map(|alternative| {
                    let grant = grants
                        .iter()
                        .find(|grant| grant.permission == alternative.permission)?;
                    satisfying_kind_and_lifecycle(
                        &alternative.selector.to_constraint(),
                        &grant.constraints,
                    )
                    .map(|_| (alternative, grant))
                })
                .collect::<Vec<_>>();

            if eligible.is_empty() {
                let input = PolicyInput {
                    actor_id,
                    actor_project_id: Some(project_id),
                    role,
                    status: UserStatus::Active,
                    authentication_kind: AuthenticationKind::BrowserSession,
                    requirement: DeclaredRequirement::Browser(policy.domain_requirement()),
                    action: route.action,
                    expected_target_type: route.target_type,
                    target: saturated_target(route.target_type, actor_id, project_id, None),
                    aggregate_only: Some(true),
                    credential_scopes: vec![],
                };
                let decision = evaluate_policy(&input);
                assert!(
                    !decision.allow,
                    "{} unexpectedly allowed {} without a declared grant",
                    route.route_id,
                    role.as_str()
                );
                assert!(decision.resolved_permission.is_none());
                continue;
            }

            for (alternative, grant) in eligible {
                let mut domain_alternative = PermissionAlternative::new(
                    alternative.permission,
                    alternative.selector.to_constraint(),
                );
                domain_alternative.response_projection = alternative.response_projection;
                let requirement = PolicyRequirement::new(route.route_id, vec![domain_alternative])
                    .expect("single route alternative");
                let target = saturated_target(
                    route.target_type,
                    actor_id,
                    project_id,
                    Some((&alternative.selector, &grant.constraints)),
                );
                let decision = evaluate_policy(&PolicyInput {
                    actor_id,
                    actor_project_id: Some(project_id),
                    role,
                    status: UserStatus::Active,
                    authentication_kind: AuthenticationKind::BrowserSession,
                    requirement: DeclaredRequirement::Browser(requirement),
                    action: route.action,
                    expected_target_type: route.target_type,
                    target,
                    aggregate_only: Some(true),
                    credential_scopes: vec![],
                });

                assert!(
                    decision.allow,
                    "{} did not resolve {} for {}: {:?}",
                    route.route_id,
                    alternative.permission,
                    role.as_str(),
                    decision.reason_code
                );
                assert_eq!(
                    decision.resolved_permission,
                    Some(alternative.permission),
                    "{} for {}",
                    route.route_id,
                    role.as_str()
                );
                assert_eq!(
                    decision.response_projection,
                    alternative
                        .response_projection
                        .or(grant.response_projection),
                    "projection for {} / {} / {}",
                    route.route_id,
                    role.as_str(),
                    alternative.permission
                );
            }
        }
    }

    assert_eq!(browser_actions, 77);
    assert_eq!(role_cases, browser_actions * PrimaryRole::ALL.len());
}

#[test]
fn every_connection_action_requires_its_exact_scope_for_all_current_roles() {
    let manifest = route_manifest();
    let actor_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let mut scope_counts = BTreeMap::new();
    let mut role_cases = 0usize;

    for route in &manifest.routes {
        let RouteAuthentication::Connection(policy) = &route.authentication else {
            continue;
        };
        *scope_counts
            .entry(policy.required_scope.as_str())
            .or_insert(0usize) += 1;

        for role in PrimaryRole::ALL {
            role_cases += 1;
            assert!(
                scope_is_role_compatible(role, policy.required_scope),
                "V1 route {} is incompatible with {}",
                route.route_id,
                role.as_str()
            );
            let target = saturated_target(route.target_type, actor_id, project_id, None);
            let input = |credential_scopes| PolicyInput {
                actor_id,
                actor_project_id: Some(project_id),
                role,
                status: UserStatus::Active,
                authentication_kind: AuthenticationKind::ConnectionToken,
                requirement: DeclaredRequirement::Connection(policy.domain_requirement()),
                action: route.action,
                expected_target_type: route.target_type,
                target: target.clone(),
                aggregate_only: Some(true),
                credential_scopes,
            };

            let allowed = evaluate_policy(&input(vec![policy.required_scope]));
            assert!(
                allowed.allow,
                "{} rejected exact scope {} for {}",
                route.route_id,
                policy.required_scope.as_str(),
                role.as_str()
            );
            assert_eq!(allowed.required_scope, Some(policy.required_scope));

            for wrong_scope in SecretScope::ALL
                .into_iter()
                .filter(|scope| *scope != policy.required_scope)
            {
                let denied = evaluate_policy(&input(vec![wrong_scope]));
                assert!(
                    !denied.allow,
                    "{} accepted wrong scope {}",
                    route.route_id,
                    wrong_scope.as_str()
                );
                assert!(denied.resolved_permission.is_none());
            }
            assert!(!evaluate_policy(&input(vec![])).allow);
        }
    }

    assert_eq!(
        scope_counts,
        BTreeMap::from([
            ("report_telemetry", 2),
            ("subscribe_resources", 8),
            ("sync_inventory", 1),
        ])
    );
    assert_eq!(role_cases, 11 * PrimaryRole::ALL.len());
}

#[test]
fn public_bootstrap_and_protected_classes_are_explicit_and_exhaustive() {
    let manifest = route_manifest();
    let mut classes = BTreeMap::new();
    for route in &manifest.routes {
        let class = match &route.authentication {
            RouteAuthentication::ExplicitPublic => "public",
            RouteAuthentication::Bootstrap => "bootstrap",
            RouteAuthentication::Browser(policy) => {
                assert_eq!(policy.requirement_id, route.route_id);
                assert!(!policy.alternatives.is_empty());
                "browser"
            }
            RouteAuthentication::Connection(policy) => {
                assert_eq!(policy.requirement_id, route.route_id);
                "connection"
            }
        };
        *classes.entry(class).or_insert(0usize) += 1;
    }
    assert_eq!(
        classes,
        BTreeMap::from([
            ("bootstrap", 1),
            ("browser", 77),
            ("connection", 11),
            ("public", 6),
        ])
    );
}

#[test]
fn generated_inventory_is_current_and_never_marks_a_target_case_as_unconditional() {
    let rendered = render_route_inventory().expect("render route inventory");
    let snapshot_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("docs/generated/req-004-route-inventory.json");
    let snapshot = std::fs::read_to_string(&snapshot_path).unwrap_or_else(|error| {
        panic!(
            "read checked route inventory {}: {error}",
            snapshot_path.display()
        )
    });
    assert_eq!(rendered, snapshot, "generated route inventory drifted");

    let inventory = build_route_inventory().expect("build route inventory");
    assert_eq!(inventory.route_count, EXPECTED_ROUTE_ACTIONS);
    let projections = |route_id: &str, role: PrimaryRole| {
        let route = inventory
            .routes
            .iter()
            .find(|route| route.route_id == route_id)
            .unwrap_or_else(|| panic!("missing inventory route {route_id}"));
        match role {
            PrimaryRole::Admin => &route.role_baselines.admin.possible_response_projections,
            PrimaryRole::Contribute => {
                &route
                    .role_baselines
                    .contribute
                    .possible_response_projections
            }
            PrimaryRole::User => &route.role_baselines.user.possible_response_projections,
        }
    };
    assert_eq!(
        projections("project.dashboard.read", PrimaryRole::Admin),
        &[ResponseProjection::Full]
    );
    assert_eq!(
        projections("project.dashboard.read", PrimaryRole::Contribute),
        &[ResponseProjection::AggregateOnly]
    );
    assert_eq!(
        projections("member.directory.list", PrimaryRole::Contribute),
        &[ResponseProjection::DirectorySafe]
    );
    for route_id in [
        "resource.monitoring.read",
        "resource.inventory_monitoring.read",
    ] {
        assert_eq!(
            projections(route_id, PrimaryRole::Admin),
            &[ResponseProjection::Full]
        );
        assert_eq!(
            projections(route_id, PrimaryRole::Contribute),
            &[ResponseProjection::AggregateOnly]
        );
    }

    for route in &inventory.routes {
        for (role, baseline) in [
            ("admin", &route.role_baselines.admin),
            ("contribute", &route.role_baselines.contribute),
            ("user", &route.role_baselines.user),
        ] {
            assert!(
                !(baseline.target_dependent && baseline.outcome == BaselineOutcome::Allow),
                "{} falsely claims unconditional allow for {role}",
                route.route_id
            );
            assert_eq!(
                baseline.outcome == BaselineOutcome::Conditional,
                baseline.target_dependent,
                "conditional marker for {} / {role}",
                route.route_id
            );
        }
    }

    let parsed: Value = serde_json::from_str(&snapshot).expect("snapshot JSON");
    assert_eq!(parsed["route_count"], EXPECTED_ROUTE_ACTIONS);
    assert_eq!(
        parsed["routes"].as_array().map(Vec::len),
        Some(EXPECTED_ROUTE_ACTIONS)
    );
}

#[tokio::test]
async fn representative_http_error_taxonomy_is_exact() {
    for (error, expected_status, expected_code) in [
        (
            conductor_server::ApiError::from(ConductorError::Unauthorized),
            StatusCode::UNAUTHORIZED,
            "unauthorized",
        ),
        (
            conductor_server::ApiError::from(ConductorError::Forbidden),
            StatusCode::FORBIDDEN,
            "permission_denied",
        ),
        (
            conductor_server::ApiError::scope_denied(),
            StatusCode::FORBIDDEN,
            "scope_denied",
        ),
        (
            conductor_server::ApiError::from(ConductorError::NotFound("resource".into())),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
    ] {
        let response = error.into_response();
        assert_eq!(response.status(), expected_status);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("collect error body")
            .to_bytes();
        let body: Value = serde_json::from_slice(&bytes).expect("error JSON");
        assert_eq!(body["code"], expected_status.as_u16());
        assert_eq!(body["error_code"], expected_code);
    }

    let app = test_app().await;
    let (status, body) = app.get("/api/dashboard", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error_code"], "unauthorized");
    assert!(body["request_id"].as_str().is_some());

    let user = app.seed_user(PrimaryRole::User).await;
    let user_token = app.token_for(&user).await;
    let (status, body) = app.get("/api/dashboard", Some(&user_token)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error_code"], "permission_denied");

    const WRONG_SCOPE_TOKEN: &str = "evc_SCOPE_DENIAL_CANARY_never_serialize";
    app.state
        .db
        .secrets()
        .insert(
            user.id,
            "wrong scope proof",
            "evc_scope",
            &hash_token(WRONG_SCOPE_TOKEN),
            &[SecretScope::ReportTelemetry],
            None,
        )
        .await
        .expect("seed wrong-scope credential");
    let (status, body) = app
        .get("/api/v1/subscribe/resources", Some(WRONG_SCOPE_TOKEN))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error_code"], "scope_denied");
    assert!(!body.to_string().contains(WRONG_SCOPE_TOKEN));

    let contributor = app.seed_user(PrimaryRole::Contribute).await;
    let contributor_token = app.token_for(&contributor).await;
    let (status, body) = app
        .get(
            &format!("/api/resources/{}/access", Uuid::new_v4()),
            Some(&contributor_token),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error_code"], "not_found");

    let conflict_app = test_app().await;
    let (_, admin) = conflict_app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "status-taxonomy-conflict".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "status-taxonomy-admin@example.test".into(),
                admin_display_name: "Status taxonomy admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure status taxonomy project");
    let admin_token = conflict_app.token_for(&admin).await;
    let (status, body) = conflict_app
        .patch(
            &format!("/api/members/{}", admin.id),
            Some(&admin_token),
            serde_json::json!({ "primary_role": "user" }),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error_code"], "self_role_change");
}

#[derive(Default)]
struct RecordingObserver(Mutex<Vec<AuthorizationEvent>>);

impl AuthorizationDecisionObserver for RecordingObserver {
    fn observe(&self, event: &AuthorizationEvent) {
        self.0.lock().expect("observer lock").push(event.clone());
    }
}

#[tokio::test]
async fn decision_observer_excludes_secret_header_query_and_identity_canaries() {
    const RAW_TOKEN_CANARY: &str = "evc_RAW_TOKEN_CANARY_never_serialize";
    const TOKEN_NAME_CANARY: &str = "TOKEN_NAME_CANARY_never_serialize";
    const QUERY_CANARY: &str = "QUERY_CANARY_never_serialize";
    const HEADER_CANARY: &str = "HEADER_CANARY_never_serialize";

    let app = test_app().await;
    let observer = std::sync::Arc::new(RecordingObserver::default());
    app.state.authorization.set_observer(observer.clone());
    let (_, member) = app
        .state
        .db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "decision-redaction-proof".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "IDENTITY_CANARY_never_serialize@example.test".into(),
                admin_display_name: "Decision proof".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            "unused-test-password-hash",
            "unused-test-jwt-secret",
            None,
        )
        .await
        .expect("configure redaction proof project");
    let secret = app
        .state
        .db
        .secrets()
        .insert(
            member.id,
            TOKEN_NAME_CANARY,
            "evc_canary",
            &hash_token(RAW_TOKEN_CANARY),
            &[SecretScope::SubscribeResources],
            None,
        )
        .await
        .expect("seed canary connection token");
    let mut headers = HeaderMap::new();
    headers.insert("x-private-canary", HeaderValue::from_static(HEADER_CANARY));

    let (status, _, _) = app
        .get_bytes_with_headers(
            &format!("/api/v1/subscribe/resources?probe={QUERY_CANARY}"),
            Some(RAW_TOKEN_CANARY),
            headers,
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    let events = observer.0.lock().expect("observer lock");
    assert!(!events.is_empty(), "authorized request emitted no decision");
    assert!(events
        .iter()
        .any(|event| event.safe_credential_id == Some(secret.id)));
    let serialized = serde_json::to_string(&*events).expect("serialize decisions");
    for forbidden in [
        RAW_TOKEN_CANARY,
        TOKEN_NAME_CANARY,
        QUERY_CANARY,
        HEADER_CANARY,
        member.email.as_str(),
    ] {
        assert!(
            !serialized.contains(forbidden),
            "decision leaked canary {forbidden}"
        );
    }
}

fn saturated_target(
    target_type: conductor_domain::TargetType,
    actor_id: Uuid,
    project_id: Uuid,
    conditions: Option<(&RouteTargetSelector, &ConstraintExpr)>,
) -> AuthorizationTarget {
    let (resource_kind, lifecycle) = conditions
        .and_then(|(selector, grant)| {
            satisfying_kind_and_lifecycle(&selector.to_constraint(), grant)
        })
        .unwrap_or((ResourceKind::Agent, LifecycleState::Published));

    AuthorizationTarget {
        project_id: Some(project_id),
        target_type,
        target_id: Some(actor_id),
        owner_id: Some(actor_id),
        resource_kind: Some(resource_kind),
        lifecycle: Some(lifecycle),
        effective_audience: Some(true),
    }
}

fn satisfying_kind_and_lifecycle(
    left: &ConstraintExpr,
    right: &ConstraintExpr,
) -> Option<(ResourceKind, LifecycleState)> {
    const LIFECYCLES: [LifecycleState; 5] = [
        LifecycleState::Draft,
        LifecycleState::Beta,
        LifecycleState::Published,
        LifecycleState::Archived,
        LifecycleState::Deprecated,
    ];
    RESOURCE_KINDS.into_iter().find_map(|kind| {
        LIFECYCLES
            .into_iter()
            .find(|lifecycle| {
                constraint_matches(left, kind, *lifecycle)
                    && constraint_matches(right, kind, *lifecycle)
            })
            .map(|lifecycle| (kind, lifecycle))
    })
}

fn constraint_matches(
    expression: &ConstraintExpr,
    kind: ResourceKind,
    lifecycle: LifecycleState,
) -> bool {
    match expression {
        ConstraintExpr::Atom(TargetConstraint::ResourceKindIn { values }) => values.contains(&kind),
        ConstraintExpr::Atom(TargetConstraint::LifecycleIn { values }) => {
            values.contains(&lifecycle)
        }
        ConstraintExpr::Atom(
            TargetConstraint::Any
            | TargetConstraint::SelfActor
            | TargetConstraint::OwnerActor
            | TargetConstraint::EffectiveAudience
            | TargetConstraint::SameProject
            | TargetConstraint::AggregateOnly,
        ) => true,
        ConstraintExpr::AllOf(items) => items
            .iter()
            .all(|item| constraint_matches(item, kind, lifecycle)),
        ConstraintExpr::AnyOf(items) => items
            .iter()
            .any(|item| constraint_matches(item, kind, lifecycle)),
    }
}
