use conductor_domain::{
    evaluate_policy, grants_for_role, role_has_permission, scope_is_role_compatible,
    AuthenticationKind, AuthorizationAction, AuthorizationTarget, ConnectionPolicyRequirement,
    ConstraintExpr, DecisionReason, DeclaredRequirement, LifecycleState, NonEmptySet,
    PermissionAlternative, PermissionKey, PolicyInput, PolicyRequirement, PrimaryRole,
    ResourceKind, SecretScope, TargetConstraint, TargetType, UserStatus, V1_POLICY_REVISION,
};
use serde_json::json;
use uuid::Uuid;

fn allowed_roles(permission: PermissionKey) -> &'static [PrimaryRole] {
    use PermissionKey as P;
    use PrimaryRole as R;

    match permission {
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
        | P::ConnectionTokenRevokeSelf => &[R::Admin, R::Contribute, R::User],

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
        | P::AnalyticsViewManageSelf => &[R::Admin, R::Contribute],

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
        | P::AuditExport => &[R::Admin],
    }
}

#[test]
fn full_v1_role_permission_matrix_is_exact() {
    for permission in PermissionKey::ALL {
        for role in PrimaryRole::ALL {
            assert_eq!(
                role_has_permission(role, *permission),
                allowed_roles(*permission).contains(&role),
                "{} for {}",
                permission,
                role.as_str()
            );
        }
    }

    for role in PrimaryRole::ALL {
        let grants = grants_for_role(role);
        assert_eq!(
            grants.len(),
            PermissionKey::ALL
                .iter()
                .filter(|permission| allowed_roles(**permission).contains(&role))
                .count()
        );
        assert!(grants
            .iter()
            .all(|grant| role_has_permission(role, grant.permission)));
    }
}

#[test]
fn self_and_ordered_any_of_resolve_deterministically() {
    let actor_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let requirement = PolicyRequirement::new(
        "member.private.read",
        vec![
            PermissionAlternative::new(
                PermissionKey::MemberPrivateReadSelf,
                ConstraintExpr::atom(TargetConstraint::SelfActor),
            ),
            PermissionAlternative::new(
                PermissionKey::MemberPrivateReadAny,
                ConstraintExpr::atom(TargetConstraint::SameProject),
            ),
        ],
    )
    .unwrap();

    let decision = evaluate_policy(&PolicyInput {
        actor_id,
        actor_project_id: Some(project_id),
        role: PrimaryRole::Admin,
        status: UserStatus::Active,
        authentication_kind: AuthenticationKind::BrowserSession,
        requirement: DeclaredRequirement::Browser(requirement),
        action: AuthorizationAction::MemberPrivateRead,
        expected_target_type: TargetType::Project,
        target: target(Some(project_id), Some(actor_id), None, None, None, None),
        aggregate_only: None,
        credential_scopes: vec![],
    });

    assert!(decision.allow);
    assert_eq!(decision.reason_code, DecisionReason::AllowSelf);
    assert_eq!(
        decision.resolved_permission,
        Some(PermissionKey::MemberPrivateReadSelf)
    );
}

