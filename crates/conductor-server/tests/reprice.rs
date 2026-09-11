//! Repricing has to be conservative in two directions at once: it must not
//! rewrite a cost that was computed from the rate in force (a member's receipt
//! was reconciled against it), and it must not pass off an estimate as a
//! price. These tests pin both, plus idempotency.

mod support;

use conductor_server::core::model_pricing::apply_catalog;
use conductor_server::core::reprice::{self, Basis};
use sqlx::Row;
use support::test_app;
use uuid::Uuid;

const CATALOG: &[u8] = br#"{"anthropic":{"id":"anthropic","models":{
    "claude-sonnet-4":{"id":"claude-sonnet-4","cost":{"input":3,"output":15}}}}}"#;

/// Insert a model call directly, bypassing ingest so the row's age and its
/// unpriced state can be set precisely.
async fn seed_model_call(
    app: &support::TestApp,
    project_id: Uuid,
    user_id: Uuid,
    reported_at: chrono::DateTime<chrono::Utc>,
    model: &str,
) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, project_id, user_id, request_id, event_type, sequence,
            provider, model, tokens_in, tokens_out, cache_read_tokens,
            cache_write_tokens, reasoning_tokens, tool_use_tokens, duration_ms,
            status, reported_at, received_at
        ) VALUES (?, ?, ?, 'req', 'model_call', 1, 'anthropic', ?,
                  1000000, 0, 0, 0, 0, 0, 10, 'success', ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(project_id.to_string())
    .bind(user_id.to_string())
    .bind(model)
    .bind(reported_at.to_rfc3339())
    .bind(chrono::Utc::now().to_rfc3339())
    .execute(app.state.db.pool())
    .await
    .expect("seed model call");
    id
}

async fn stored(app: &support::TestApp, id: Uuid) -> (Option<i64>, Option<String>, Option<String>) {
    let row = sqlx::query(
        "SELECT server_cost_usd_micros, pricing_basis, unpriced_reason \
         FROM telemetry_events WHERE id = ?",
    )
    .bind(id.to_string())
    .fetch_one(app.state.db.pool())
    .await
    .expect("read row");
    (
        row.get("server_cost_usd_micros"),
        row.get("pricing_basis"),
        row.get("unpriced_reason"),
    )
}

