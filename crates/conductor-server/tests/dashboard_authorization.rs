mod support;

use std::sync::Arc;

use axum::http::StatusCode;
use chrono::{DateTime, TimeDelta, Utc};
use conductor_domain::{
    DashboardHostMetrics, DashboardHostMetricsScope, PrimaryRole, User,
    DASHBOARD_PRESENCE_THRESHOLD_SECONDS,
};
use conductor_server::core::host_metrics::HostMetricsProvider;
use serde_json::{json, Value};
use support::{test_app, test_app_with_host_metrics, TestApp};
use uuid::Uuid;

#[derive(Clone)]
struct FixedHostMetricsProvider {
    metrics: DashboardHostMetrics,
}

impl HostMetricsProvider for FixedHostMetricsProvider {
    fn sample(&self) -> DashboardHostMetrics {
        self.metrics.clone()
    }
}

#[tokio::test]
async fn dashboard_allows_admin_and_contributor_but_denies_user() {
    let app = test_app().await;
    app.seed_project_identity().await;

    assert_eq!(
        app.get("/api/dashboard", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    for role in [PrimaryRole::Admin, PrimaryRole::Contribute] {
        let token = app.token_for_role(role).await;
        let (status, body) = app.get("/api/dashboard", Some(&token)).await;
        assert_eq!(status, StatusCode::OK, "{role:?}: {body}");
    }

    let user_token = app.token_for_role(PrimaryRole::User).await;
    assert_eq!(
        app.get("/api/dashboard", Some(&user_token)).await.0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn dashboard_projects_presence_realtime_host_metrics_and_role_scoped_feedback() {
    let sampled_at = DateTime::parse_from_rfc3339("2026-08-18T05:06:07Z")
        .expect("fixed timestamp")
        .with_timezone(&Utc);
    let app = test_app_with_host_metrics(Arc::new(FixedHostMetricsProvider {
        metrics: DashboardHostMetrics {
            scope: DashboardHostMetricsScope::ConductorHost,
            sampled_at,
            // A provider that is still warming must remain nullable.
            cpu_usage_percent: None,
            memory_used_bytes: Some(3_000),
            memory_total_bytes: Some(8_000),
            // Unsupported GPU telemetry must not be presented as zero.
            gpu_usage_percent: None,
            vram_used_bytes: None,
            vram_total_bytes: None,
        },
    }))
    .await;
    let project_id = app.seed_project_identity().await;
    let admin = app.seed_user(PrimaryRole::Admin).await;
    let contributor = app.seed_user(PrimaryRole::Contribute).await;
    let other = app.seed_user(PrimaryRole::User).await;
    let disabled = app.seed_user(PrimaryRole::User).await;
    sqlx::query("UPDATE users SET status = 'disabled' WHERE id = ?")
        .bind(disabled.id.to_string())
        .execute(app.state.db.pool())
        .await
        .expect("disable presence fixture member");

    let before_request = Utc::now();
    seed_installation(
        &app,
        project_id,
        &admin,
        before_request - TimeDelta::seconds(20),
        "admin-a",
    )
    .await;
    seed_installation(
        &app,
        project_id,
        &admin,
        before_request - TimeDelta::seconds(40),
        "admin-b",
    )
    .await;
    seed_installation(
        &app,
        project_id,
        &contributor,
        before_request - TimeDelta::seconds(60),
        "contributor",
    )
    .await;
    seed_installation(
        &app,
        project_id,
        &other,
        before_request - TimeDelta::seconds(600),
        "stale",
    )
    .await;
    seed_installation(
        &app,
        project_id,
        &disabled,
        before_request - TimeDelta::seconds(10),
        "disabled",
    )
    .await;

    let contributor_resource_a =
        seed_resource(&app, project_id, contributor.id, "contributor-a").await;
    let contributor_resource_b =
        seed_resource(&app, project_id, contributor.id, "contributor-b").await;
    let other_resource = seed_resource(&app, project_id, other.id, "other").await;
    seed_feedback(
        &app,
        contributor_resource_a,
        admin.id,
        5,
        "PRIVATE_FEEDBACK_COMMENT_ALPHA",
    )
    .await;
    seed_feedback(
        &app,
        contributor_resource_b,
        other.id,
        2,
        "PRIVATE_FEEDBACK_COMMENT_BETA",
    )
    .await;
    seed_feedback(
        &app,
        other_resource,
        contributor.id,
        4,
        "PRIVATE_FEEDBACK_COMMENT_GAMMA",
    )
    .await;

    // These permits model three open SSE streams on this process: two owners,
    // with two distinct streams belonging to the same owner.
    let _realtime_permits = [
        app.state
            .realtime
            .try_connect(Uuid::new_v4(), admin.id)
            .expect("first admin stream"),
        app.state
            .realtime
            .try_connect(Uuid::new_v4(), admin.id)
            .expect("second admin stream"),
        app.state
            .realtime
            .try_connect(Uuid::new_v4(), contributor.id)
            .expect("contributor stream"),
    ];

    let admin_token = app.token_for(&admin).await;
    let contributor_token = app.token_for(&contributor).await;
    let (admin_status, admin_body) = app.get("/api/dashboard", Some(&admin_token)).await;
    let after_request = Utc::now();
    assert_eq!(admin_status, StatusCode::OK, "{admin_body}");
    assert_presence_and_runtime(&admin_body, before_request, after_request, sampled_at);
    assert_eq!(admin_body["feedback"]["scope"], "project");
    assert_feedback(&admin_body, 3, 3.7, 2, 66.7, [0, 1, 0, 1, 1]);

    let (contributor_status, contributor_body) =
        app.get("/api/dashboard", Some(&contributor_token)).await;
    assert_eq!(contributor_status, StatusCode::OK, "{contributor_body}");
    assert_presence_and_runtime(&contributor_body, before_request, Utc::now(), sampled_at);
    assert_eq!(contributor_body["feedback"]["scope"], "owned_resources");
    assert_feedback(&contributor_body, 2, 3.5, 1, 50.0, [0, 1, 0, 0, 1]);

    for body in [&admin_body, &contributor_body] {
        let rendered = body.to_string();
        assert!(!rendered.contains("PRIVATE_FEEDBACK_COMMENT"));
        assert!(!rendered.contains(&admin.email));
        assert!(!rendered.contains(&contributor.email));
        assert!(!rendered.contains(&other.email));
    }
}

async fn seed_installation(
    app: &TestApp,
    project_id: Uuid,
    user: &User,
    last_seen_at: DateTime<Utc>,
    label: &str,
) {
    let id = Uuid::new_v4();
    let created_at = last_seen_at - TimeDelta::seconds(10);
    sqlx::query(
        r#"
        INSERT INTO client_installations (
            id, instance_id, user_id, installation_key, display_name, platform,
            evoflux_version, connected_at, last_seen_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, 'test', 'test', ?, ?, ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(project_id.to_string())
    .bind(user.id.to_string())
    .bind(format!("{label}-{}", Uuid::new_v4().simple()))
    .bind(label)
    .bind(created_at.to_rfc3339())
    .bind(last_seen_at.to_rfc3339())
    .bind(created_at.to_rfc3339())
    .bind(last_seen_at.to_rfc3339())
    .execute(app.state.db.pool())
    .await
    .expect("seed client installation");
}

async fn seed_resource(app: &TestApp, project_id: Uuid, owner_user_id: Uuid, label: &str) -> Uuid {
    let resource_id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO resources (
            id, project_id, kind, slug, name, version, owner_user_id, visibility,
            status, payload, created_at, updated_at
        ) VALUES (?, ?, 'skill', ?, ?, '1.0.0', ?, 'shared', 'published', '{}', ?, ?)
        "#,
    )
    .bind(resource_id.to_string())
    .bind(project_id.to_string())
    .bind(format!("dashboard-{label}-{}", Uuid::new_v4().simple()))
    .bind(format!("Dashboard {label}"))
    .bind(owner_user_id.to_string())
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed dashboard resource");
    resource_id
}

async fn seed_feedback(
    app: &TestApp,
    resource_id: Uuid,
    user_id: Uuid,
    rating: i64,
    comment: &str,
) {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO resource_feedback (
            id, resource_id, resource_version, user_id, rating, comment,
            created_at, updated_at
        ) VALUES (?, ?, '1.0.0', ?, ?, ?, ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(resource_id.to_string())
    .bind(user_id.to_string())
    .bind(rating)
    .bind(comment)
    .bind(&now)
    .bind(&now)
    .execute(app.state.db.pool())
    .await
    .expect("seed resource feedback");
}

fn assert_presence_and_runtime(
    body: &Value,
    request_started_at: DateTime<Utc>,
    request_finished_at: DateTime<Utc>,
    sampled_at: DateTime<Utc>,
) {
    assert_eq!(body["presence"]["clients_seen_recently"], 3);
    assert_eq!(body["presence"]["members_seen_recently"], 2);
    assert_eq!(
        body["presence"]["threshold_seconds"],
        DASHBOARD_PRESENCE_THRESHOLD_SECONDS
    );
    // Legacy clients must see exactly the same heartbeat-derived member count.
    assert_eq!(
        body["members_online"],
        body["presence"]["members_seen_recently"]
    );
    let observed_at = body["presence"]["observed_at"]
        .as_str()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
        .expect("presence observed_at timestamp");
    assert!(observed_at >= request_started_at);
    assert!(observed_at <= request_finished_at);

    assert_eq!(
        body["realtime"],
        json!({"scope": "this_node", "active_owners": 2, "active_streams": 3})
    );
    assert_eq!(body["host_metrics"]["scope"], "conductor_host");
    assert_eq!(body["host_metrics"]["sampled_at"], json!(sampled_at));
    assert_eq!(body["host_metrics"]["cpu_usage_percent"], Value::Null);
    assert_eq!(body["host_metrics"]["memory_used_bytes"], 3_000);
    assert_eq!(body["host_metrics"]["memory_total_bytes"], 8_000);
    assert_eq!(body["host_metrics"]["gpu_usage_percent"], Value::Null);
    assert_eq!(body["host_metrics"]["vram_used_bytes"], Value::Null);
    assert_eq!(body["host_metrics"]["vram_total_bytes"], Value::Null);
}

fn assert_feedback(
    body: &Value,
    count: u64,
    average: f64,
    positive_count: u64,
    positive_percent: f64,
    distribution: [u64; 5],
) {
    assert_eq!(body["feedback"]["count"], count);
    assert_eq!(body["feedback"]["average_rating"], average);
    assert_eq!(body["feedback"]["positive_count"], positive_count);
    assert_eq!(body["feedback"]["positive_percent"], positive_percent);
    for (index, expected) in distribution.into_iter().enumerate() {
        let rating = format!("rating_{}", index + 1);
        assert_eq!(body["feedback"]["distribution"][rating.as_str()], expected);
    }
}
