use std::collections::BTreeSet;

use conductor_domain::{
    AuthorizationAction, ConnectionPolicyRequirement, ConstraintExpr, DeclaredRequirement,
    NonEmptySet, PermissionAlternative, PermissionKey, PolicyRequirement, ResourceKind,
    ResponseProjection, SecretScope, TargetConstraint, TargetType,
};
use serde::Serialize;

pub const EXPECTED_ROUTE_ACTIONS: usize = 95;
pub const MAX_LOGO_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RouteMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl RouteMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }
}

/// Describes where authoritative facts for a route requirement come from.
///
/// This is intentionally more specific than the pure domain constraints. For
/// example, `ResourceOwnerPath` tells a handler that ownership must be loaded
/// from the resource identified by the path, while the domain evaluator only
/// needs the resulting `OwnerActor` fact. No free-form selector strings are
/// accepted by the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "items", rename_all = "snake_case")]
pub enum RouteTargetSelector {
    None,
    SetupIncomplete,
    ProjectMember,
    SelfActor,
    SelfMemberPath,
    OtherMemberPath,
    AnyMemberPath,
    MemberSecretPath,
    SecretOwnerPath,
    EntityMemberPath,
    EntityResourcePath,
    NewResourceOwnerActor,
    ResourceOwnerPath,
    KindPlugin,
    KindPath,
    KindOfResourcePath,
    AgentOrSkill,
    RestrictedKind,
    InProjectResourcePath,
    EffectiveAudienceList,
    EffectiveVersionPath,
    AggregateOnly,
    MemberDetail,
    VisibleResourcePath,
    VisibleAnalyticsViewPath,
    ViewOwnerPath,
    NewInstallationOwnerActor,
    InstallationOwnerBody,
    InventoryItemsVisibleBody,
    TelemetryAttributionOwner,
    CurrentScopePolicy,
    AllOf(Vec<RouteTargetSelector>),
    AnyOf(Vec<RouteTargetSelector>),
}

impl RouteTargetSelector {
    pub fn all_of(items: impl IntoIterator<Item = Self>) -> Self {
        Self::AllOf(items.into_iter().collect())
    }

    pub fn to_constraint(&self) -> ConstraintExpr {
        use RouteTargetSelector as S;
        use TargetConstraint as C;

        match self {
            S::SelfActor | S::SelfMemberPath => ConstraintExpr::atom(C::SelfActor),
            S::SecretOwnerPath | S::TelemetryAttributionOwner => {
                ConstraintExpr::atom(C::OwnerActor)
            }
            S::ResourceOwnerPath | S::ViewOwnerPath | S::NewInstallationOwnerActor => {
                ConstraintExpr::atom(C::OwnerActor)
            }
            S::InstallationOwnerBody => ConstraintExpr::all_of(vec![
                ConstraintExpr::atom(C::OwnerActor),
                ConstraintExpr::atom(C::SameProject),
            ])
            .expect("installation owner selector has two constraints"),
            S::EffectiveAudienceList
            | S::EffectiveVersionPath
            | S::VisibleResourcePath
            | S::VisibleAnalyticsViewPath
            | S::InventoryItemsVisibleBody => ConstraintExpr::atom(C::EffectiveAudience),
            S::ProjectMember
            | S::OtherMemberPath
            | S::AnyMemberPath
            | S::MemberSecretPath
            | S::EntityMemberPath
            | S::EntityResourcePath
            | S::InProjectResourcePath
            | S::MemberDetail => ConstraintExpr::atom(C::SameProject),
            S::AggregateOnly => ConstraintExpr::atom(C::AggregateOnly),
            S::KindPlugin => kind_constraint(vec![ResourceKind::Plugin]),
            S::AgentOrSkill => kind_constraint(vec![ResourceKind::Agent, ResourceKind::Skill]),
            S::RestrictedKind => kind_constraint(vec![
                ResourceKind::Plugin,
                ResourceKind::Workflow,
                ResourceKind::Command,
            ]),
            S::AllOf(items) => {
                ConstraintExpr::all_of(items.iter().map(Self::to_constraint).collect::<Vec<_>>())
                    .expect("route all-of selectors are declared non-empty")
            }
            S::AnyOf(items) => {
                ConstraintExpr::any_of(items.iter().map(Self::to_constraint).collect::<Vec<_>>())
                    .expect("route any-of selectors are declared non-empty")
            }
            S::None
            | S::SetupIncomplete
            | S::NewResourceOwnerActor
            | S::KindPath
            | S::KindOfResourcePath
            | S::CurrentScopePolicy => ConstraintExpr::any(),
        }
    }

