//! Deterministic, reviewable projection of the sealed API route manifest.
//!
//! The route manifest remains the only method/path/policy registry. This module
//! derives the checked-in REQ-004 review artifact and role baselines from that
//! manifest plus the current domain permission bundles.

use anyhow::Context;
use conductor_domain::{
    evaluate_policy, grants_for_role, scope_is_role_compatible, AuthenticationKind,
    AuthorizationTarget, ConstraintExpr, DeclaredRequirement, LifecycleState, PermissionGrant,
    PermissionKey, PolicyInput, PrimaryRole, ResourceKind, ResponseProjection, SecretScope,
    TargetConstraint, UserStatus,
};
use serde::Serialize;
use uuid::Uuid;

use crate::http::authorization::{
    route_manifest, RouteAuthentication, RouteSpec, RouteTargetSelector,
};

pub const GENERATED_ROUTE_INVENTORY_PATH: &str = "docs/generated/req-004-route-inventory.json";

const RESOURCE_KINDS: [ResourceKind; 5] = [
    ResourceKind::Agent,
    ResourceKind::Skill,
    ResourceKind::Plugin,
    ResourceKind::Workflow,
    ResourceKind::Command,
];
const LIFECYCLES: [LifecycleState; 5] = [
    LifecycleState::Draft,
    LifecycleState::Beta,
    LifecycleState::Published,
    LifecycleState::Archived,
    LifecycleState::Deprecated,
];

