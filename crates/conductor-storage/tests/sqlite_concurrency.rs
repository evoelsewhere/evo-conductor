mod support;

use chrono::Utc;
use conductor_domain::{
    PrimaryRole, TelemetryEventRequest, TelemetryEventStatus, TelemetryEventType,
};
use conductor_storage::Db;
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
async fn concurrent_idempotent_telemetry_waits_for_the_sqlite_writer() {
    let path = std::env::temp_dir().join(format!(
        "evo-conductor-telemetry-concurrency-{}.db",
        Uuid::new_v4().simple()
    ));
    let url = format!("sqlite:{}?mode=rwc", path.display());
    let db = Db::connect(&url).await.expect("connect file-backed sqlite");
    let now = Utc::now().to_rfc3339();
    let project_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO instance (
            id, project_name, bind_host, bind_port, collection_level,
            setup_completed, jwt_secret, created_at, updated_at
        ) VALUES (?, 'Concurrency test', '127.0.0.1', 0, 'L1', 1, 'unused', ?, ?)
        "#,
    )
    .bind(project_id.to_string())
    .bind(&now)
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("seed instance");
    let user = support::seed_active_user(&db, PrimaryRole::User).await;
    let installation_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO client_installations (
            id, instance_id, user_id, installation_key, display_name, platform,
            evoflux_version, connected_at, last_seen_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, 'Concurrent desktop', 'linux', '0.9.0', ?, ?, ?, ?)
        "#,
    )
    .bind(installation_id.to_string())
    .bind(project_id.to_string())
    .bind(user.id.to_string())
    .bind(Uuid::new_v4().to_string())
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("seed installation");

    let event = TelemetryEventRequest {
        event_id: Uuid::new_v4(),
        request_id: "concurrent-request".into(),
        session_id: Some("concurrent-session".into()),
        event_type: TelemetryEventType::Request,
        sequence: 1,
        agent_name: Some("test-agent".into()),
        provider: None,
        model: None,
        response_model: None,
        tokens_in: 0,
        tokens_out: 0,
        cache_read_tokens: 0,
        reasoning_tokens: 0,
        tool_use_tokens: 0,
        duration_ms: 10,
        tool_name: None,
        tool_category: None,
        status: TelemetryEventStatus::Success,
        error_category: None,
        estimated_cost_usd_micros: None,
        cost_source: None,
        evoflux_version: Some("0.9.0".into()),
        resources: vec![],
        reported_at: Utc::now(),
    };

    let mut tasks = Vec::new();
    for _ in 0..24 {
        let repo = db.telemetry();
        let user = user.clone();
        let event = event.clone();
        tasks.push(tokio::spawn(async move {
            repo.ingest(project_id, &user, installation_id, "0.9.0", &[event])
                .await
        }));
    }

    let mut accepted = 0;
    let mut duplicates = 0;
    for task in tasks {
        let result = task.await.expect("telemetry task joined").expect("ingest");
        accepted += result.accepted;
        duplicates += result.duplicates;
    }
    assert_eq!(accepted, 1);
    assert_eq!(duplicates, 23);

    let journal_mode: String = sqlx::query("PRAGMA journal_mode")
        .fetch_one(db.pool())
        .await
        .expect("journal mode")
        .get(0);
    let busy_timeout: i64 = sqlx::query("PRAGMA busy_timeout")
        .fetch_one(db.pool())
        .await
        .expect("busy timeout")
        .get(0);
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    assert_eq!(busy_timeout, 30_000);

    db.pool().close().await;
    for candidate in [
        path.clone(),
        path.with_extension("db-wal"),
        path.with_extension("db-shm"),
    ] {
        let _ = std::fs::remove_file(candidate);
    }
}