#[test]
fn compound_owner_and_kind_policy_fails_closed() {
    let actor_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let make_input = |owner_id, kind| PolicyInput {
        actor_id,
        actor_project_id: Some(project_id),
        role: PrimaryRole::Contribute,
        status: UserStatus::Active,
        authentication_kind: AuthenticationKind::BrowserSession,
        requirement: DeclaredRequirement::Browser(
            PolicyRequirement::new(
                "resource.release",
                vec![PermissionAlternative::new(
                    PermissionKey::ResourceReleaseNonExecutable,
                    ConstraintExpr::any(),
                )],
            )
            .unwrap(),
        ),
        action: AuthorizationAction::ResourceRelease,
        expected_target_type: TargetType::Resource,
        target: target(
            Some(project_id),
            Some(Uuid::new_v4()),
            owner_id,
            Some(kind),
            Some(LifecycleState::Draft),
            Some(true),
        ),
        aggregate_only: None,
        credential_scopes: vec![],
    };

    let owned_agent = evaluate_policy(&make_input(Some(actor_id), ResourceKind::Agent));
    assert!(owned_agent.allow);
    assert_eq!(owned_agent.reason_code, DecisionReason::AllowOwner);

    let foreign_agent = evaluate_policy(&make_input(Some(Uuid::new_v4()), ResourceKind::Agent));
    assert!(!foreign_agent.allow);
    assert_eq!(foreign_agent.reason_code, DecisionReason::DenyNotOwner);

    let owned_plugin = evaluate_policy(&make_input(Some(actor_id), ResourceKind::Plugin));
    assert!(!owned_plugin.allow);
    assert_eq!(owned_plugin.reason_code, DecisionReason::DenyKind);

    let admin_wrong_permission = browser_decision(
        actor_id,
        project_id,
        PrimaryRole::Admin,
        PermissionKey::ResourceReleaseNonExecutable,
        AuthorizationAction::ResourceRelease,
        target(
            Some(project_id),
            Some(Uuid::new_v4()),
            Some(actor_id),
            Some(ResourceKind::Plugin),
            Some(LifecycleState::Draft),
            Some(true),
        ),
        ConstraintExpr::any(),
    );
    assert!(!admin_wrong_permission.allow);
    assert_eq!(admin_wrong_permission.reason_code, DecisionReason::DenyKind);
}

#[test]
fn contributor_cannot_release_restricted_resource_kinds() {
    let actor_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    for kind in [
        ResourceKind::Plugin,
        ResourceKind::Workflow,
        ResourceKind::Command,
    ] {
        let decision = browser_decision(
            actor_id,
            project_id,
            PrimaryRole::Contribute,
            PermissionKey::ResourceReleaseRestricted,
            AuthorizationAction::ResourceRelease,
            target(
                Some(project_id),
                Some(Uuid::new_v4()),
                Some(actor_id),
                Some(kind),
                Some(LifecycleState::Draft),
                Some(true),
            ),
            ConstraintExpr::any(),
        );
        assert!(!decision.allow, "Contributor released {kind:?}");
        assert_eq!(decision.reason_code, DecisionReason::DenyRole);
    }
}

#[test]
fn audience_project_and_lifecycle_constraints_are_target_aware() {
    let actor_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let other_project = Uuid::new_v4();

    let visible = browser_decision(
        actor_id,
        project_id,
        PrimaryRole::User,
        PermissionKey::ResourceConsume,
        AuthorizationAction::ResourcesList,
        target(
            Some(project_id),
            Some(Uuid::new_v4()),
            None,
            Some(ResourceKind::Skill),
            Some(LifecycleState::Published),
            Some(true),
        ),
        ConstraintExpr::atom(TargetConstraint::LifecycleIn {
            values: NonEmptySet::try_new(vec![LifecycleState::Published]).unwrap(),
        }),
    );
    assert!(visible.allow);

    let outside_audience = browser_decision(
        actor_id,
        project_id,
        PrimaryRole::User,
        PermissionKey::ResourceConsume,
        AuthorizationAction::ResourcesList,
        target(
            Some(project_id),
            Some(Uuid::new_v4()),
            None,
            Some(ResourceKind::Skill),
            Some(LifecycleState::Published),
            Some(false),
        ),
        ConstraintExpr::any(),
    );
    assert_eq!(
        outside_audience.reason_code,
        DecisionReason::DenyOutsideAudience
    );

    let cross_project = browser_decision(
        actor_id,
        project_id,
        PrimaryRole::Admin,
        PermissionKey::ProjectSettingsRead,
        AuthorizationAction::ProjectSettingsRead,
        target(
            Some(other_project),
            Some(other_project),
            None,
            None,
            None,
            None,
        ),
        ConstraintExpr::any(),
    );
    assert_eq!(cross_project.reason_code, DecisionReason::DenyCrossProject);

    let wrong_lifecycle = browser_decision(
        actor_id,
        project_id,
        PrimaryRole::User,
        PermissionKey::ResourceConsume,
        AuthorizationAction::ResourcesList,
        target(
            Some(project_id),
            Some(Uuid::new_v4()),
            None,
            Some(ResourceKind::Skill),
            Some(LifecycleState::Draft),
            Some(true),
        ),
        ConstraintExpr::atom(TargetConstraint::LifecycleIn {
            values: NonEmptySet::try_new(vec![LifecycleState::Published]).unwrap(),
        }),
    );
    assert_eq!(wrong_lifecycle.reason_code, DecisionReason::DenyLifecycle);
}

