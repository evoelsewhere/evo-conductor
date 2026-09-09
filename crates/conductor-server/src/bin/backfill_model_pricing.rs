//! One-off maintenance tool: prices `model_call` telemetry rows already in
//! storage that no one has priced yet — history from before Conductor could
//! backfill cost on ingest, or from before a provider-id alias existed.
//!
//! Ingest only prices a new event as it arrives (see
//! `http::routes::telemetry::price_unpriced_model_calls`); it never revisits
//! rows already in the database. This binary is the other half: run it once
//! after a catalog or alias fix to bring existing history in line, rather
//! than leaving it unpriced forever.
//!
//! Usage: `cargo run -p conductor-server --bin backfill_model_pricing -- [limit]`
//! (default limit: 10,000 rows in one pass)

use std::env;

use anyhow::Context;
use conductor_domain::TelemetryCostSource;
use conductor_server::core::constants::pricing::MODELS_DEV_CACHE_PATH;
use conductor_server::core::constants::server::{DEFAULT_DATABASE_URL, ENV_DATABASE_URL};
use conductor_server::core::model_pricing::{price_model_call, ModelsDevPricingCatalog};
use conductor_storage::Db;

const DEFAULT_LIMIT: u32 = 10_000;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let limit = parse_limit(std::env::args().skip(1))?;
    let database_url = env::var(ENV_DATABASE_URL).unwrap_or_else(|_| DEFAULT_DATABASE_URL.into());
    let db = Db::connect(&database_url)
        .await
        .with_context(|| format!("connect to database at {database_url}"))?;
    let pricing = ModelsDevPricingCatalog::new(MODELS_DEV_CACHE_PATH);

    let unpriced = db.telemetry().list_unpriced_model_calls(limit).await?;
    let total = unpriced.len();
    let mut priced = 0usize;
    let mut skipped = 0usize;

    for event in unpriced {
        let Some(usd) = price_model_call(
            &pricing,
            &event.provider,
            &event.model,
            event.tokens_in,
            event.tokens_out,
            event.cache_read_tokens,
            event.cache_write_tokens,
            event.reasoning_tokens,
        )
        .await
        else {
            skipped += 1;
            continue;
        };
        let micros = (usd * 1_000_000.0).round().max(0.0) as u64;
        let updated = db
            .telemetry()
            .backfill_event_cost(event.event_id, micros, TelemetryCostSource::ConductorCatalog)
            .await?;
        if updated {
            priced += 1;
        } else {
            skipped += 1;
        }
    }

    println!(
        "priced {priced} of {total} unpriced model_call events ({skipped} still unpriced — no catalog rate found for their provider/model)"
    );
    if total == DEFAULT_LIMIT as usize || total == limit as usize {
        println!("hit the row limit ({limit}) — rerun with a higher limit if more remain");
    }
    Ok(())
}

fn parse_limit(args: impl IntoIterator<Item = String>) -> anyhow::Result<u32> {
    let mut args = args.into_iter();
    match args.next() {
        None => Ok(DEFAULT_LIMIT),
        Some(value) => value
            .parse()
            .with_context(|| format!("invalid limit argument: {value}")),
    }
}
