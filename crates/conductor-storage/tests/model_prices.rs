//! Rate rows are append-only: a cost must stay explainable by the price that
//! was in force when its event was reported. These tests pin that, because a
//! repo that quietly updated a row in place would silently rewrite historical
//! spend the next time a vendor changed a price.

mod support;

use chrono::{Duration, Utc};
use conductor_domain::{rate_from_usd_per_million, ModelPricing, ModelRates};
use conductor_storage::repos::{ModelPriceRow, PriceCatalogSnapshot};
use support::connect_test_db;

fn catalog(version: &str, changed: u32) -> PriceCatalogSnapshot {
    PriceCatalogSnapshot {
        version: version.to_string(),
        source: "models_dev".into(),
        source_url: Some("https://models.dev/api.json".into()),
        fetched_at: Utc::now(),
        model_count: 2,
        priced_model_count: 2,
        changed_model_count: changed,
    }
}

fn rates(input: f64, output: f64) -> ModelRates {
    ModelRates {
        input: rate_from_usd_per_million(input),
        output: rate_from_usd_per_million(output),
        cache_read: None,
        cache_write: None,
        reasoning: None,
    }
}

fn pricing(input: f64, output: f64) -> ModelPricing {
    ModelPricing::flat(rates(input, output))
}

#[tokio::test]
async fn resolves_the_rate_in_force_at_the_time_of_the_event() {
    let db = connect_test_db().await;
    let prices = db.model_prices();
    let first_effective = Utc::now() - Duration::days(10);
    let second_effective = Utc::now() - Duration::days(2);

    prices.record_catalog(&catalog("v1", 1)).await.unwrap();
    prices
        .insert_prices(
            "v1",
            first_effective,
            &[ModelPriceRow {
                provider: "anthropic".into(),
                model: "claude-sonnet-4".into(),
                pricing: pricing(3.0, 15.0),
            }],
        )
        .await
        .unwrap();

    prices.record_catalog(&catalog("v2", 1)).await.unwrap();
    prices
        .insert_prices(
            "v2",
            second_effective,
            &[ModelPriceRow {
                provider: "anthropic".into(),
                model: "claude-sonnet-4".into(),
                pricing: pricing(4.0, 20.0),
            }],
        )
        .await
        .unwrap();

    // An event from before the price change must still price at the old rate.
    let old = prices
        .pricing_at(
            "anthropic",
            "claude-sonnet-4",
            Utc::now() - Duration::days(5),
        )
        .await
        .unwrap()
        .expect("a rate was in force five days ago");
    assert_eq!(old.pricing.base.input, rate_from_usd_per_million(3.0));

    let current = prices
        .pricing_at("anthropic", "claude-sonnet-4", Utc::now())
        .await
        .unwrap()
        .expect("a rate is in force now");
    assert_eq!(current.pricing.base.input, rate_from_usd_per_million(4.0));

    // Older than any row: unpriced rather than silently borrowing a later rate.
    assert!(prices
        .pricing_at(
            "anthropic",
            "claude-sonnet-4",
            Utc::now() - Duration::days(30)
        )
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn lookup_is_insensitive_to_provider_and_model_casing() {
    let db = connect_test_db().await;
    let prices = db.model_prices();
    prices.record_catalog(&catalog("v1", 1)).await.unwrap();
    prices
        .insert_prices(
            "v1",
            Utc::now() - Duration::hours(1),
            &[ModelPriceRow {
                provider: "  OpenAI ".into(),
                model: "GPT-5".into(),
                pricing: pricing(1.25, 10.0),
            }],
        )
        .await
        .unwrap();

    for (provider, model) in [
        ("openai", "gpt-5"),
        ("OpenAI", "GPT-5"),
        (" openai ", " Gpt-5 "),
    ] {
        assert!(
            prices
                .pricing_at(provider, model, Utc::now())
                .await
                .unwrap()
                .is_some(),
            "{provider}/{model} must resolve to the same rate row"
        );
    }
}

#[tokio::test]
async fn re_inserting_the_same_effective_row_is_a_no_op() {
    let db = connect_test_db().await;
    let prices = db.model_prices();
    let effective = Utc::now() - Duration::hours(1);
    prices.record_catalog(&catalog("v1", 1)).await.unwrap();
    let row = ModelPriceRow {
        provider: "anthropic".into(),
        model: "claude-sonnet-4".into(),
        pricing: pricing(3.0, 15.0),
    };

    assert_eq!(
        prices
            .insert_prices("v1", effective, std::slice::from_ref(&row))
            .await
            .unwrap(),
        1
    );
    // A retried sync must not fail or duplicate.
    assert_eq!(
        prices.insert_prices("v1", effective, &[row]).await.unwrap(),
        0
    );
    assert_eq!(prices.current_rate_table().await.unwrap().len(), 1);
}

#[tokio::test]
async fn current_rate_table_returns_only_the_newest_row_per_model() {
    let db = connect_test_db().await;
    let prices = db.model_prices();
    prices.record_catalog(&catalog("v1", 2)).await.unwrap();
    prices
        .insert_prices(
            "v1",
            Utc::now() - Duration::days(5),
            &[
                ModelPriceRow {
                    provider: "anthropic".into(),
                    model: "claude-sonnet-4".into(),
                    pricing: pricing(3.0, 15.0),
                },
                ModelPriceRow {
                    provider: "openai".into(),
                    model: "gpt-5".into(),
                    pricing: pricing(1.25, 10.0),
                },
            ],
        )
        .await
        .unwrap();

    prices.record_catalog(&catalog("v2", 1)).await.unwrap();
    prices
        .insert_prices(
            "v2",
            Utc::now() - Duration::days(1),
            &[ModelPriceRow {
                provider: "anthropic".into(),
                model: "claude-sonnet-4".into(),
                pricing: pricing(4.0, 20.0),
            }],
        )
        .await
        .unwrap();

    let table = prices.current_rate_table().await.unwrap();
    assert_eq!(
        table.len(),
        2,
        "one row per model, not one per price change"
    );
    let sonnet = table
        .iter()
        .find(|row| row.model == "claude-sonnet-4")
        .expect("sonnet present");
    assert_eq!(sonnet.pricing.base.input, rate_from_usd_per_million(4.0));
}

#[tokio::test]
async fn latest_catalog_reports_the_most_recent_sync() {
    let db = connect_test_db().await;
    let prices = db.model_prices();
    assert!(prices.latest_catalog().await.unwrap().is_none());

    let mut older = catalog("v1", 7);
    older.fetched_at = Utc::now() - Duration::days(2);
    prices.record_catalog(&older).await.unwrap();
    let newer = catalog("v2", 3);
    prices.record_catalog(&newer).await.unwrap();

    let latest = prices.latest_catalog().await.unwrap().expect("a catalog");
    assert_eq!(latest.version, "v2");
    assert_eq!(latest.changed_model_count, 3);
    assert_eq!(latest.source, "models_dev");
}

/// Rates a catalog leaves unpriced must survive as NULL rather than becoming
/// zero, or an unpriced component would read as free.
#[tokio::test]
async fn absent_rates_round_trip_as_none_not_zero() {
    let db = connect_test_db().await;
    let prices = db.model_prices();
    prices.record_catalog(&catalog("v1", 1)).await.unwrap();
    prices
        .insert_prices(
            "v1",
            Utc::now() - Duration::hours(1),
            &[ModelPriceRow {
                provider: "local".into(),
                model: "llama".into(),
                pricing: ModelPricing::flat(ModelRates {
                    input: rate_from_usd_per_million(0.0),
                    output: None,
                    cache_read: None,
                    cache_write: None,
                    reasoning: None,
                }),
            }],
        )
        .await
        .unwrap();

    let found = prices
        .pricing_at("local", "llama", Utc::now())
        .await
        .unwrap()
        .expect("row present");
    assert_eq!(
        found.pricing.base.input,
        Some(0),
        "an explicit free rate stays zero"
    );
    assert_eq!(
        found.pricing.base.output, None,
        "an absent rate stays absent"
    );
    assert_eq!(found.pricing.base.cache_read, None);
}
