//! Applying a catalog must add a rate row only when a rate actually changed.
//! Upstream edits descriptions and capability flags constantly; if every such
//! edit appended an `effective_from`, the price history would become unusable
//! for explaining a historical cost.

mod support;

use conductor_server::core::model_pricing::{apply_catalog, catalog_version};
use support::test_app;

const SOURCE_URL: &str = "https://models.dev/api.json";

fn catalog(sonnet_input: &str, description: &str) -> Vec<u8> {
    format!(
        r#"{{
            "anthropic": {{
                "id": "anthropic",
                "models": {{
                    "claude-sonnet-4": {{
                        "id": "claude-sonnet-4",
                        "description": "{description}",
                        "cost": {{"input": {sonnet_input}, "output": 15, "cache_read": 0.3}}
                    }},
                    "claude-haiku-4": {{
                        "id": "claude-haiku-4",
                        "cost": {{"input": 0.8, "output": 4}}
                    }}
                }}
            }}
        }}"#
    )
    .into_bytes()
}

#[tokio::test]
async fn first_sync_records_the_catalog_and_every_rate() {
    let app = test_app().await;
    let payload = catalog("3", "first");

    let outcome = apply_catalog(&app.state.db, &payload, SOURCE_URL, "models_dev")
        .await
        .expect("apply");

    assert!(!outcome.unchanged);
    assert_eq!(outcome.version, catalog_version(&payload));
    assert_eq!(outcome.model_count, 2);
    assert_eq!(outcome.priced_model_count, 2);
    assert_eq!(outcome.changed_model_count, 2);

    let rates = app
        .state
        .db
        .model_prices()
        .pricing_at("anthropic", "claude-sonnet-4", chrono::Utc::now())
        .await
        .expect("lookup")
        .expect("sonnet priced");
    assert_eq!(rates.pricing.base.input, Some(3_000_000));
    assert_eq!(rates.pricing.base.cache_read, Some(300_000));

    let recorded = app
        .state
        .db
        .model_prices()
        .latest_catalog()
        .await
        .expect("catalog")
        .expect("recorded");
    assert_eq!(recorded.version, outcome.version);
    assert_eq!(recorded.source, "models_dev");
    assert_eq!(recorded.source_url.as_deref(), Some(SOURCE_URL));
}

#[tokio::test]
async fn re_applying_an_identical_document_does_no_work() {
    let app = test_app().await;
    let payload = catalog("3", "first");
    apply_catalog(&app.state.db, &payload, SOURCE_URL, "models_dev")
        .await
        .expect("first apply");

    let again = apply_catalog(&app.state.db, &payload, SOURCE_URL, "models_dev")
        .await
        .expect("second apply");

    assert!(again.unchanged, "an unchanged document must short-circuit");
    assert_eq!(again.changed_model_count, 0);
    assert_eq!(
        app.state
            .db
            .model_prices()
            .current_rate_table()
            .await
            .expect("table")
            .len(),
        2
    );
}

/// The property this module exists for: a new document whose rates are
/// identical must record the catalog but append no rate row.
#[tokio::test]
async fn a_new_document_with_unchanged_rates_appends_no_rate_row() {
    let app = test_app().await;
    apply_catalog(
        &app.state.db,
        &catalog("3", "first"),
        SOURCE_URL,
        "models_dev",
    )
    .await
    .expect("first apply");

    let edited = catalog("3", "upstream reworded this");
    let outcome = apply_catalog(&app.state.db, &edited, SOURCE_URL, "models_dev")
        .await
        .expect("second apply");

    assert!(
        !outcome.unchanged,
        "the document differs, so it is not a short-circuit"
    );
    assert_eq!(
        outcome.changed_model_count, 0,
        "no rate changed, so no row may be appended"
    );

    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_prices")
        .fetch_one(app.state.db.pool())
        .await
        .expect("count rows");
    assert_eq!(rows, 2, "still one row per model");
}

#[tokio::test]
async fn a_changed_rate_appends_a_row_and_leaves_the_old_one_resolvable() {
    let app = test_app().await;
    let before = chrono::Utc::now();
    apply_catalog(
        &app.state.db,
        &catalog("3", "first"),
        SOURCE_URL,
        "models_dev",
    )
    .await
    .expect("first apply");

    // Separate the two effective_from stamps so the boundary is observable.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let between = chrono::Utc::now();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;

    let outcome = apply_catalog(
        &app.state.db,
        &catalog("4.5", "first"),
        SOURCE_URL,
        "models_dev",
    )
    .await
    .expect("second apply");
    assert_eq!(
        outcome.changed_model_count, 1,
        "only sonnet's rate moved; haiku must not be rewritten"
    );

    let prices = app.state.db.model_prices();
    let now = prices
        .pricing_at("anthropic", "claude-sonnet-4", chrono::Utc::now())
        .await
        .unwrap()
        .expect("current rate");
    assert_eq!(now.pricing.base.input, Some(4_500_000));

    let historical = prices
        .pricing_at("anthropic", "claude-sonnet-4", between)
        .await
        .unwrap()
        .expect("rate in force before the change");
    assert_eq!(
        historical.pricing.base.input,
        Some(3_000_000),
        "a cost reported before the change must still price at the old rate"
    );

    assert!(
        prices
            .pricing_at("anthropic", "claude-sonnet-4", before)
            .await
            .unwrap()
            .is_none(),
        "no rate existed before the first sync"
    );
}

#[tokio::test]
async fn an_unknown_model_has_no_rate_rather_than_a_free_one() {
    let app = test_app().await;
    apply_catalog(
        &app.state.db,
        &catalog("3", "first"),
        SOURCE_URL,
        "models_dev",
    )
    .await
    .expect("apply");

    assert!(
        app.state
            .db
            .model_prices()
            .pricing_at("anthropic", "claude-opus-9", chrono::Utc::now())
            .await
            .expect("lookup")
            .is_none(),
        "a model absent from the catalog must not resolve to a rate"
    );
}

/// A model upstream later drops still has to price events reported while it
/// existed, so its rate row is never deleted. That means the in-force rate
/// table's size is a cumulative, all-time count — it can exceed the latest
/// catalog's `model_count`, and the two must never be presented as a subset
/// pair (`X of Y`) in an operator-facing view.
#[tokio::test]
async fn a_model_dropped_from_a_later_catalog_still_counts_in_the_rate_table() {
    let app = test_app().await;
    apply_catalog(
        &app.state.db,
        &catalog("3", "first"),
        SOURCE_URL,
        "models_dev",
    )
    .await
    .expect("first sync: two models");

    let shrunk = br#"{
        "anthropic": {
            "id": "anthropic",
            "models": {
                "claude-sonnet-4": {
                    "id": "claude-sonnet-4",
                    "cost": {"input": 3, "output": 15, "cache_read": 0.3}
                }
            }
        }
    }"#;
    apply_catalog(&app.state.db, shrunk, SOURCE_URL, "models_dev")
        .await
        .expect("second sync: haiku dropped upstream");

    let rate_table = app
        .state
        .db
        .model_prices()
        .current_rate_table()
        .await
        .expect("rate table");
    assert_eq!(
        rate_table.len(),
        2,
        "haiku's rate must survive even though the second catalog no longer lists it"
    );
    assert!(
        app.state
            .db
            .model_prices()
            .pricing_at("anthropic", "claude-haiku-4", chrono::Utc::now())
            .await
            .expect("lookup")
            .is_some(),
        "an event reported for haiku after it was dropped must still be priceable"
    );
}
