mod support;

use conductor_domain::{
    AnalyticsDashboardDensity, AnalyticsDashboardPreset, AnalyticsDateRange, AnalyticsDimension,
    AnalyticsMetric, AnalyticsQuery, AnalyticsViewDefinition, AnalyticsViewVisibility,
    AnalyticsVisualization, AnalyticsWidget, AnalyticsWidgetSize, CreateAnalyticsViewRequest,
    PrimaryRole, SetupRequest, UpdateAnalyticsViewRequest, ANALYTICS_VIEW_SCHEMA_VERSION,
};
use conductor_storage::repos::AnalyticsViewWriteError;
use sqlx::Row;
use support::{connect_test_db, seed_active_user, PLACEHOLDER_PASSWORD_HASH};

fn definition() -> AnalyticsViewDefinition {
    AnalyticsViewDefinition {
        schema_version: ANALYTICS_VIEW_SCHEMA_VERSION,
        preset: AnalyticsDashboardPreset::Executive,
        density: AnalyticsDashboardDensity::Comfortable,
        query: AnalyticsQuery {
            date_range: AnalyticsDateRange::Last30Days,
            ..AnalyticsQuery::default()
        },
        widgets: vec![AnalyticsWidget {
            id: "requests".into(),
            title: "Requests".into(),
            visualization: AnalyticsVisualization::Area,
            metric: AnalyticsMetric::Requests,
            group_by: Some(AnalyticsDimension::Time),
            size: AnalyticsWidgetSize::Full,
            limit: 10,
            show_legend: false,
        }],
    }
}

#[tokio::test]
async fn repository_scopes_reads_and_writes_and_checks_revisions() {
    let db = connect_test_db().await;
    let (project, admin) = db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: "analytics-repo".into(),
                display_name: None,
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "analytics-repo@example.test".into(),
                admin_display_name: "Analytics Repo".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            PLACEHOLDER_PASSWORD_HASH,
            "analytics-repo-secret",
            None,
        )
        .await
        .expect("complete setup");
    let owner = seed_active_user(&db, PrimaryRole::Contribute).await;
    let peer = seed_active_user(&db, PrimaryRole::Contribute).await;

    let request = CreateAnalyticsViewRequest {
        name: "Team health".into(),
        description: None,
        visibility: AnalyticsViewVisibility::Private,
        definition: definition(),
    };
    let view = db
        .analytics_views()
        .create(project.id, owner.id, &request)
        .await
        .expect("create view");

    assert!(db
        .analytics_views()
        .find_accessible(project.id, view.id, peer.id, false)
        .await
        .expect("peer read")
        .is_none());
    assert!(db
        .analytics_views()
        .find_accessible(project.id, view.id, admin.id, true)
        .await
        .expect("admin read")
        .is_some());

    let update = UpdateAnalyticsViewRequest {
        name: request.name.clone(),
        description: Some("Shared operations view".into()),
        visibility: AnalyticsViewVisibility::Shared,
        definition: definition(),
        revision: 1,
    };
    let error = db
        .analytics_views()
        .update(project.id, view.id, peer.id, false, &update)
        .await
        .expect_err("peer cannot edit shared view");
    assert!(matches!(error, AnalyticsViewWriteError::Forbidden));

    let updated = db
        .analytics_views()
        .update(project.id, view.id, owner.id, false, &update)
        .await
        .expect("owner update");
    assert_eq!(updated.revision, 2);
    let error = db
        .analytics_views()
        .update(project.id, view.id, owner.id, false, &update)
        .await
        .expect_err("stale update must conflict");
    assert!(matches!(
        error,
        AnalyticsViewWriteError::RevisionConflict {
            current_revision: 2
        }
    ));

    // A project id is part of every lookup, even when the UUID is known.
    let unrelated_project = uuid::Uuid::new_v4();
    assert!(db
        .analytics_views()
        .find_accessible(unrelated_project, view.id, owner.id, true)
        .await
        .expect("cross-project lookup")
        .is_none());

    let stored: String = sqlx::query("SELECT definition FROM analytics_views WHERE id = ?")
        .bind(view.id.to_string())
        .fetch_one(db.pool())
        .await
        .expect("load stored definition")
        .get("definition");
    assert!(!stored.to_lowercase().contains("select "));
}
