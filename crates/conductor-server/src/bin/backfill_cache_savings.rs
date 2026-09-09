//! One-off maintenance tool: computes `cache_savings_usd_micros` for
//! `model_call` telemetry rows already in storage that predate the column —
//! every row inserted before this feature shipped has it `NULL`, regardless
//! of whether the call was already priced.
//!
//! Ingest only computes this for a new event as it arrives (see
//! `http::routes::telemetry::compute_cache_savings`); it never revisits rows
//! already in the database. This binary is the other half: run it once after
//! deploying the feature to bring existing history in line.
//!
//! Usage: `cargo run -p conductor-server --bin backfill_cache_savings -- [limit]`
//! (default limit: 10,000 rows in one pass)

use std::env;

use anyhow::Context;
use conductor_server::core::constants::pricing::MODELS_DEV_CACHE_PATH;
use conductor_server::core::constants::server::{DEFAULT_DATABASE_URL, ENV_DATABASE_URL};
use conductor_server::core::model_pricing::{cache_savings_for_call, ModelsDevPricingCatalog};
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

    let missing = db
        .telemetry()
        .list_model_calls_missing_cache_savings(limit)
        .await?;
    let total = missing.len();
    let mut computed = 0usize;
    let mut skipped = 0usize;

    for event in missing {
        let Some(usd) = cache_savings_for_call(
            &pricing,
            &event.provider,
            &event.model,
            event.tokens_in,
            event.cache_read_tokens,
            event.cache_write_tokens,
        )
        .await
        else {
            skipped += 1;
            continue;
        };
        let micros = (usd * 1_000_000.0).round() as i64;
        let updated = db
            .telemetry()
            .backfill_event_cache_savings(event.event_id, micros)
            .await?;
        if updated {
            computed += 1;
        } else {
            skipped += 1;
        }
    }

    println!(
        "computed cache savings for {computed} of {total} model_call events ({skipped} still missing — no catalog rate found for their provider/model)"
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
