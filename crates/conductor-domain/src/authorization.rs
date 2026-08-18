use std::fmt;
use std::str::FromStr;

use serde::de::Error as _;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::{PrimaryRole, ResourceKind, SecretScope, UserStatus};

pub const V1_POLICY_REVISION: &str = "req-004-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("unknown {type_name}")]
pub struct UnknownPolicyValue {
    type_name: &'static str,
}

impl UnknownPolicyValue {
    const fn new(type_name: &'static str) -> Self {
        Self { type_name }
    }
}

macro_rules! stable_string_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($variant:ident => $wire:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }

            pub fn parse(value: &str) -> Option<Self> {
                match value {
                    $($wire => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = UnknownPolicyValue;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value).ok_or_else(|| UnknownPolicyValue::new(stringify!($name)))
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).ok_or_else(|| D::Error::custom(concat!("unknown ", stringify!($name))))
            }
        }
    };
}

stable_string_enum! {
    /// Stable browser-management permission keys. Unknown strings are rejected.
    pub enum PermissionKey {
        AuthorizationGrantsReadSelf => "authorization.grants.read_self",
        SessionSelfRead => "session.self.read",
        SessionPasswordChange => "session.password.change",
        ProjectBrandingRead => "project.branding.read",
        ProjectDashboardRead => "project.dashboard.read",
        ProjectSettingsRead => "project.settings.read",
        ProjectSettingsManage => "project.settings.manage",
        MemberDirectoryRead => "member.directory.read",
        MemberManage => "member.manage",
        MemberPrivateReadSelf => "member.private.read_self",
        MemberPrivateReadAny => "member.private.read_any",
        TelemetryProjectRead => "telemetry.project.read",
        TelemetryMemberReadSelf => "telemetry.member.read_self",
        TelemetryMemberReadAny => "telemetry.member.read_any",
        TaxonomyRead => "taxonomy.read",
        TaxonomyDefinitionManage => "taxonomy.definition.manage",
        MemberTagAssignmentManage => "member.tag_assignment.manage",
        ResourceConsume => "resource.consume",
        ResourceAuthor => "resource.author",
        ResourceAccessManage => "resource.access.manage",
        ResourceLifecycleManage => "resource.lifecycle.manage",
        ResourceReleaseNonExecutable => "resource.release.non_executable",
        ResourceReleaseRestricted => "resource.release.restricted",
        ResourceMonitoringAggregateRead => "resource.monitoring.aggregate.read",
        ResourceMonitoringMemberDetailRead => "resource.monitoring.member_detail.read",
        ResourceFeedbackSubmit => "resource.feedback.submit",
        ResourceFeedbackRead => "resource.feedback.read",
        AnalyticsViewRead => "analytics_view.read",
        AnalyticsViewManageSelf => "analytics_view.manage_self",
        AnalyticsViewManageAny => "analytics_view.manage_any",
        ConnectionTokenIssueSelf => "connection_token.issue_self",
        ConnectionTokenReadSelf => "connection_token.read_self",
        ConnectionTokenRevokeSelf => "connection_token.revoke_self",
        ConnectionTokenReadAny => "connection_token.read_any",
        ConnectionTokenRevokeAny => "connection_token.revoke_any",
        AuditRead => "audit.read",
        AuditExport => "audit.export"
    }
}