#[derive(Debug, Clone, Serialize)]
pub struct RouteInventory {
    pub schema_version: u8,
    pub policy_revision: &'static str,
    pub generated_from: &'static str,
    pub route_count: usize,
    pub baseline_assumptions: Vec<&'static str>,
    pub role_permission_bundles: Vec<RolePermissionBundle>,
    pub routes: Vec<RouteInventoryEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RolePermissionBundle {
    pub role: PrimaryRole,
    pub grants: Vec<PermissionGrant>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteInventoryEntry {
    pub route_id: &'static str,
    pub action: conductor_domain::AuthorizationAction,
    pub method: &'static str,
    pub path: String,
    pub target_type: conductor_domain::TargetType,
    pub authentication: RouteAuthentication,
    pub body_policy: BodyPolicy,
    pub role_baselines: RoleBaselines,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BodyPolicy {
    Default,
    MaxBytes { bytes: usize },
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleBaselines {
    pub admin: RoleBaseline,
    pub contribute: RoleBaseline,
    pub user: RoleBaseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineOutcome {
    Allow,
    Deny,
    Conditional,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoleBaseline {
    pub outcome: BaselineOutcome,
    pub target_dependent: bool,
    pub possible_response_projections: Vec<ResponseProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_scope: Option<SecretScope>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub feasible_alternatives: Vec<FeasibleBrowserAlternative>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeasibleBrowserAlternative {
    pub permission: PermissionKey,
    pub role_grant_constraints: ConstraintExpr,
    pub route_selector: RouteTargetSelector,
    pub target_dependent: bool,
    pub response_projection: ResponseProjection,
}

pub fn build_route_inventory() -> anyhow::Result<RouteInventory> {
    let manifest = route_manifest();
    manifest
        .validate()
        .context("cannot export an invalid route manifest")?;

    let role_permission_bundles = PrimaryRole::ALL
        .into_iter()
        .map(|role| {
            let mut grants = grants_for_role(role);
            grants.sort_by(|left, right| left.permission.as_str().cmp(right.permission.as_str()));
            RolePermissionBundle { role, grants }
        })
        .collect();

    let routes = manifest.routes.iter().map(route_inventory_entry).collect();

    Ok(RouteInventory {
        schema_version: 1,
        policy_revision: manifest.policy_revision,
        generated_from: "conductor_server::http::authorization::route_manifest",
        route_count: manifest.routes.len(),
        baseline_assumptions: vec![
            "role baselines use the current persisted active primary role",
            "browser and connection credentials are otherwise structurally valid",
            "allow means no target-dependent authorization fact remains",
            "conditional means the listed selector or grant constraints must be evaluated from authoritative request/storage facts",
            "business invariants and handler payload validation are outside the role baseline",
        ],
        role_permission_bundles,
        routes,
    })
}

pub fn render_route_inventory() -> anyhow::Result<String> {
    let inventory = build_route_inventory()?;
    let mut rendered = serde_json::to_string_pretty(&inventory)
        .context("serialize the REQ-004 route inventory")?;
    rendered.push('\n');
    Ok(rendered)
}

fn route_inventory_entry(route: &RouteSpec) -> RouteInventoryEntry {
    RouteInventoryEntry {
        route_id: route.route_id,
        action: route.action,
        method: route.method.as_str(),
        path: format!("/api{}", route.path),
        target_type: route.target_type,
        authentication: route.authentication.clone(),
        body_policy: match route.transport.body_limit_bytes {
            Some(bytes) => BodyPolicy::MaxBytes { bytes },
            None => BodyPolicy::Default,
        },
        role_baselines: RoleBaselines {
            admin: role_baseline(route, PrimaryRole::Admin),
            contribute: role_baseline(route, PrimaryRole::Contribute),
            user: role_baseline(route, PrimaryRole::User),
        },
    }
}

fn role_baseline(route: &RouteSpec, role: PrimaryRole) -> RoleBaseline {
    match &route.authentication {
        RouteAuthentication::ExplicitPublic => RoleBaseline {
            outcome: BaselineOutcome::Allow,
            target_dependent: false,
            possible_response_projections: vec![ResponseProjection::Full],
            required_scope: None,
            feasible_alternatives: vec![],
        },
        RouteAuthentication::Bootstrap => RoleBaseline {
            outcome: BaselineOutcome::Conditional,
            target_dependent: true,
            possible_response_projections: vec![ResponseProjection::Full],
            required_scope: None,
            feasible_alternatives: vec![],
        },
        RouteAuthentication::Connection(policy) => {
            let compatible = scope_is_role_compatible(role, policy.required_scope);
            RoleBaseline {
                outcome: if compatible {
                    BaselineOutcome::Conditional
                } else {
                    BaselineOutcome::Deny
                },
                target_dependent: compatible,
                possible_response_projections: compatible
                    .then_some(ResponseProjection::Full)
                    .into_iter()
                    .collect(),
                required_scope: Some(policy.required_scope),
                feasible_alternatives: vec![],
            }
        }
        RouteAuthentication::Browser(policy) => {
            let grants = grants_for_role(role);
            let feasible_alternatives = policy
                .alternatives
                .iter()
                .filter_map(|alternative| {
                    let grant = grants
                        .iter()
                        .find(|grant| grant.permission == alternative.permission)?;
                    let selector_constraints = alternative.selector.to_constraint();
                    if !constraints_can_overlap(&grant.constraints, &selector_constraints) {
                        return None;
                    }
                    let target_dependent = selector_is_target_dependent(&alternative.selector)
                        || constraint_is_target_dependent(&grant.constraints);
                    Some(FeasibleBrowserAlternative {
                        permission: alternative.permission,
                        role_grant_constraints: grant.constraints.clone(),
                        route_selector: alternative.selector.clone(),
                        target_dependent,
                        response_projection: alternative
                            .response_projection
                            .or(grant.response_projection)
                            .unwrap_or(ResponseProjection::Full),
                    })
                })
                .collect::<Vec<_>>();

            if feasible_alternatives.is_empty() {
                return RoleBaseline {
                    outcome: BaselineOutcome::Deny,
                    target_dependent: false,
                    possible_response_projections: vec![],
                    required_scope: None,
                    feasible_alternatives,
                };
            }

            let target_dependent = feasible_alternatives
                .iter()
                .all(|alternative| alternative.target_dependent);
            let outcome = if target_dependent {
                BaselineOutcome::Conditional
            } else {
                BaselineOutcome::Allow
            };
            let projections = reachable_browser_projections(route, role);

            RoleBaseline {
                outcome,
                target_dependent,
                possible_response_projections: projections,
                required_scope: None,
                feasible_alternatives,
            }
        }
    }
}

fn reachable_browser_projections(route: &RouteSpec, role: PrimaryRole) -> Vec<ResponseProjection> {
    let RouteAuthentication::Browser(policy) = &route.authentication else {
        return vec![];
    };
    let actor_id = Uuid::from_u128(1);
    let other_id = Uuid::from_u128(2);
    let project_id = Uuid::from_u128(3);
    let other_project_id = Uuid::from_u128(4);
    let mut projections = vec![];

    for self_actor in [false, true] {
        for owner_actor in [false, true] {
            for same_project in [false, true] {
                for effective_audience in [false, true] {
                    for resource_kind in RESOURCE_KINDS {
                        for lifecycle in LIFECYCLES {
                            let decision = evaluate_policy(&PolicyInput {
                                actor_id,
                                actor_project_id: Some(project_id),
                                role,
                                status: UserStatus::Active,
                                authentication_kind: AuthenticationKind::BrowserSession,
                                requirement: DeclaredRequirement::Browser(
                                    policy.domain_requirement(),
                                ),
                                action: route.action,
                                expected_target_type: route.target_type,
                                target: AuthorizationTarget {
                                    project_id: Some(if same_project {
                                        project_id
                                    } else {
                                        other_project_id
                                    }),
                                    target_type: route.target_type,
                                    target_id: Some(if self_actor { actor_id } else { other_id }),
                                    owner_id: Some(if owner_actor { actor_id } else { other_id }),
                                    resource_kind: Some(resource_kind),
                                    lifecycle: Some(lifecycle),
                                    effective_audience: Some(effective_audience),
                                },
                                aggregate_only: Some(true),
                                credential_scopes: vec![],
                            });
                            if decision.allow {
                                let projection = decision
                                    .response_projection
                                    .unwrap_or(ResponseProjection::Full);
                                if !projections.contains(&projection) {
                                    projections.push(projection);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    projections.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    projections
}

fn selector_is_target_dependent(selector: &RouteTargetSelector) -> bool {
    match selector {
        RouteTargetSelector::None
        | RouteTargetSelector::AggregateOnly
        | RouteTargetSelector::CurrentScopePolicy => false,
        RouteTargetSelector::AllOf(items) | RouteTargetSelector::AnyOf(items) => {
            items.iter().any(selector_is_target_dependent)
        }
        RouteTargetSelector::SetupIncomplete
        | RouteTargetSelector::ProjectMember
        | RouteTargetSelector::SelfActor
        | RouteTargetSelector::SelfMemberPath
        | RouteTargetSelector::OtherMemberPath
        | RouteTargetSelector::AnyMemberPath
        | RouteTargetSelector::MemberSecretPath
        | RouteTargetSelector::SecretOwnerPath
        | RouteTargetSelector::EntityMemberPath
        | RouteTargetSelector::EntityResourcePath
        | RouteTargetSelector::NewResourceOwnerActor
        | RouteTargetSelector::ResourceOwnerPath
        | RouteTargetSelector::KindPlugin
        | RouteTargetSelector::KindPath
        | RouteTargetSelector::KindOfResourcePath
        | RouteTargetSelector::AgentOrSkill
        | RouteTargetSelector::RestrictedKind
        | RouteTargetSelector::InProjectResourcePath
        | RouteTargetSelector::EffectiveAudienceList
        | RouteTargetSelector::EffectiveVersionPath
        | RouteTargetSelector::MemberDetail
        | RouteTargetSelector::VisibleResourcePath
        | RouteTargetSelector::VisibleAnalyticsViewPath
        | RouteTargetSelector::ViewOwnerPath
        | RouteTargetSelector::NewInstallationOwnerActor
        | RouteTargetSelector::InstallationOwnerBody
        | RouteTargetSelector::InventoryItemsVisibleBody
        | RouteTargetSelector::TelemetryAttributionOwner => true,
    }
}

fn constraint_is_target_dependent(constraint: &ConstraintExpr) -> bool {
    match constraint {
        ConstraintExpr::Atom(TargetConstraint::Any) => false,
        ConstraintExpr::Atom(
            TargetConstraint::SelfActor
            | TargetConstraint::OwnerActor
            | TargetConstraint::EffectiveAudience
            | TargetConstraint::SameProject
            | TargetConstraint::AggregateOnly
            | TargetConstraint::ResourceKindIn { .. }
            | TargetConstraint::LifecycleIn { .. },
        ) => true,
        ConstraintExpr::AllOf(items) | ConstraintExpr::AnyOf(items) => {
            items.iter().any(constraint_is_target_dependent)
        }
    }
}

/// Whether the positive, closed V1 target constraints share at least one
/// resource-kind/lifecycle assignment. Actor/project/audience atoms can all be
/// simultaneously true; kind/lifecycle sets are the only conflicting atoms.
fn constraints_can_overlap(left: &ConstraintExpr, right: &ConstraintExpr) -> bool {
    RESOURCE_KINDS.into_iter().any(|kind| {
        LIFECYCLES.into_iter().any(|lifecycle| {
            constraint_matches(left, kind, lifecycle) && constraint_matches(right, kind, lifecycle)
        })
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
