//! Reprice stored model calls from Conductor's rate table.
//!
//! Two things make this less trivial than it looks:
//!
//! 1. Rate rows are effective from the moment they were synced, and models.dev
//!    publishes no history, so an event older than the first sync has no rate
//!    covering it. Strict mode leaves those unpriced; [`Basis::EarliestKnown`]
//!    prices them from the oldest rate on record and marks them as estimates,
//!    which is a different claim and is stored as such.
//! 2. Only rows Conductor has never priced are touched. Rewriting a cost that
//!    was computed from the rate in force would silently change a figure a
//!    member's receipt already reconciled against.

use conductor_domain::{price_model_call_tiered, PricedCost, PricingBasis, UnpricedReason};
use conductor_storage::repos::{RepriceCandidate, RepricedCost};
use conductor_storage::{Db, StorageError};
use uuid::Uuid;

/// Rows per transaction. Small enough to keep a lock window short on a table
/// that ingest is still writing to.
const BATCH: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Price only where a rate covers the event.
    InForceOnly,
    /// Also estimate pre-catalog events from the earliest rate on record.
    EarliestKnownFallback,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepriceReport {
    pub examined: u64,
    pub priced_in_force: u64,
    pub priced_from_earliest: u64,
    /// Still unpriced: no rate covers the event, and either the fallback was
    /// not requested or the model is absent from the catalog entirely.
    pub left_unpriced: u64,
}

pub async fn run(db: &Db, project_id: Uuid, basis: Basis) -> Result<RepriceReport, StorageError> {
    if db.model_prices().latest_catalog().await?.is_none() {
        // Nothing to price from; reporting zero beats writing "unpriced"
        // reasons that a later sync would immediately invalidate.
        return Ok(RepriceReport::default());
    }

    let mut report = RepriceReport::default();
    let mut after_id: Option<Uuid> = None;
    loop {
        let candidates = db
            .telemetry()
            .unpriced_model_calls(project_id, after_id, BATCH)
            .await?;
        if candidates.is_empty() {
            break;
        }
        after_id = candidates.last().map(|candidate| candidate.id);

        let mut priced = Vec::with_capacity(candidates.len());
        for candidate in &candidates {
            report.examined += 1;
            let resolved = resolve(db, candidate, basis).await?;
            match resolved {
                Some(resolved) if resolved.cost.cost_micros().is_some() => {
                    match resolved.basis {
                        PricingBasis::InForce => report.priced_in_force += 1,
                        PricingBasis::EarliestKnown => report.priced_from_earliest += 1,
                    }
                    priced.push(RepricedCost {
                        id: candidate.id,
                        cost: resolved.cost,
                        // The catalog that published the rate actually used,
                        // not whichever is newest: an `earliest_known`
                        // estimate stamped with today's catalog would claim a
                        // provenance it does not have.
                        catalog_version: Some(resolved.catalog_version),
                        basis: Some(resolved.basis),
                    });
                }
                _ => {
                    report.left_unpriced += 1;
                    // Recording the reason makes unpriced volume explainable
                    // instead of looking like an oversight.
                    priced.push(RepricedCost {
                        id: candidate.id,
                        cost: PricedCost::Unpriced {
                            reason: UnpricedReason::ModelUnknown,
                        },
                        catalog_version: None,
                        basis: None,
                    });
                }
            }
        }
        db.telemetry().apply_repriced_costs(&priced).await?;

        if candidates.len() < BATCH as usize {
            break;
        }
    }
    Ok(report)
}

/// A repriced event, with the catalog that actually supplied its rate.
struct Resolved {
    cost: PricedCost,
    basis: PricingBasis,
    catalog_version: String,
}

async fn resolve(
    db: &Db,
    candidate: &RepriceCandidate,
    basis: Basis,
) -> Result<Option<Resolved>, StorageError> {
    let (Some(provider), Some(model)) = (candidate.provider.as_deref(), candidate.model.as_deref())
    else {
        return Ok(None);
    };
    let prices = db.model_prices();
    let service_tier = candidate.service_tier.as_deref();

    if let Some(found) = prices
        .pricing_at(provider, model, candidate.reported_at)
        .await?
    {
        return Ok(Some(Resolved {
            cost: price_model_call_tiered(&found.pricing, candidate.usage, service_tier),
            basis: PricingBasis::InForce,
            catalog_version: found.catalog_version,
        }));
    }
    if basis == Basis::InForceOnly {
        return Ok(None);
    }
    Ok(prices
        .earliest_pricing(provider, model)
        .await?
        .map(|found| Resolved {
            cost: price_model_call_tiered(&found.pricing, candidate.usage, service_tier),
            basis: PricingBasis::EarliestKnown,
            catalog_version: found.catalog_version,
        }))
}