#[test]
fn inactive_or_invalid_policy_never_grants() {
    let actor_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let mut input = basic_browser_input(
        actor_id,
        project_id,
        PrimaryRole::Admin,
        PermissionKey::ProjectDashboardRead,
        AuthorizationAction::ProjectDashboardRead,
    );
    input.status = UserStatus::Disabled;
    assert_eq!(
        evaluate_policy(&input).reason_code,
        DecisionReason::DenyInactivePrincipal
    );

    input.status = UserStatus::Active;
    input.requirement = DeclaredRequirement::Browser(PolicyRequirement {
        requirement_id: "".into(),
        alternatives: vec![],
    });
    assert_eq!(
        evaluate_policy(&input).reason_code,
        DecisionReason::DenyUnknownPolicy
    );
}

#[test]
fn a_target_type_different_from_the_sealed_route_type_is_denied() {
    let actor_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let mut input = basic_browser_input(
        actor_id,
        project_id,
        PrimaryRole::Admin,
        PermissionKey::ProjectDashboardRead,
        AuthorizationAction::ProjectDashboardRead,
    );
    input.target.target_type = TargetType::Member;

    let decision = evaluate_policy(&input);
    assert!(!decision.allow);
    assert_eq!(decision.reason_code, DecisionReason::DenyTargetType);
}

#[test]
fn sub_roles_and_tags_never_grant_endpoint_permission() {
    let grants = serde_json::to_value(grants_for_role(PrimaryRole::User)).unwrap();
    let wire = grants.to_string();
    assert!(!wire.contains("sub_role"));
    assert!(!wire.contains("tag"));
    assert!(!role_has_permission(
        PrimaryRole::User,
        PermissionKey::TaxonomyRead
    ));
    assert!(!role_has_permission(
        PrimaryRole::User,
        PermissionKey::MemberTagAssignmentManage
    ));
}

#[test]
fn all_current_machine_scopes_are_role_compatible() {
    for role in PrimaryRole::ALL {
        for scope in SecretScope::ALL {
            assert!(scope_is_role_compatible(role, scope));
        }
    }
    assert_eq!(SecretScope::parse("read_documents"), None);
    assert_eq!(SecretScope::parse("unknown"), None);
}

#[test]
fn connection_scope_and_target_are_both_required() {
    let actor_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let requirement = ConnectionPolicyRequirement {
        requirement_id: "client.resources.snapshot".into(),
        required_scope: SecretScope::SubscribeResources,
        constraints: ConstraintExpr::atom(TargetConstraint::EffectiveAudience),
    };
    let mut input = PolicyInput {
        actor_id,
        actor_project_id: Some(project_id),
        role: PrimaryRole::User,
        status: UserStatus::Active,
        authentication_kind: AuthenticationKind::ConnectionToken,
        requirement: DeclaredRequirement::Connection(requirement),
        action: AuthorizationAction::ClientResourcesSnapshot,
        expected_target_type: TargetType::Resource,
        target: target(
            Some(project_id),
            Some(Uuid::new_v4()),
            None,
            Some(ResourceKind::Agent),
            Some(LifecycleState::Published),
            Some(true),
        ),
        aggregate_only: None,
        credential_scopes: vec![],
    };

    assert_eq!(
        evaluate_policy(&input).reason_code,
        DecisionReason::DenyScope
    );
    input.credential_scopes = vec![SecretScope::SubscribeResources];
    assert!(evaluate_policy(&input).allow);
    input.target.effective_audience = Some(false);
    assert_eq!(
        evaluate_policy(&input).reason_code,
        DecisionReason::DenyOutsideAudience
    );
}