stable_string_enum! {
    /// Stable, audit-oriented action names. Actions are intentionally more granular than permissions.
    pub enum AuthorizationAction {
        HealthRead => "health.read",
        SetupStatusRead => "setup.status.read",
        SetupComplete => "setup.complete",
        AuthLogin => "auth.login",
        AuthSsoStart => "auth.sso.start",
        AuthSsoCallback => "auth.sso.callback",
        SessionSelfRead => "session.self.read",
        SessionPasswordChange => "session.password.change",
        AuthorizationGrantsReadSelf => "authorization.grants.read_self",
        ProjectBrandingRead => "project.branding.read",
        ProjectDashboardRead => "project.dashboard.read",
        ProjectSettingsRead => "project.settings.read",
        ProjectSettingsUpdate => "project.settings.update",
        ProjectLogoRead => "project.logo.read",
        ProjectLogoUpload => "project.logo.upload",
        ProjectLogoDelete => "project.logo.delete",
        ProjectNetworkUpdate => "project.network.update",
        ProjectSsoRead => "project.sso.read",
        ProjectSsoUpdate => "project.sso.update",
        ProjectStorageUpdate => "project.storage.update",
        ProjectDataPolicyUpdate => "project.data_policy.update",
        MemberDirectoryList => "member.directory.list",
        MemberPendingCountRead => "member.pending_count.read",
        MemberPrivateRead => "member.private.read",
        MemberCreate => "member.create",
        MemberApprove => "member.approve",
        MemberAccessProfileUpdate => "member.access_profile.update",
        MemberDisable => "member.disable",
        MemberEnable => "member.enable",
        MemberPasswordReset => "member.password.reset",
        MemberInstallationsList => "member.installations.list",
        MemberConnectionTokensList => "member.connection_tokens.list",
        MemberConnectionTokenIssueSelf => "member.connection_token.issue_self",
        MemberConnectionTokenRevoke => "member.connection_token.revoke",
        MemberUsageSummaryRead => "member.usage_summary.read",
        MemberActivityList => "member.activity.list",
        MemberActivityDetailRead => "member.activity_detail.read",
        MemberToolsSummaryRead => "member.tools_summary.read",
        TaxonomySubRolesList => "taxonomy.sub_roles.list",
        TaxonomySubRoleCreate => "taxonomy.sub_role.create",
        TaxonomySubRoleUpdate => "taxonomy.sub_role.update",
        TaxonomySubRoleDelete => "taxonomy.sub_role.delete",
        TaxonomyTagsList => "taxonomy.tags.list",
        TaxonomyTagCreate => "taxonomy.tag.create",
        TaxonomyTagUpdate => "taxonomy.tag.update",
        TaxonomyTagDelete => "taxonomy.tag.delete",
        TaxonomyAssignmentRead => "taxonomy.assignment.read",
        TaxonomyAssignmentSet => "taxonomy.assignment.set",
        ConnectionTokensSelfList => "connection_tokens.self.list",
        ConnectionTokensSelfIssue => "connection_tokens.self.issue",
        ConnectionTokensSelfRevoke => "connection_tokens.self.revoke",
        ResourcesList => "resources.list",
        ResourceCreate => "resource.create",
        ResourceUpdate => "resource.update",
        ResourceArchive => "resource.archive",
        ResourceAccessRead => "resource.access.read",
        ResourceAccessUpdate => "resource.access.update",
        ResourceRelease => "resource.release",
        ResourceVersionsList => "resource.versions.list",
        ResourceVersionDeprecate => "resource.version.deprecate",
        ResourceVersionRestoreToDraft => "resource.version.restore_to_draft",
        ResourceMonitoringRead => "resource.monitoring.read",
        ResourceInventoryMonitoringRead => "resource.inventory_monitoring.read",
        ResourceFeedbackList => "resource.feedback.list",
        ResourceFeedbackSubmit => "resource.feedback.submit",
        ResourceDraftTreeRead => "resource.draft_tree.read",
        ResourceDraftFileSave => "resource.draft_file.save",
        ResourceDraftEntryCreate => "resource.draft_entry.create",
        ResourceDraftEntryDelete => "resource.draft_entry.delete",
        ResourceDraftEntryMove => "resource.draft_entry.move",
        ResourceDraftValidate => "resource.draft.validate",
        ResourceDraftArchiveImport => "resource.draft_archive.import",
        ResourcePluginArchiveInspect => "resource.plugin_archive.inspect",
        ResourcePluginArchiveImport => "resource.plugin_archive.import",
        ResourceArchiveInspect => "resource.archive.inspect",
        ResourceArchiveImport => "resource.archive.import",
        ResourceAuthoringGuideRead => "resource.authoring_guide.read",
        ResourceAuthoringTemplateRead => "resource.authoring_template.read",
        AnalyticsResourceUsageRead => "analytics.resource_usage.read",
        AnalyticsViewsList => "analytics_views.list",
        AnalyticsViewRead => "analytics_view.read",
        AnalyticsViewCreate => "analytics_view.create",
        AnalyticsViewUpdate => "analytics_view.update",
        AnalyticsViewDelete => "analytics_view.delete",
        ClientRegister => "client.register",
        ClientHeartbeat => "client.heartbeat",
        ClientResourcesSnapshot => "client.resources.snapshot",
        ClientResourcesChanges => "client.resources.changes",
        ClientResourcesFetch => "client.resources.fetch",
        ClientResourceVersionRead => "client.resource_version.read",
        ClientResourceArtifactRead => "client.resource_artifact.read",
        ClientInventorySync => "client.inventory.sync",
        ClientTelemetryIngest => "client.telemetry.ingest",
        ClientResourceUsageIngest => "client.resource_usage.ingest",
        ClientRealtimeEvents => "client.realtime.events"
    }
}

