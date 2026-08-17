mod access;
mod analytics_views;
mod auth;
mod client;
mod dashboard;
mod health;
mod realtime;
mod resource_delivery;
mod resources;
mod secrets;
mod settings;
mod setup;
mod sso;
mod telemetry;
mod users;

use axum::Router;
use conductor_domain::{
    AuthorizationAction as A, PermissionKey as P, ResponseProjection, SecretScope as Scope,
    TargetType as Target,
};

use crate::core::resource_authoring::MAX_IMPORT_ARCHIVE_BYTES;
use crate::core::state::AppState;
use crate::http::authorization::{
    BrowserRouteAlternative as Alternative, ClassifiedRouter, ManifestCollector, RouteDefinition,
    RouteManifest, RouteRegistrar, RouteTargetSelector as Selector, RouteTransport, MAX_LOGO_BYTES,
};

pub fn router(state: AppState) -> Router {
    let mut routes = ClassifiedRouter::new(state);
    declare_routes(&mut routes);
    routes.finish()
}

pub(crate) fn manifest() -> RouteManifest {
    let mut routes = ManifestCollector::default();
    declare_routes(&mut routes);
    routes.finish()
}

/// The only production declaration of API method/path actions.
///
/// `RouteRegistrar` generates both the Axum router and the review manifest, so
/// authentication class, permission/scope, selector and transport metadata
/// cannot drift from route registration.
fn declare_routes(routes: &mut impl RouteRegistrar) {
    routes.get(
        "/health",
        RouteDefinition::public(A::HealthRead, Target::Project),
        health::health,
    );
    routes.get(
        "/setup/status",
        RouteDefinition::public(A::SetupStatusRead, Target::Project),
        setup::status,
    );
    routes.post(
        "/setup",
        RouteDefinition::bootstrap(A::SetupComplete, Target::Project),
        setup::complete,
    );
    routes.post(
        "/auth/login",
        RouteDefinition::public(A::AuthLogin, Target::Session),
        auth::login,
    );
    routes.get(
        "/auth/sso/start",
        RouteDefinition::public(A::AuthSsoStart, Target::Session),
        auth::sso_start,
    );
    routes.get(
        "/auth/sso/callback",
        RouteDefinition::public(A::AuthSsoCallback, Target::Session),
        auth::sso_callback,
    );
    routes.get(
        "/project/logo",
        RouteDefinition::public(A::ProjectLogoRead, Target::Project),
        settings::get_project_logo,
    );

    routes.get(
        "/auth/me",
        browser(
            A::SessionSelfRead,
            Target::Session,
            P::SessionSelfRead,
            self_actor(),
        ),
        auth::me,
    );
    routes.post(
        "/auth/change-password",
        browser(
            A::SessionPasswordChange,
            Target::Session,
            P::SessionPasswordChange,
            self_actor(),
        ),
        auth::change_password,
    );
    routes.get(
        "/authorization/me",
        browser(
            A::AuthorizationGrantsReadSelf,
            Target::Session,
            P::AuthorizationGrantsReadSelf,
            self_actor(),
        ),
        auth::authorization_me,
    );
    routes.get(
        "/sso",
        browser(
            A::ProjectSsoRead,
            Target::Project,
            P::ProjectSettingsRead,
            project_member(),
        ),
        sso::get_config,
    );
    routes.put(
        "/sso",
        browser(
            A::ProjectSsoUpdate,
            Target::Project,
            P::ProjectSettingsManage,
            project_member(),
        ),
        settings::update_sso,
    );
    routes.get(
        "/project",
        browser(
            A::ProjectBrandingRead,
            Target::Project,
            P::ProjectBrandingRead,
            project_member(),
        ),
        settings::get_project,
    );
    routes.get(
        "/settings",
        browser(
            A::ProjectSettingsRead,
            Target::Project,
            P::ProjectSettingsRead,
            project_member(),
        ),
        settings::get_settings,
    );
    routes.patch(
        "/settings",
        browser(
            A::ProjectSettingsUpdate,
            Target::Project,
            P::ProjectSettingsManage,
            project_member(),
        ),
        settings::update_settings,
    );
    routes.put(
        "/settings/network",
        browser(
            A::ProjectNetworkUpdate,
            Target::Project,
            P::ProjectSettingsManage,
            project_member(),
        ),
        settings::update_network,
    );
    routes.put(
        "/settings/data-policy",
        browser(
            A::ProjectDataPolicyUpdate,
            Target::Project,
            P::ProjectSettingsManage,
            project_member(),
        ),
        settings::update_data_policy,
    );
    routes.put(
        "/settings/storage",
        browser(
            A::ProjectStorageUpdate,
            Target::Project,
            P::ProjectSettingsManage,
            project_member(),
        ),
        settings::update_storage,
    );
    routes.put(
        "/settings/logo",
        browser(
            A::ProjectLogoUpload,
            Target::Project,
            P::ProjectSettingsManage,
            project_member(),
        )
        .with_transport(RouteTransport::body_limit(MAX_LOGO_BYTES)),
        settings::upload_logo,
    );
    routes.delete(
        "/settings/logo",
        browser(
            A::ProjectLogoDelete,
            Target::Project,
            P::ProjectSettingsManage,
            project_member(),
        ),
        settings::delete_logo,
    );
    routes.get(
        "/dashboard",
        browser(
            A::ProjectDashboardRead,
            Target::Project,
            P::ProjectDashboardRead,
            Selector::AggregateOnly,
        ),
        dashboard::summary,
    );

    routes.get(
        "/members",
        browser(
            A::MemberDirectoryList,
            Target::Member,
            P::MemberDirectoryRead,
            project_member(),
        ),
        users::list,
    );
    routes.post(
        "/members",
        browser(
            A::MemberCreate,
            Target::Member,
            P::MemberManage,
            project_member(),
        ),
        users::create,
    );
    routes.get(
        "/members/pending/count",
        browser(
            A::MemberPendingCountRead,
            Target::Member,
            P::MemberManage,
            project_member(),
        ),
        users::pending_count,
    );
    routes.get(
        "/members/{id}",
        browser_alternatives(
            A::MemberPrivateRead,
            Target::Member,
            vec![
                alt(P::MemberPrivateReadSelf, Selector::SelfMemberPath),
                alt(P::MemberPrivateReadAny, Selector::OtherMemberPath),
            ],
        ),
        users::get,
    );
    routes.patch(
        "/members/{id}",
        browser(
            A::MemberAccessProfileUpdate,
            Target::Member,
            P::MemberManage,
            Selector::AnyMemberPath,
        ),
        users::update,
    );
    routes.get(
        "/members/{id}/installations",
        browser_alternatives(
            A::MemberInstallationsList,
            Target::ClientInstallation,
            vec![
                alt(P::MemberPrivateReadSelf, Selector::SelfMemberPath),
                alt(P::MemberPrivateReadAny, Selector::OtherMemberPath),
            ],
        ),
        client::list_member_installations,
    );
    routes.get(
        "/members/{id}/secrets",
        browser_alternatives(
            A::MemberConnectionTokensList,
            Target::ConnectionToken,
            vec![
                alt(P::ConnectionTokenReadSelf, Selector::SelfMemberPath),
                alt(P::ConnectionTokenReadAny, Selector::OtherMemberPath),
            ],
        ),
        secrets::list_for_member,
    );
    routes.post(
        "/members/{id}/secrets",
        browser(
            A::MemberConnectionTokenIssueSelf,
            Target::ConnectionToken,
            P::ConnectionTokenIssueSelf,
            Selector::SelfMemberPath,
        ),
        secrets::create_for_member,
    );
    routes.post(
        "/members/{id}/secrets/{secret_id}/revoke",
        browser_alternatives(
            A::MemberConnectionTokenRevoke,
            Target::ConnectionToken,
            vec![
                alt(
                    P::ConnectionTokenRevokeSelf,
                    all([Selector::SelfMemberPath, Selector::MemberSecretPath]),
                ),
                alt(
                    P::ConnectionTokenRevokeAny,
                    all([Selector::OtherMemberPath, Selector::MemberSecretPath]),
                ),
            ],
        ),
        secrets::revoke_for_member,
    );
    routes.post(
        "/members/{id}/approve",
        browser(
            A::MemberApprove,
            Target::Member,
            P::MemberManage,
            Selector::AnyMemberPath,
        ),
        users::approve,
    );
    routes.post(
        "/members/{id}/disable",
        browser(
            A::MemberDisable,
            Target::Member,
            P::MemberManage,
            Selector::AnyMemberPath,
        ),
        users::disable,
    );
    routes.post(
        "/members/{id}/enable",
        browser(
            A::MemberEnable,
            Target::Member,
            P::MemberManage,
            Selector::AnyMemberPath,
        ),
        users::enable,
    );
    routes.post(
        "/members/{id}/reset-password",
        browser(
            A::MemberPasswordReset,
            Target::Member,
            P::MemberManage,
            Selector::AnyMemberPath,
        ),
        users::reset_password,
    );

    routes.get(
        "/sub-roles",
        browser(
            A::TaxonomySubRolesList,
            Target::Taxonomy,
            P::TaxonomyRead,
            project_member(),
        ),
        access::list_sub_roles,
    );
    routes.post(
        "/sub-roles",
        browser(
            A::TaxonomySubRoleCreate,
            Target::Taxonomy,
            P::TaxonomyDefinitionManage,
            project_member(),
        ),
        access::create_sub_role,
    );
    routes.patch(
        "/sub-roles/{id}",
        browser(
            A::TaxonomySubRoleUpdate,
            Target::Taxonomy,
            P::TaxonomyDefinitionManage,
            project_member(),
        ),
        access::update_sub_role,
    );
    routes.delete(
        "/sub-roles/{id}",
        browser(
            A::TaxonomySubRoleDelete,
            Target::Taxonomy,
            P::TaxonomyDefinitionManage,
            project_member(),
        ),
        access::delete_sub_role,
    );
    routes.get(
        "/tags",
        browser(
            A::TaxonomyTagsList,
            Target::Taxonomy,
            P::TaxonomyRead,
            project_member(),
        ),
        access::list_tags,
    );
    routes.post(
        "/tags",
        browser(
            A::TaxonomyTagCreate,
            Target::Taxonomy,
            P::TaxonomyDefinitionManage,
            project_member(),
        ),
        access::create_tag,
    );
    routes.patch(
        "/tags/{id}",
        browser(
            A::TaxonomyTagUpdate,
            Target::Taxonomy,
            P::TaxonomyDefinitionManage,
            project_member(),
        ),
        access::update_tag,
    );
    routes.delete(
        "/tags/{id}",
        browser(
            A::TaxonomyTagDelete,
            Target::Taxonomy,
            P::TaxonomyDefinitionManage,
            project_member(),
        ),
        access::delete_tag,
    );
    routes.get(
        "/tag-assignments/{entity_type}/{entity_id}",
        taxonomy_assignment(A::TaxonomyAssignmentRead),
        access::get_entity_tags,
    );
    routes.put(
        "/tag-assignments/{entity_type}/{entity_id}",
        taxonomy_assignment(A::TaxonomyAssignmentSet),
        access::set_entity_tags,
    );

    routes.get(
        "/secrets",
        browser(
            A::ConnectionTokensSelfList,
            Target::ConnectionToken,
            P::ConnectionTokenReadSelf,
            self_actor(),
        ),
        secrets::list,
    );
    routes.post(
        "/secrets",
        browser(
            A::ConnectionTokensSelfIssue,
            Target::ConnectionToken,
            P::ConnectionTokenIssueSelf,
            self_actor(),
        ),
        secrets::create,
    );
    routes.post(
        "/secrets/{id}/revoke",
        browser(
            A::ConnectionTokensSelfRevoke,
            Target::ConnectionToken,
            P::ConnectionTokenRevokeSelf,
            Selector::SecretOwnerPath,
        ),
        secrets::revoke,
    );

    routes.get(
        "/resources",
        browser(
            A::ResourcesList,
            Target::Resource,
            P::ResourceConsume,
            Selector::EffectiveAudienceList,
        ),
        resources::list,
    );
    routes.post(
        "/resources",
        browser(
            A::ResourceCreate,
            Target::Resource,
            P::ResourceAuthor,
            Selector::NewResourceOwnerActor,
        ),
        resources::create,
    );
    routes.post(
        "/resources/plugins/inspect",
        browser(
            A::ResourcePluginArchiveInspect,
            Target::Resource,
            P::ResourceAuthor,
            all([Selector::NewResourceOwnerActor, Selector::KindPlugin]),
        )
        .with_transport(RouteTransport::body_limit(MAX_IMPORT_ARCHIVE_BYTES)),
        resource_delivery::inspect_plugin_archive,
    );
    routes.post(
        "/resources/plugins/import",
        browser(
            A::ResourcePluginArchiveImport,
            Target::Resource,
            P::ResourceAuthor,
            all([Selector::NewResourceOwnerActor, Selector::KindPlugin]),
        )
        .with_transport(RouteTransport::body_limit(MAX_IMPORT_ARCHIVE_BYTES)),
        resource_delivery::create_plugin_archive,
    );
    routes.post(
        "/resources/imports/{kind}/inspect",
        browser(
            A::ResourceArchiveInspect,
            Target::Resource,
            P::ResourceAuthor,
            all([
                Selector::NewResourceOwnerActor,
                Selector::KindPath,
                Selector::AgentOrSkill,
            ]),
        )
        .with_transport(RouteTransport::body_limit(MAX_IMPORT_ARCHIVE_BYTES)),
        resource_delivery::inspect_resource_archive,
    );
    routes.post(
        "/resources/imports/{kind}",
        browser(
            A::ResourceArchiveImport,
            Target::Resource,
            P::ResourceAuthor,
            all([
                Selector::NewResourceOwnerActor,
                Selector::KindPath,
                Selector::AgentOrSkill,
            ]),
        )
        .with_transport(RouteTransport::body_limit(MAX_IMPORT_ARCHIVE_BYTES)),
        resource_delivery::create_resource_archive,
    );
    routes.get(
        "/resources/guides/{kind}",
        browser(
            A::ResourceAuthoringGuideRead,
            Target::Resource,
            P::ResourceAuthor,
            all([Selector::NewResourceOwnerActor, Selector::KindPath]),
        ),
        resource_delivery::guide,
    );
    routes.get(
        "/resources/templates/{kind}",
        browser(
            A::ResourceAuthoringTemplateRead,
            Target::Resource,
            P::ResourceAuthor,
            all([Selector::NewResourceOwnerActor, Selector::KindPath]),
        ),
        resource_delivery::template,
    );
    routes.patch(
        "/resources/{id}",
        resource_owner(A::ResourceUpdate, P::ResourceAuthor),
        resources::update,
    );
    routes.post(
        "/resources/{id}/archive",
        resource_lifecycle(A::ResourceArchive),
        resources::archive,
    );
    routes.get(
        "/resources/{id}/draft/files",
        resource_owner(A::ResourceDraftTreeRead, P::ResourceAuthor),
        resource_delivery::draft_tree,
    );
    routes.put(
        "/resources/{id}/draft/files/{*path}",
        resource_owner(A::ResourceDraftFileSave, P::ResourceAuthor),
        resource_delivery::save_draft_file,
    );
    routes.post(
        "/resources/{id}/draft/entries",
        resource_owner(A::ResourceDraftEntryCreate, P::ResourceAuthor),
        resource_delivery::create_draft_file,
    );
    routes.patch(
        "/resources/{id}/draft/entries",
        resource_owner(A::ResourceDraftEntryMove, P::ResourceAuthor),
        resource_delivery::move_draft_entry,
    );
    routes.delete(
        "/resources/{id}/draft/entries",
        resource_owner(A::ResourceDraftEntryDelete, P::ResourceAuthor),
        resource_delivery::delete_draft_entry,
    );
    routes.post(
        "/resources/{id}/draft/import",
        resource_owner(A::ResourceDraftArchiveImport, P::ResourceAuthor)
            .with_transport(RouteTransport::body_limit(MAX_IMPORT_ARCHIVE_BYTES)),
        resource_delivery::import_archive,
    );
    routes.post(
        "/resources/{id}/draft/validate",
        resource_owner(A::ResourceDraftValidate, P::ResourceAuthor),
        resource_delivery::validate,
    );
    routes.post(
        "/resources/{id}/release",
        browser_alternatives(
            A::ResourceRelease,
            Target::Resource,
            vec![
                alt(
                    P::ResourceReleaseNonExecutable,
                    all([
                        Selector::InProjectResourcePath,
                        Selector::KindOfResourcePath,
                        Selector::AgentOrSkill,
                    ]),
                ),
                alt(
                    P::ResourceReleaseRestricted,
                    all([
                        Selector::InProjectResourcePath,
                        Selector::KindOfResourcePath,
                        Selector::RestrictedKind,
                    ]),
                ),
            ],
        ),
        resource_delivery::release,
    );
    routes.get(
        "/resources/{id}/versions",
        resource_owner(A::ResourceVersionsList, P::ResourceAuthor),
        resources::versions,
    );
    routes.post(
        "/resources/{id}/versions/{version_id}/deprecate",
        resource_lifecycle(A::ResourceVersionDeprecate),
        resources::deprecate_version,
    );
    routes.post(
        "/resources/{id}/versions/{version_id}/restore-to-draft",
        resource_lifecycle(A::ResourceVersionRestoreToDraft),
        resources::restore_version_to_draft,
    );
    routes.get(
        "/resources/{id}/access",
        resource_owner(A::ResourceAccessRead, P::ResourceAccessManage),
        resources::get_access,
    );
    routes.put(
        "/resources/{id}/access",
        resource_owner(A::ResourceAccessUpdate, P::ResourceAccessManage),
        resources::set_access,
    );
    routes.get(
        "/resources/{id}/monitoring",
        browser_alternatives(
            A::ResourceMonitoringRead,
            Target::Resource,
            vec![
                alt(
                    P::ResourceMonitoringMemberDetailRead,
                    all([Selector::InProjectResourcePath, Selector::MemberDetail]),
                ),
                alt(
                    P::ResourceMonitoringAggregateRead,
                    all([
                        Selector::InProjectResourcePath,
                        Selector::ResourceOwnerPath,
                        Selector::AggregateOnly,
                    ]),
                )
                .with_projection(ResponseProjection::AggregateOnly),
            ],
        ),
        resources::monitoring,
    );
    routes.get(
        "/resources/{id}/inventory",
        browser_alternatives(
            A::ResourceInventoryMonitoringRead,
            Target::Resource,
            vec![
                alt(
                    P::ResourceMonitoringMemberDetailRead,
                    all([Selector::InProjectResourcePath, Selector::MemberDetail]),
                ),
                alt(
                    P::ResourceMonitoringAggregateRead,
                    all([
                        Selector::InProjectResourcePath,
                        Selector::ResourceOwnerPath,
                        Selector::AggregateOnly,
                    ]),
                )
                .with_projection(ResponseProjection::AggregateOnly),
            ],
        ),
        resources::inventory_monitoring,
    );
    routes.get(
        "/resources/{id}/feedback",
        resource_owner(A::ResourceFeedbackList, P::ResourceFeedbackRead),
        resources::feedback,
    );
    routes.put(
        "/resources/{id}/feedback",
        browser(
            A::ResourceFeedbackSubmit,
            Target::Resource,
            P::ResourceFeedbackSubmit,
            Selector::VisibleResourcePath,
        ),
        resources::upsert_feedback,
    );

    routes.get(
        "/members/{id}/usage/summary",
        member_telemetry(A::MemberUsageSummaryRead),
        telemetry::usage_summary,
    );
    routes.get(
        "/members/{id}/activity",
        member_telemetry(A::MemberActivityList),
        telemetry::activity,
    );
    routes.get(
        "/members/{id}/activity/{request_id}",
        member_telemetry(A::MemberActivityDetailRead),
        telemetry::request_detail,
    );
    routes.get(
        "/members/{id}/tools",
        member_telemetry(A::MemberToolsSummaryRead),
        telemetry::tools_summary,
    );
    routes.get(
        "/analytics/resource-usage",
        browser_alternatives(
            A::AnalyticsResourceUsageRead,
            Target::Project,
            vec![
                alt(P::TelemetryMemberReadAny, Selector::MemberDetail),
                alt(P::TelemetryProjectRead, Selector::AggregateOnly)
                    .with_projection(ResponseProjection::AggregateOnly),
            ],
        ),
        telemetry::resource_usage,
    );
    routes.get(
        "/analytics/views",
        browser(
            A::AnalyticsViewsList,
            Target::AnalyticsView,
            P::AnalyticsViewRead,
            Selector::EffectiveAudienceList,
        ),
        analytics_views::list,
    );
    routes.post(
        "/analytics/views",
        browser(
            A::AnalyticsViewCreate,
            Target::AnalyticsView,
            P::AnalyticsViewManageSelf,
            self_actor(),
        ),
        analytics_views::create,
    );
    routes.get(
        "/analytics/views/{id}",
        browser(
            A::AnalyticsViewRead,
            Target::AnalyticsView,
            P::AnalyticsViewRead,
            Selector::VisibleAnalyticsViewPath,
        ),
        analytics_views::get,
    );
    routes.put(
        "/analytics/views/{id}",
        analytics_view_target(A::AnalyticsViewUpdate),
        analytics_views::update,
    );
    routes.delete(
        "/analytics/views/{id}",
        analytics_view_target(A::AnalyticsViewDelete),
        analytics_views::delete,
    );

    routes.get(
        "/v1/subscribe/resources",
        connection(
            A::ClientResourcesSnapshot,
            Target::Resource,
            Scope::SubscribeResources,
            Selector::EffectiveAudienceList,
        ),
        resources::subscribe,
    );
    routes.get(
        "/v1/resources/changes",
        connection(
            A::ClientResourcesChanges,
            Target::Resource,
            Scope::SubscribeResources,
            Selector::EffectiveAudienceList,
        ),
        resource_delivery::changes,
    );
    routes.post(
        "/v1/resources/fetch",
        connection(
            A::ClientResourcesFetch,
            Target::Resource,
            Scope::SubscribeResources,
            all([
                Selector::InstallationOwnerBody,
                Selector::EffectiveAudienceList,
            ]),
        ),
        resource_delivery::fetch,
    );
    routes.get(
        "/v1/resources/{id}/versions/{version_id}",
        connection(
            A::ClientResourceVersionRead,
            Target::Resource,
            Scope::SubscribeResources,
            Selector::EffectiveVersionPath,
        ),
        resource_delivery::version_payload,
    );
    routes.get(
        "/v1/resources/{id}/versions/{version_id}/artifact",
        connection(
            A::ClientResourceArtifactRead,
            Target::Resource,
            Scope::SubscribeResources,
            Selector::EffectiveVersionPath,
        ),
        resource_delivery::artifact,
    );
    routes.put(
        "/v1/client/inventory",
        connection(
            A::ClientInventorySync,
            Target::ClientInstallation,
            Scope::SyncInventory,
            all([
                Selector::InstallationOwnerBody,
                Selector::InventoryItemsVisibleBody,
            ]),
        ),
        resource_delivery::inventory,
    );
    routes.post(
        "/v1/client/register",
        connection(
            A::ClientRegister,
            Target::ClientInstallation,
            Scope::SubscribeResources,
            Selector::NewInstallationOwnerActor,
        ),
        client::register,
    );
    routes.post(
        "/v1/client/heartbeat",
        connection(
            A::ClientHeartbeat,
            Target::ClientInstallation,
            Scope::SubscribeResources,
            Selector::InstallationOwnerBody,
        ),
        client::heartbeat,
    );
    routes.post(
        "/v1/telemetry/batch",
        connection(
            A::ClientTelemetryIngest,
            Target::ClientInstallation,
            Scope::ReportTelemetry,
            all([
                Selector::InstallationOwnerBody,
                Selector::TelemetryAttributionOwner,
            ]),
        ),
        telemetry::ingest,
    );
    routes.post(
        "/v1/usage/resources",
        connection(
            A::ClientResourceUsageIngest,
            Target::Resource,
            Scope::ReportTelemetry,
            Selector::TelemetryAttributionOwner,
        ),
        resources::ingest_usage,
    );
    routes.get(
        "/v1/realtime/events",
        connection(
            A::ClientRealtimeEvents,
            Target::Resource,
            Scope::SubscribeResources,
            Selector::EffectiveAudienceList,
        ),
        realtime::events,
    );
}