#[test]
fn stable_wire_names_are_exact_and_unknown_values_fail() {
    assert_eq!(V1_POLICY_REVISION, "req-004-v1");
    assert_eq!(
        serde_json::to_value(PermissionKey::ResourceReleaseRestricted).unwrap(),
        json!("resource.release.restricted")
    );
    assert_eq!(
        serde_json::to_value(AuthorizationAction::MemberAccessProfileUpdate).unwrap(),
        json!("member.access_profile.update")
    );
    assert_eq!(
        serde_json::to_value(DecisionReason::DenyCrossProject).unwrap(),
        json!("deny_cross_project")
    );
    assert!(serde_json::from_value::<PermissionKey>(json!("new.permission")).is_err());
}

#[test]
fn authorization_decision_serializes_only_safe_fields() {
    let actor_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let decision = browser_decision(
        actor_id,
        project_id,
        PrimaryRole::Admin,
        PermissionKey::ProjectDashboardRead,
        AuthorizationAction::ProjectDashboardRead,
        target(Some(project_id), Some(project_id), None, None, None, None),
        ConstraintExpr::any(),
    );
    let wire = serde_json::to_string(&decision).unwrap();
    for canary in [
        "canary.jwt.value",
        "evc_canary_token",
        "Authorization: Bearer",
        "canary request body",
        "private work content",
        "user@example.com",
    ] {
        assert!(!wire.contains(canary));
    }
    let object = serde_json::to_value(decision).unwrap();
    let object = object.as_object().unwrap();
    assert!(!object.contains_key("headers"));
    assert!(!object.contains_key("body"));
    assert!(!object.contains_key("email"));
    assert!(!object.contains_key("token"));
    assert!(!object.contains_key("metadata"));
}

fn basic_browser_input(
    actor_id: Uuid,
    project_id: Uuid,
    role: PrimaryRole,
    permission: PermissionKey,
    action: AuthorizationAction,
) -> PolicyInput {
    PolicyInput {
        actor_id,
        actor_project_id: Some(project_id),
        role,
        status: UserStatus::Active,
        authentication_kind: AuthenticationKind::BrowserSession,
        requirement: DeclaredRequirement::Browser(
            PolicyRequirement::new(
                "test.requirement",
                vec![PermissionAlternative::new(
                    permission,
                    ConstraintExpr::any(),
                )],
            )
            .unwrap(),
        ),
        action,
        expected_target_type: TargetType::Project,
        target: target(
            Some(project_id),
            Some(project_id),
            None,
            None,
            None,
            Some(true),
        ),
        aggregate_only: None,
        credential_scopes: vec![],
    }
}

fn browser_decision(
    actor_id: Uuid,
    project_id: Uuid,
    role: PrimaryRole,
    permission: PermissionKey,
    action: AuthorizationAction,
    target: AuthorizationTarget,
    constraints: ConstraintExpr,
) -> conductor_domain::PolicyDecision {
    evaluate_policy(&PolicyInput {
        actor_id,
        actor_project_id: Some(project_id),
        role,
        status: UserStatus::Active,
        authentication_kind: AuthenticationKind::BrowserSession,
        requirement: DeclaredRequirement::Browser(
            PolicyRequirement::new(
                "test.requirement",
                vec![PermissionAlternative::new(permission, constraints)],
            )
            .unwrap(),
        ),
        action,
        expected_target_type: target.target_type,
        target,
        aggregate_only: None,
        credential_scopes: vec![],
    })
}

fn target(
    project_id: Option<Uuid>,
    target_id: Option<Uuid>,
    owner_id: Option<Uuid>,
    resource_kind: Option<ResourceKind>,
    lifecycle: Option<LifecycleState>,
    effective_audience: Option<bool>,
) -> AuthorizationTarget {
    AuthorizationTarget {
        project_id,
        target_type: if resource_kind.is_some() {
            TargetType::Resource
        } else {
            TargetType::Project
        },
        target_id,
        owner_id,
        resource_kind,
        lifecycle,
        effective_audience,
    }
}