stable_string_enum! {
    pub enum AuthenticationKind {
        BrowserSession => "browser_session",
        ConnectionToken => "connection_token",
        Bootstrap => "bootstrap",
        Public => "public"
    }
}

stable_string_enum! {
    pub enum TargetType {
        Project => "project",
        Session => "session",
        Member => "member",
        Taxonomy => "taxonomy",
        Resource => "resource",
        AnalyticsView => "analytics_view",
        ConnectionToken => "connection_token",
        ClientInstallation => "client_installation",
        Audit => "audit"
    }
}

stable_string_enum! {
    pub enum LifecycleState {
        Draft => "draft",
        Beta => "beta",
        Published => "published",
        Archived => "archived",
        Deprecated => "deprecated"
    }
}

stable_string_enum! {
    pub enum ResponseProjection {
        Full => "full",
        DirectorySafe => "directory_safe",
        AggregateOnly => "aggregate_only"
    }
}

stable_string_enum! {
    pub enum DecisionReason {
        AllowRole => "allow_role",
        AllowSelf => "allow_self",
        AllowOwner => "allow_owner",
        AllowAudience => "allow_audience",
        DenyInactivePrincipal => "deny_inactive_principal",
        DenyUnknownPolicy => "deny_unknown_policy",
        DenyAuthenticationKind => "deny_authentication_kind",
        DenyRole => "deny_role",
        DenyScope => "deny_scope",
        DenyNotSelf => "deny_not_self",
        DenyNotOwner => "deny_not_owner",
        DenyKind => "deny_kind",
        DenyLifecycle => "deny_lifecycle",
        DenyCrossProject => "deny_cross_project",
        DenyOutsideAudience => "deny_outside_audience",
        DenyDetailAccess => "deny_detail_access",
        DenyTargetType => "deny_target_type"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyDefinitionError {
    #[error("a policy collection must not be empty")]
    EmptyCollection,
    #[error("a policy set must not contain duplicate values")]
    DuplicateValue,
}

/// A non-empty, duplicate-free sequence with deterministic serialization order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct NonEmptySet<T>(Vec<T>);

impl<T: PartialEq> NonEmptySet<T> {
    pub fn try_new(values: Vec<T>) -> Result<Self, PolicyDefinitionError> {
        if values.is_empty() {
            return Err(PolicyDefinitionError::EmptyCollection);
        }
        if values
            .iter()
            .enumerate()
            .any(|(index, value)| values[..index].contains(value))
        {
            return Err(PolicyDefinitionError::DuplicateValue);
        }
        Ok(Self(values))
    }

    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    pub fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
    }
}

impl<'de, T> Deserialize<'de> for NonEmptySet<T>
where
    T: Deserialize<'de> + PartialEq,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Vec::<T>::deserialize(deserializer)?;
        Self::try_new(values).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetConstraint {
    Any,
    #[serde(rename = "self")]
    SelfActor,
    #[serde(rename = "owner")]
    OwnerActor,
    EffectiveAudience,
    SameProject,
    AggregateOnly,
    ResourceKindIn {
        values: NonEmptySet<ResourceKind>,
    },
    LifecycleIn {
        values: NonEmptySet<LifecycleState>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintExpr {
    Atom(TargetConstraint),
    AllOf(Vec<ConstraintExpr>),
    AnyOf(Vec<ConstraintExpr>),
}

impl ConstraintExpr {
    pub const fn any() -> Self {
        Self::Atom(TargetConstraint::Any)
    }

    pub const fn atom(constraint: TargetConstraint) -> Self {
        Self::Atom(constraint)
    }

    pub fn all_of(items: Vec<Self>) -> Result<Self, PolicyDefinitionError> {
        if items.is_empty() {
            Err(PolicyDefinitionError::EmptyCollection)
        } else {
            Ok(Self::AllOf(items))
        }
    }

    pub fn any_of(items: Vec<Self>) -> Result<Self, PolicyDefinitionError> {
        if items.is_empty() {
            Err(PolicyDefinitionError::EmptyCollection)
        } else {
            Ok(Self::AnyOf(items))
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::Atom(_) => true,
            Self::AllOf(items) | Self::AnyOf(items) => {
                !items.is_empty() && items.iter().all(Self::is_valid)
            }
        }
    }
}

impl Serialize for ConstraintExpr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Atom(constraint) => constraint.serialize(serializer),
            Self::AllOf(items) => {
                let mut state = serializer.serialize_struct("ConstraintExpr", 2)?;
                state.serialize_field("kind", "all_of")?;
                state.serialize_field("items", items)?;
                state.end()
            }
            Self::AnyOf(items) => {
                let mut state = serializer.serialize_struct("ConstraintExpr", 2)?;
                state.serialize_field("kind", "any_of")?;
                state.serialize_field("items", items)?;
                state.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for ConstraintExpr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case")]
        enum Wire {
            Any,
            #[serde(rename = "self")]
            SelfActor,
            #[serde(rename = "owner")]
            OwnerActor,
            EffectiveAudience,
            SameProject,
            AggregateOnly,
            ResourceKindIn {
                values: NonEmptySet<ResourceKind>,
            },
            LifecycleIn {
                values: NonEmptySet<LifecycleState>,
            },
            AllOf {
                items: Vec<ConstraintExpr>,
            },
            AnyOf {
                items: Vec<ConstraintExpr>,
            },
        }

        let expression = match Wire::deserialize(deserializer)? {
            Wire::Any => Self::Atom(TargetConstraint::Any),
            Wire::SelfActor => Self::Atom(TargetConstraint::SelfActor),
            Wire::OwnerActor => Self::Atom(TargetConstraint::OwnerActor),
            Wire::EffectiveAudience => Self::Atom(TargetConstraint::EffectiveAudience),
            Wire::SameProject => Self::Atom(TargetConstraint::SameProject),
            Wire::AggregateOnly => Self::Atom(TargetConstraint::AggregateOnly),
            Wire::ResourceKindIn { values } => {
                Self::Atom(TargetConstraint::ResourceKindIn { values })
            }
            Wire::LifecycleIn { values } => Self::Atom(TargetConstraint::LifecycleIn { values }),
            Wire::AllOf { items } => Self::all_of(items).map_err(D::Error::custom)?,
            Wire::AnyOf { items } => Self::any_of(items).map_err(D::Error::custom)?,
        };
        Ok(expression)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionGrant {
    pub permission: PermissionKey,
    pub constraints: ConstraintExpr,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_projection: Option<ResponseProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionAlternative {
    pub permission: PermissionKey,
    pub constraints: ConstraintExpr,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_projection: Option<ResponseProjection>,
}

impl PermissionAlternative {
    pub fn new(permission: PermissionKey, constraints: ConstraintExpr) -> Self {
        Self {
            permission,
            constraints,
            response_projection: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRequirement {
    pub requirement_id: String,
    pub alternatives: Vec<PermissionAlternative>,
}

impl PolicyRequirement {
    pub fn new(
        requirement_id: impl Into<String>,
        alternatives: Vec<PermissionAlternative>,
    ) -> Result<Self, PolicyDefinitionError> {
        if alternatives.is_empty() {
            return Err(PolicyDefinitionError::EmptyCollection);
        }
        Ok(Self {
            requirement_id: requirement_id.into(),
            alternatives,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionPolicyRequirement {
    pub requirement_id: String,
    pub required_scope: SecretScope,
    pub constraints: ConstraintExpr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum DeclaredRequirement {
    Browser(PolicyRequirement),
    Connection(ConnectionPolicyRequirement),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationTarget {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
    pub target_type: TargetType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_kind: Option<ResourceKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<LifecycleState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_audience: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyInput {
    pub actor_id: Uuid,
    pub actor_project_id: Option<Uuid>,
    pub role: PrimaryRole,
    pub status: UserStatus,
    pub authentication_kind: AuthenticationKind,
    pub requirement: DeclaredRequirement,
    pub action: AuthorizationAction,
    pub expected_target_type: TargetType,
    pub target: AuthorizationTarget,
    pub aggregate_only: Option<bool>,
    pub credential_scopes: Vec<SecretScope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizationTargetSummary {
    pub target_type: TargetType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_kind: Option<ResourceKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle: Option<LifecycleState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_actor: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_actor: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_audience: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same_project: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate_only: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub allow: bool,
    pub reason_code: DecisionReason,
    pub declared_requirement_id: String,
    pub evaluated_permissions: Vec<PermissionKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_permission: Option<PermissionKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_scope: Option<SecretScope>,
    pub action: AuthorizationAction,
    pub role_snapshot: PrimaryRole,
    pub authentication_kind: AuthenticationKind,
    pub matched_constraints: Vec<TargetConstraint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_constraint: Option<TargetConstraint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_projection: Option<ResponseProjection>,
    pub target_summary: AuthorizationTargetSummary,
    pub policy_revision: String,
}

pub fn role_has_permission(role: PrimaryRole, permission: PermissionKey) -> bool {
    grant_for_role(role, permission).is_some()
}

/// V1 machine scopes are owner-bound and compatible with all three active roles.
pub const fn scope_is_role_compatible(role: PrimaryRole, scope: SecretScope) -> bool {
    matches!(
        (role, scope),
        (
            PrimaryRole::Admin | PrimaryRole::Contribute | PrimaryRole::User,
            SecretScope::SubscribeResources
                | SecretScope::ReportTelemetry
                | SecretScope::SyncInventory
        )
    )
}

pub fn grants_for_role(role: PrimaryRole) -> Vec<PermissionGrant> {
    PermissionKey::ALL
        .iter()
        .filter_map(|permission| grant_for_role(role, *permission))
        .collect()
}

pub fn evaluate_policy(input: &PolicyInput) -> PolicyDecision {
    if input.status != UserStatus::Active {
        return deny(
            input,
            DecisionReason::DenyInactivePrincipal,
            None,
            vec![],
            None,
        );
    }

    if input.target.target_type != input.expected_target_type {
        let (required_scope, evaluated_permissions) = match &input.requirement {
            DeclaredRequirement::Browser(requirement) => (
                None,
                requirement
                    .alternatives
                    .iter()
                    .map(|alternative| alternative.permission)
                    .collect(),
            ),
            DeclaredRequirement::Connection(requirement) => {
                (Some(requirement.required_scope), vec![])
            }
        };
        return deny(
            input,
            DecisionReason::DenyTargetType,
            required_scope,
            evaluated_permissions,
            None,
        );
    }

    match &input.requirement {
        DeclaredRequirement::Browser(requirement) => evaluate_browser(input, requirement),
        DeclaredRequirement::Connection(requirement) => evaluate_connection(input, requirement),
    }
}

fn evaluate_browser(input: &PolicyInput, requirement: &PolicyRequirement) -> PolicyDecision {
    let evaluated_permissions = requirement
        .alternatives
        .iter()
        .map(|alternative| alternative.permission)
        .collect::<Vec<_>>();

    if input.authentication_kind != AuthenticationKind::BrowserSession {
        return deny(
            input,
            DecisionReason::DenyAuthenticationKind,
            None,
            evaluated_permissions,
            None,
        );
    }
    if requirement.requirement_id.trim().is_empty()
        || requirement.alternatives.is_empty()
        || requirement
            .alternatives
            .iter()
            .any(|alternative| !alternative.constraints.is_valid())
    {
        return deny(
            input,
            DecisionReason::DenyUnknownPolicy,
            None,
            evaluated_permissions,
            None,
        );
    }

    let mut first_failure = None;
    let mut role_matched = false;
    for alternative in &requirement.alternatives {
        let Some(grant) = grant_for_role(input.role, alternative.permission) else {
            continue;
        };
        role_matched = true;

        let grant_result = evaluate_constraint(&grant.constraints, input);
        if !grant_result.allowed {
            if first_failure.is_none() {
                first_failure = grant_result.failed;
            }
            continue;
        }
        let alternative_result = evaluate_constraint(&alternative.constraints, input);
        if !alternative_result.allowed {
            if first_failure.is_none() {
                first_failure = alternative_result.failed;
            }
            continue;
        }

        let mut matched = grant_result.matched;
        for constraint in alternative_result.matched {
            if !matched.contains(&constraint) {
                matched.push(constraint);
            }
        }
        return PolicyDecision {
            allow: true,
            reason_code: allow_reason(&matched),
            declared_requirement_id: requirement.requirement_id.clone(),
            evaluated_permissions,
            resolved_permission: Some(alternative.permission),
            required_scope: None,
            action: input.action,
            role_snapshot: input.role,
            authentication_kind: input.authentication_kind,
            matched_constraints: matched,
            failed_constraint: None,
            response_projection: alternative
                .response_projection
                .or(grant.response_projection),
            target_summary: summarize_target(input),
            policy_revision: V1_POLICY_REVISION.to_owned(),
        };
    }

    let reason = if role_matched {
        first_failure
            .as_ref()
            .map(reason_for_failed_constraint)
            .unwrap_or(DecisionReason::DenyUnknownPolicy)
    } else {
        DecisionReason::DenyRole
    };
    deny(input, reason, None, evaluated_permissions, first_failure)
}

fn evaluate_connection(
    input: &PolicyInput,
    requirement: &ConnectionPolicyRequirement,
) -> PolicyDecision {
    if input.authentication_kind != AuthenticationKind::ConnectionToken {
        return deny(
            input,
            DecisionReason::DenyAuthenticationKind,
            Some(requirement.required_scope),
            vec![],
            None,
        );
    }
    if requirement.requirement_id.trim().is_empty() || !requirement.constraints.is_valid() {
        return deny(
            input,
            DecisionReason::DenyUnknownPolicy,
            Some(requirement.required_scope),
            vec![],
            None,
        );
    }
    if !scope_is_role_compatible(input.role, requirement.required_scope)
        || !input
            .credential_scopes
            .contains(&requirement.required_scope)
    {
        return deny(
            input,
            DecisionReason::DenyScope,
            Some(requirement.required_scope),
            vec![],
            None,
        );
    }

    let constraint = evaluate_constraint(&requirement.constraints, input);
    if !constraint.allowed {
        let reason = constraint
            .failed
            .as_ref()
            .map(reason_for_failed_constraint)
            .unwrap_or(DecisionReason::DenyUnknownPolicy);
        return deny(
            input,
            reason,
            Some(requirement.required_scope),
            vec![],
            constraint.failed,
        );
    }

    PolicyDecision {
        allow: true,
        reason_code: allow_reason(&constraint.matched),
        declared_requirement_id: requirement.requirement_id.clone(),
        evaluated_permissions: vec![],
        resolved_permission: None,
        required_scope: Some(requirement.required_scope),
        action: input.action,
        role_snapshot: input.role,
        authentication_kind: input.authentication_kind,
        matched_constraints: constraint.matched,
        failed_constraint: None,
        response_projection: None,
        target_summary: summarize_target(input),
        policy_revision: V1_POLICY_REVISION.to_owned(),
    }
}

fn deny(
    input: &PolicyInput,
    reason_code: DecisionReason,
    required_scope: Option<SecretScope>,
    evaluated_permissions: Vec<PermissionKey>,
    failed_constraint: Option<TargetConstraint>,
) -> PolicyDecision {
    let declared_requirement_id = match &input.requirement {
        DeclaredRequirement::Browser(requirement) => requirement.requirement_id.clone(),
        DeclaredRequirement::Connection(requirement) => requirement.requirement_id.clone(),
    };
    PolicyDecision {
        allow: false,
        reason_code,
        declared_requirement_id,
        evaluated_permissions,
        resolved_permission: None,
        required_scope,
        action: input.action,
        role_snapshot: input.role,
        authentication_kind: input.authentication_kind,
        matched_constraints: vec![],
        failed_constraint,
        response_projection: None,
        target_summary: summarize_target(input),
        policy_revision: V1_POLICY_REVISION.to_owned(),
    }
}

#[derive(Debug)]
struct ConstraintResult {
    allowed: bool,
    matched: Vec<TargetConstraint>,
    failed: Option<TargetConstraint>,
}

fn evaluate_constraint(expression: &ConstraintExpr, input: &PolicyInput) -> ConstraintResult {
    match expression {
        ConstraintExpr::Atom(constraint) => {
            let allowed = match constraint {
                TargetConstraint::Any => true,
                TargetConstraint::SelfActor => input.target.target_id == Some(input.actor_id),
                TargetConstraint::OwnerActor => input.target.owner_id == Some(input.actor_id),
                TargetConstraint::EffectiveAudience => {
                    input.target.effective_audience == Some(true)
                }
                TargetConstraint::SameProject => {
                    input.actor_project_id.is_some()
                        && input.actor_project_id == input.target.project_id
                }
                TargetConstraint::AggregateOnly => input.aggregate_only == Some(true),
                TargetConstraint::ResourceKindIn { values } => input
                    .target
                    .resource_kind
                    .is_some_and(|kind| values.contains(&kind)),
                TargetConstraint::LifecycleIn { values } => input
                    .target
                    .lifecycle
                    .is_some_and(|lifecycle| values.contains(&lifecycle)),
            };
            ConstraintResult {
                allowed,
                matched: if allowed {
                    vec![constraint.clone()]
                } else {
                    vec![]
                },
                failed: (!allowed).then(|| constraint.clone()),
            }
        }
        ConstraintExpr::AllOf(items) => {
            if items.is_empty() {
                return ConstraintResult {
                    allowed: false,
                    matched: vec![],
                    failed: None,
                };
            }
            let mut matched = vec![];
            for item in items {
                let result = evaluate_constraint(item, input);
                if !result.allowed {
                    return ConstraintResult {
                        allowed: false,
                        matched,
                        failed: result.failed,
                    };
                }
                matched.extend(result.matched);
            }
            ConstraintResult {
                allowed: true,
                matched,
                failed: None,
            }
        }
        ConstraintExpr::AnyOf(items) => {
            if items.is_empty() {
                return ConstraintResult {
                    allowed: false,
                    matched: vec![],
                    failed: None,
                };
            }
            let mut first_failure = None;
            for item in items {
                let result = evaluate_constraint(item, input);
                if result.allowed {
                    return result;
                }
                if first_failure.is_none() {
                    first_failure = result.failed;
                }
            }
            ConstraintResult {
                allowed: false,
                matched: vec![],
                failed: first_failure,
            }
        }
    }
}

fn allow_reason(matched: &[TargetConstraint]) -> DecisionReason {
    if matched.contains(&TargetConstraint::SelfActor) {
        DecisionReason::AllowSelf
    } else if matched.contains(&TargetConstraint::OwnerActor) {
        DecisionReason::AllowOwner
    } else if matched.contains(&TargetConstraint::EffectiveAudience) {
        DecisionReason::AllowAudience
    } else {
        DecisionReason::AllowRole
    }
}

fn reason_for_failed_constraint(constraint: &TargetConstraint) -> DecisionReason {
    match constraint {
        TargetConstraint::Any => DecisionReason::DenyUnknownPolicy,
        TargetConstraint::SelfActor => DecisionReason::DenyNotSelf,
        TargetConstraint::OwnerActor => DecisionReason::DenyNotOwner,
        TargetConstraint::EffectiveAudience => DecisionReason::DenyOutsideAudience,
        TargetConstraint::SameProject => DecisionReason::DenyCrossProject,
        TargetConstraint::AggregateOnly => DecisionReason::DenyDetailAccess,
        TargetConstraint::ResourceKindIn { .. } => DecisionReason::DenyKind,
        TargetConstraint::LifecycleIn { .. } => DecisionReason::DenyLifecycle,
    }
}

fn summarize_target(input: &PolicyInput) -> AuthorizationTargetSummary {
    AuthorizationTargetSummary {
        target_type: input.target.target_type,
        target_id: input.target.target_id,
        project_id: input.target.project_id,
        resource_kind: input.target.resource_kind,
        lifecycle: input.target.lifecycle,
        self_actor: input
            .target
            .target_id
            .map(|target_id| target_id == input.actor_id),
        owner_actor: input
            .target
            .owner_id
            .map(|owner_id| owner_id == input.actor_id),
        effective_audience: input.target.effective_audience,
        same_project: match (input.actor_project_id, input.target.project_id) {
            (Some(actor), Some(target)) => Some(actor == target),
            _ => None,
        },
        aggregate_only: input.aggregate_only,
    }
}

fn grant_for_role(role: PrimaryRole, permission: PermissionKey) -> Option<PermissionGrant> {
    use PermissionKey as P;
    use PrimaryRole as R;

    let allowed = match permission {
        P::AuthorizationGrantsReadSelf
        | P::SessionSelfRead
        | P::SessionPasswordChange
        | P::ProjectBrandingRead
        | P::MemberPrivateReadSelf
        | P::ResourceConsume
        | P::ResourceFeedbackSubmit
        | P::TelemetryMemberReadSelf
        | P::ConnectionTokenIssueSelf
        | P::ConnectionTokenReadSelf
        | P::ConnectionTokenRevokeSelf => true,

        P::ProjectDashboardRead
        | P::MemberDirectoryRead
        | P::TelemetryProjectRead
        | P::TaxonomyRead
        | P::ResourceAuthor
        | P::ResourceAccessManage
        | P::ResourceLifecycleManage
        | P::ResourceReleaseNonExecutable
        | P::ResourceMonitoringAggregateRead
        | P::ResourceFeedbackRead
        | P::AnalyticsViewRead
        | P::AnalyticsViewManageSelf => matches!(role, R::Admin | R::Contribute),

        P::ProjectSettingsRead
        | P::ProjectSettingsManage
        | P::MemberManage
        | P::MemberPrivateReadAny
        | P::TelemetryMemberReadAny
        | P::TaxonomyDefinitionManage
        | P::MemberTagAssignmentManage
        | P::ResourceReleaseRestricted
        | P::ResourceMonitoringMemberDetailRead
        | P::AnalyticsViewManageAny
        | P::ConnectionTokenReadAny
        | P::ConnectionTokenRevokeAny
        | P::AuditRead
        | P::AuditExport => role == R::Admin,
    };
    if !allowed {
        return None;
    }

    let constraints = match permission {
        P::AuthorizationGrantsReadSelf
        | P::SessionSelfRead
        | P::SessionPasswordChange
        | P::MemberPrivateReadSelf
        | P::TelemetryMemberReadSelf => ConstraintExpr::atom(TargetConstraint::SelfActor),

        P::ConnectionTokenIssueSelf | P::ConnectionTokenReadSelf | P::ConnectionTokenRevokeSelf => {
            ConstraintExpr::atom(TargetConstraint::OwnerActor)
        }

        P::ResourceConsume | P::ResourceFeedbackSubmit | P::AnalyticsViewRead => {
            ConstraintExpr::atom(TargetConstraint::EffectiveAudience)
        }

        P::ResourceAuthor
        | P::ResourceAccessManage
        | P::ResourceFeedbackRead
        | P::ResourceMonitoringAggregateRead
            if role == R::Contribute =>
        {
            ConstraintExpr::atom(TargetConstraint::OwnerActor)
        }

        P::AnalyticsViewManageSelf => ConstraintExpr::atom(TargetConstraint::OwnerActor),

        P::ResourceLifecycleManage if role == R::Contribute => ConstraintExpr::AllOf(vec![
            ConstraintExpr::atom(TargetConstraint::OwnerActor),
            ConstraintExpr::atom(TargetConstraint::ResourceKindIn {
                values: NonEmptySet::try_new(vec![ResourceKind::Agent, ResourceKind::Skill])
                    .expect("fixed resource-kind grant is non-empty and unique"),
            }),
        ]),

        P::ResourceReleaseNonExecutable => {
            let kind = ConstraintExpr::atom(TargetConstraint::ResourceKindIn {
                values: NonEmptySet::try_new(vec![ResourceKind::Agent, ResourceKind::Skill])
                    .expect("fixed resource-kind grant is non-empty and unique"),
            });
            if role == R::Contribute {
                ConstraintExpr::AllOf(vec![
                    ConstraintExpr::atom(TargetConstraint::OwnerActor),
                    kind,
                ])
            } else {
                kind
            }
        }

        P::ResourceReleaseRestricted => ConstraintExpr::atom(TargetConstraint::ResourceKindIn {
            values: NonEmptySet::try_new(vec![
                ResourceKind::Plugin,
                ResourceKind::Workflow,
                ResourceKind::Command,
            ])
            .expect("fixed resource-kind grant is non-empty and unique"),
        }),

        P::ProjectBrandingRead
        | P::ProjectDashboardRead
        | P::ProjectSettingsRead
        | P::ProjectSettingsManage
        | P::MemberDirectoryRead
        | P::MemberManage
        | P::MemberPrivateReadAny
        | P::TelemetryProjectRead
        | P::TelemetryMemberReadAny
        | P::TaxonomyRead
        | P::TaxonomyDefinitionManage
        | P::MemberTagAssignmentManage
        | P::AnalyticsViewManageAny
        | P::ConnectionTokenReadAny
        | P::ConnectionTokenRevokeAny
        | P::AuditRead
        | P::AuditExport => ConstraintExpr::atom(TargetConstraint::SameProject),

        _ => ConstraintExpr::any(),
    };

    let response_projection = match (role, permission) {
        (R::Contribute, P::MemberDirectoryRead) => Some(ResponseProjection::DirectorySafe),
        (R::Contribute, P::ProjectDashboardRead | P::TelemetryProjectRead)
        | (_, P::ResourceMonitoringAggregateRead) => Some(ResponseProjection::AggregateOnly),
        _ => None,
    };

    Some(PermissionGrant {
        permission,
        constraints,
        response_projection,
    })
}