/// An event that a synced rate covers is priced exactly, and marked as such.
#[tokio::test]
async fn prices_an_event_the_rate_in_force_covers() {
    let app = test_app().await;
    let project_id = Uuid::new_v4();
    let user = app.seed_user(conductor_domain::PrimaryRole::User).await;

    apply_catalog(
        &app.state.db,
        CATALOG,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");
    // Reported after the sync, so the rate is genuinely in force.
    let id = seed_model_call(
        &app,
        project_id,
        user.id,
        chrono::Utc::now() + chrono::Duration::seconds(2),
        "claude-sonnet-4",
    )
    .await;

    let report = reprice::run(&app.state.db, project_id, Basis::InForceOnly)
        .await
        .expect("reprice");
    assert_eq!(report.examined, 1);
    assert_eq!(report.priced_in_force, 1);
    assert_eq!(report.priced_from_earliest, 0);

    let (cost, basis, reason) = stored(&app, id).await;
    assert_eq!(cost, Some(3_000_000));
    assert_eq!(basis.as_deref(), Some("in_force"));
    assert_eq!(reason, None);
}

/// The property that makes strict mode honest: an event older than any rate
/// row must stay unpriced rather than borrowing a later price.
#[tokio::test]
async fn strict_mode_leaves_pre_catalog_events_unpriced() {
    let app = test_app().await;
    let project_id = Uuid::new_v4();
    let user = app.seed_user(conductor_domain::PrimaryRole::User).await;

    let id = seed_model_call(
        &app,
        project_id,
        user.id,
        chrono::Utc::now() - chrono::Duration::days(30),
        "claude-sonnet-4",
    )
    .await;
    apply_catalog(
        &app.state.db,
        CATALOG,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");

    let report = reprice::run(&app.state.db, project_id, Basis::InForceOnly)
        .await
        .expect("reprice");
    assert_eq!(report.priced_in_force, 0);
    assert_eq!(report.left_unpriced, 1);

    let (cost, basis, _) = stored(&app, id).await;
    assert_eq!(cost, None, "no rate covered this event");
    assert_eq!(basis, None);
}

/// The fallback prices that same event, but records that it is an estimate —
/// a figure derived from the earliest known rate is a different claim.
#[tokio::test]
async fn fallback_estimates_pre_catalog_events_and_labels_them() {
    let app = test_app().await;
    let project_id = Uuid::new_v4();
    let user = app.seed_user(conductor_domain::PrimaryRole::User).await;

    let id = seed_model_call(
        &app,
        project_id,
        user.id,
        chrono::Utc::now() - chrono::Duration::days(30),
        "claude-sonnet-4",
    )
    .await;
    apply_catalog(
        &app.state.db,
        CATALOG,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");

    let report = reprice::run(&app.state.db, project_id, Basis::EarliestKnownFallback)
        .await
        .expect("reprice");
    assert_eq!(report.priced_from_earliest, 1);
    assert_eq!(report.priced_in_force, 0);

    let (cost, basis, _) = stored(&app, id).await;
    assert_eq!(cost, Some(3_000_000));
    assert_eq!(
        basis.as_deref(),
        Some("earliest_known"),
        "an estimate must never be indistinguishable from a price"
    );
}

/// A model absent from the catalog stays unpriced under either basis, with a
/// reason recorded so the gap is explainable.
#[tokio::test]
async fn an_unknown_model_records_a_reason_under_the_fallback_too() {
    let app = test_app().await;
    let project_id = Uuid::new_v4();
    let user = app.seed_user(conductor_domain::PrimaryRole::User).await;

    let id = seed_model_call(
        &app,
        project_id,
        user.id,
        chrono::Utc::now(),
        "model-nobody-published",
    )
    .await;
    apply_catalog(
        &app.state.db,
        CATALOG,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");

    let report = reprice::run(&app.state.db, project_id, Basis::EarliestKnownFallback)
        .await
        .expect("reprice");
    assert_eq!(report.left_unpriced, 1);

    let (cost, basis, reason) = stored(&app, id).await;
    assert_eq!(cost, None);
    assert_eq!(basis, None);
    assert_eq!(reason.as_deref(), Some("model_unknown"));
}

/// Running twice must not change anything the first pass settled, and must
/// not re-examine rows it already priced.
#[tokio::test]
async fn a_second_pass_is_a_no_op() {
    let app = test_app().await;
    let project_id = Uuid::new_v4();
    let user = app.seed_user(conductor_domain::PrimaryRole::User).await;

    apply_catalog(
        &app.state.db,
        CATALOG,
        "https://models.dev/api.json",
        "models_dev",
    )
    .await
    .expect("apply catalog");
    let id = seed_model_call(
        &app,
        project_id,
        user.id,
        chrono::Utc::now() + chrono::Duration::seconds(2),
        "claude-sonnet-4",
    )
    .await;

    let first = reprice::run(&app.state.db, project_id, Basis::InForceOnly)
        .await
        .expect("first pass");
    assert_eq!(first.priced_in_force, 1);
    let after_first = stored(&app, id).await;

    let second = reprice::run(&app.state.db, project_id, Basis::InForceOnly)
        .await
        .expect("second pass");
    assert_eq!(
        second.examined, 0,
        "an already priced row must not be re-examined"
    );
    assert_eq!(stored(&app, id).await, after_first);
}

/// With no catalog there is nothing to price from, and writing "unpriced"
/// reasons would only be invalidated by the next sync.
#[tokio::test]
async fn without_a_catalog_repricing_does_nothing() {
    let app = test_app().await;
    let project_id = Uuid::new_v4();
    let user = app.seed_user(conductor_domain::PrimaryRole::User).await;
    let id = seed_model_call(
        &app,
        project_id,
        user.id,
        chrono::Utc::now(),
        "claude-sonnet-4",
    )
    .await;

    let report = reprice::run(&app.state.db, project_id, Basis::EarliestKnownFallback)
        .await
        .expect("reprice");
    assert_eq!(report, reprice::RepriceReport::default());
    assert_eq!(stored(&app, id).await, (None, None, None));
}