fn browser(action: A, target: Target, permission: P, selector: Selector) -> RouteDefinition {
    RouteDefinition::browser(action, target, permission, selector)
}

fn browser_alternatives(
    action: A,
    target: Target,
    alternatives: Vec<Alternative>,
) -> RouteDefinition {
    RouteDefinition::browser_alternatives(action, target, alternatives)
}

fn connection(action: A, target: Target, scope: Scope, selector: Selector) -> RouteDefinition {
    RouteDefinition::connection(
        action,
        target,
        scope,
        all([selector, Selector::CurrentScopePolicy]),
    )
}

fn alt(permission: P, selector: Selector) -> Alternative {
    Alternative::new(permission, selector)
}

fn all(items: impl IntoIterator<Item = Selector>) -> Selector {
    Selector::all_of(items)
}

fn self_actor() -> Selector {
    Selector::SelfActor
}

fn project_member() -> Selector {
    Selector::ProjectMember
}

fn taxonomy_assignment(action: A) -> RouteDefinition {
    browser_alternatives(
        action,
        Target::Taxonomy,
        vec![
            alt(P::MemberTagAssignmentManage, Selector::EntityMemberPath),
            alt(P::ResourceAccessManage, Selector::EntityResourcePath),
        ],
    )
}

fn resource_owner(action: A, permission: P) -> RouteDefinition {
    browser(
        action,
        Target::Resource,
        permission,
        Selector::InProjectResourcePath,
    )
}

fn resource_lifecycle(action: A) -> RouteDefinition {
    browser_alternatives(
        action,
        Target::Resource,
        vec![
            alt(
                P::ResourceLifecycleManage,
                all([
                    Selector::InProjectResourcePath,
                    Selector::KindOfResourcePath,
                    Selector::AgentOrSkill,
                ]),
            ),
            alt(
                P::ResourceLifecycleManage,
                all([
                    Selector::InProjectResourcePath,
                    Selector::KindOfResourcePath,
                    Selector::RestrictedKind,
                ]),
            ),
        ],
    )
}

fn member_telemetry(action: A) -> RouteDefinition {
    browser_alternatives(
        action,
        Target::Member,
        vec![
            alt(P::TelemetryMemberReadSelf, Selector::SelfMemberPath),
            alt(P::TelemetryMemberReadAny, Selector::OtherMemberPath),
        ],
    )
}

fn analytics_view_target(action: A) -> RouteDefinition {
    browser_alternatives(
        action,
        Target::AnalyticsView,
        vec![
            alt(P::AnalyticsViewManageSelf, Selector::ViewOwnerPath),
            alt(P::AnalyticsViewManageAny, project_member()),
        ],
    )
}