    pub fn can_resolve_at_route_boundary(&self) -> bool {
        match self {
            Self::ProjectMember
            | Self::SelfActor
            | Self::NewResourceOwnerActor
            | Self::KindPlugin
            | Self::AggregateOnly
            | Self::NewInstallationOwnerActor
            | Self::CurrentScopePolicy => true,
            Self::AllOf(items) | Self::AnyOf(items) => {
                !items.is_empty() && items.iter().all(Self::can_resolve_at_route_boundary)
            }
            _ => false,
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::AllOf(items) | Self::AnyOf(items) => {
                !items.is_empty() && items.iter().all(Self::is_valid)
            }
            _ => true,
        }
    }
}

fn kind_constraint(values: Vec<ResourceKind>) -> ConstraintExpr {
    ConstraintExpr::atom(TargetConstraint::ResourceKindIn {
        values: NonEmptySet::try_new(values)
            .expect("fixed route resource-kind set is non-empty and unique"),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrowserRouteAlternative {
    pub permission: PermissionKey,
    pub selector: RouteTargetSelector,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_projection: Option<ResponseProjection>,
}

impl BrowserRouteAlternative {
    pub fn new(permission: PermissionKey, selector: RouteTargetSelector) -> Self {
        Self {
            permission,
            selector,
            response_projection: None,
        }
    }

    pub fn with_projection(mut self, projection: ResponseProjection) -> Self {
        self.response_projection = Some(projection);
        self
    }

    fn domain_alternative(&self) -> PermissionAlternative {
        let mut alternative =
            PermissionAlternative::new(self.permission, self.selector.to_constraint());
        alternative.response_projection = self.response_projection;
        alternative
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrowserRoutePolicy {
    pub requirement_id: &'static str,
    pub alternatives: Vec<BrowserRouteAlternative>,
}

impl BrowserRoutePolicy {
    pub fn one(
        requirement_id: &'static str,
        permission: PermissionKey,
        selector: RouteTargetSelector,
    ) -> Self {
        Self {
            requirement_id,
            alternatives: vec![BrowserRouteAlternative::new(permission, selector)],
        }
    }

    pub fn alternatives(
        requirement_id: &'static str,
        alternatives: Vec<BrowserRouteAlternative>,
    ) -> Self {
        Self {
            requirement_id,
            alternatives,
        }
    }

    pub fn domain_requirement(&self) -> PolicyRequirement {
        PolicyRequirement::new(
            self.requirement_id,
            self.alternatives
                .iter()
                .map(BrowserRouteAlternative::domain_alternative)
                .collect(),
        )
        .expect("classified browser route has at least one valid alternative")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectionRoutePolicy {
    pub requirement_id: &'static str,
    pub required_scope: SecretScope,
    pub selector: RouteTargetSelector,
}

impl ConnectionRoutePolicy {
    pub fn domain_requirement(&self) -> ConnectionPolicyRequirement {
        ConnectionPolicyRequirement {
            requirement_id: self.requirement_id.to_owned(),
            required_scope: self.required_scope,
            constraints: self.selector.to_constraint(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "class", content = "policy", rename_all = "snake_case")]
pub enum RouteAuthentication {
    ExplicitPublic,
    Bootstrap,
    Browser(BrowserRoutePolicy),
    Connection(ConnectionRoutePolicy),
}

impl RouteAuthentication {
    pub fn declared_requirement(&self) -> Option<DeclaredRequirement> {
        match self {
            Self::Browser(policy) => {
                Some(DeclaredRequirement::Browser(policy.domain_requirement()))
            }
            Self::Connection(policy) => {
                Some(DeclaredRequirement::Connection(policy.domain_requirement()))
            }
            Self::ExplicitPublic | Self::Bootstrap => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RouteTransport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_limit_bytes: Option<usize>,
}

impl RouteTransport {
    pub const fn body_limit(bytes: usize) -> Self {
        Self {
            body_limit_bytes: Some(bytes),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RouteDefinition {
    pub route_id: &'static str,
    pub action: AuthorizationAction,
    pub target_type: TargetType,
    pub authentication: RouteAuthentication,
    pub transport: RouteTransport,
}

impl RouteDefinition {
    pub fn public(action: AuthorizationAction, target_type: TargetType) -> Self {
        Self::new(action, target_type, RouteAuthentication::ExplicitPublic)
    }

    pub fn bootstrap(action: AuthorizationAction, target_type: TargetType) -> Self {
        Self::new(action, target_type, RouteAuthentication::Bootstrap)
    }

    pub fn browser(
        action: AuthorizationAction,
        target_type: TargetType,
        permission: PermissionKey,
        selector: RouteTargetSelector,
    ) -> Self {
        Self::new(
            action,
            target_type,
            RouteAuthentication::Browser(BrowserRoutePolicy::one(
                action.as_str(),
                permission,
                selector,
            )),
        )
    }

    pub fn browser_alternatives(
        action: AuthorizationAction,
        target_type: TargetType,
        alternatives: Vec<BrowserRouteAlternative>,
    ) -> Self {
        Self::new(
            action,
            target_type,
            RouteAuthentication::Browser(BrowserRoutePolicy::alternatives(
                action.as_str(),
                alternatives,
            )),
        )
    }

    pub fn connection(
        action: AuthorizationAction,
        target_type: TargetType,
        required_scope: SecretScope,
        selector: RouteTargetSelector,
    ) -> Self {
        Self::new(
            action,
            target_type,
            RouteAuthentication::Connection(ConnectionRoutePolicy {
                requirement_id: action.as_str(),
                required_scope,
                selector,
            }),
        )
    }

    pub const fn with_transport(mut self, transport: RouteTransport) -> Self {
        self.transport = transport;
        self
    }

    fn new(
        action: AuthorizationAction,
        target_type: TargetType,
        authentication: RouteAuthentication,
    ) -> Self {
        Self {
            route_id: action.as_str(),
            action,
            target_type,
            authentication,
            transport: RouteTransport {
                body_limit_bytes: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteSpec {
    pub route_id: &'static str,
    pub method: RouteMethod,
    pub path: &'static str,
    pub action: AuthorizationAction,
    pub target_type: TargetType,
    pub authentication: RouteAuthentication,
    pub transport: RouteTransport,
}

impl RouteSpec {
    pub fn new(method: RouteMethod, path: &'static str, definition: RouteDefinition) -> Self {
        Self {
            route_id: definition.route_id,
            method,
            path,
            action: definition.action,
            target_type: definition.target_type,
            authentication: definition.authentication,
            transport: definition.transport,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteManifest {
    pub schema_version: u8,
    pub policy_revision: &'static str,
    pub routes: Vec<RouteSpec>,
}

impl RouteManifest {
    pub fn new(mut routes: Vec<RouteSpec>) -> Self {
        routes.sort_by(|left, right| {
            left.route_id
                .cmp(right.route_id)
                .then(left.method.cmp(&right.method))
                .then(left.path.cmp(right.path))
        });
        Self {
            schema_version: 1,
            policy_revision: conductor_domain::V1_POLICY_REVISION,
            routes,
        }
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.routes.len() != EXPECTED_ROUTE_ACTIONS {
            return Err(ManifestError::WrongActionCount {
                expected: EXPECTED_ROUTE_ACTIONS,
                actual: self.routes.len(),
            });
        }

        let mut route_ids = BTreeSet::new();
        let mut actions = BTreeSet::new();
        let mut method_paths = BTreeSet::new();
        for route in &self.routes {
            if route.route_id != route.action.as_str() {
                return Err(ManifestError::RouteIdActionMismatch(route.route_id));
            }
            if !route_ids.insert(route.route_id) {
                return Err(ManifestError::DuplicateRouteId(route.route_id));
            }
            if !actions.insert(route.action.as_str()) {
                return Err(ManifestError::DuplicateAction(route.action.as_str()));
            }
            if !method_paths.insert((route.method, route.path)) {
                return Err(ManifestError::DuplicateMethodPath {
                    method: route.method,
                    path: route.path,
                });
            }
            match &route.authentication {
                RouteAuthentication::Browser(policy) => {
                    if policy.alternatives.is_empty()
                        || policy
                            .alternatives
                            .iter()
                            .any(|alternative| !alternative.selector.is_valid())
                    {
                        return Err(ManifestError::InvalidRequirement(route.route_id));
                    }
                }
                RouteAuthentication::Connection(policy) if !policy.selector.is_valid() => {
                    return Err(ManifestError::InvalidRequirement(route.route_id));
                }
                RouteAuthentication::ExplicitPublic
                | RouteAuthentication::Bootstrap
                | RouteAuthentication::Connection(_) => {}
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("route manifest has {actual} actions; expected {expected}")]
    WrongActionCount { expected: usize, actual: usize },
    #[error("duplicate route id: {0}")]
    DuplicateRouteId(&'static str),
    #[error("duplicate action: {0}")]
    DuplicateAction(&'static str),
    #[error("duplicate method/path: {method:?} {path}")]
    DuplicateMethodPath {
        method: RouteMethod,
        path: &'static str,
    },
    #[error("route id and action differ: {0}")]
    RouteIdActionMismatch(&'static str),
    #[error("invalid policy requirement for route: {0}")]
    InvalidRequirement(&'static str),
}
