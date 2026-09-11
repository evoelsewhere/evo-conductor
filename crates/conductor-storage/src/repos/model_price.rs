use chrono::{DateTime, Utc};
use conductor_domain::{normalize_model_key, ModelPricing, ModelRates};
use sqlx::Row;
use sqlx::{Any, Pool};

use crate::core::dialect::DatabaseKind;
use crate::core::error::StorageResult;
use crate::core::mapping::parse_dt;

/// One synced snapshot of the upstream price catalog. `version` is derived
/// from the payload, so re-syncing an unchanged catalog is recognisable
/// without diffing every model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceCatalogSnapshot {
    pub version: String,
    pub source: String,
    pub source_url: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub model_count: u32,
    pub priced_model_count: u32,
    pub changed_model_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPriceRow {
    pub provider: String,
    pub model: String,
    pub pricing: ModelPricing,
}

/// A price looked up for a point in time, carrying the catalog it came from.
///
/// The version travels with the rates deliberately. An event reported before
/// the newest sync is priced from a historical row, and stamping it with
/// whatever catalog happens to be current would misattribute the cost to a
/// catalog that never priced it -- the audit trail would say the wrong thing
/// precisely for the rows where it matters most.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricingAt {
    pub pricing: ModelPricing,
    pub catalog_version: String,
    pub effective_from: DateTime<Utc>,
}

#[derive(Clone)]
pub struct ModelPriceRepo {
    pool: Pool<Any>,
    kind: DatabaseKind,
}

impl ModelPriceRepo {
    pub fn new(pool: Pool<Any>, kind: DatabaseKind) -> Self {
        Self { pool, kind }
    }

    pub async fn record_catalog(&self, snapshot: &PriceCatalogSnapshot) -> StorageResult<()> {
        sqlx::query(
            r#"
            INSERT INTO model_price_catalogs (
                version, source, source_url, fetched_at, model_count,
                priced_model_count, changed_model_count
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&snapshot.version)
        .bind(&snapshot.source)
        .bind(snapshot.source_url.as_deref())
        .bind(snapshot.fetched_at.to_rfc3339())
        .bind(i64::from(snapshot.model_count))
        .bind(i64::from(snapshot.priced_model_count))
        .bind(i64::from(snapshot.changed_model_count))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn latest_catalog(&self) -> StorageResult<Option<PriceCatalogSnapshot>> {
        let row = sqlx::query(
            r#"
            SELECT version, source, source_url, fetched_at, model_count,
                   priced_model_count, changed_model_count
            FROM model_price_catalogs
            ORDER BY fetched_at DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await?;
        row.map(map_catalog).transpose()
    }

    /// Append rates effective from `effective_from`.
    ///
    /// Rows are never updated in place: a cost must stay explainable by the
    /// rate that was in force when its event was reported, so a price change
    /// adds a row rather than rewriting the old one. Re-inserting the same
    /// `(provider, model, effective_from)` is ignored, which makes a retried
    /// sync harmless.
    pub async fn insert_prices(
        &self,
        catalog_version: &str,
        effective_from: DateTime<Utc>,
        rows: &[ModelPriceRow],
    ) -> StorageResult<u32> {
        if rows.is_empty() {
            return Ok(0);
        }
        let effective = effective_from.to_rfc3339();
        let mut inserted = 0u32;
        let insert = match self.kind {
            DatabaseKind::Mysql => {
                r#"
                INSERT INTO model_prices (
                    provider, model, effective_from, catalog_version,
                    input_micro_per_million, output_micro_per_million,
                    cache_read_micro_per_million, cache_write_micro_per_million,
                    reasoning_micro_per_million, tiers_json, service_tiers_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE provider = provider
                "#
            }
            DatabaseKind::Sqlite | DatabaseKind::Postgres => {
                r#"
                INSERT INTO model_prices (
                    provider, model, effective_from, catalog_version,
                    input_micro_per_million, output_micro_per_million,
                    cache_read_micro_per_million, cache_write_micro_per_million,
                    reasoning_micro_per_million, tiers_json, service_tiers_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT (provider, model, effective_from) DO NOTHING
                "#
            }
        };
        let mut tx = self.pool.begin().await?;
        for row in rows {
            let affected = sqlx::query(insert)
                .bind(normalize_model_key(&row.provider))
                .bind(normalize_model_key(&row.model))
                .bind(&effective)
                .bind(catalog_version)
                .bind(row.pricing.base.input)
                .bind(row.pricing.base.output)
                .bind(row.pricing.base.cache_read)
                .bind(row.pricing.base.cache_write)
                .bind(row.pricing.base.reasoning)
                .bind(encode_list(&row.pricing.tiers))
                .bind(encode_list(&row.pricing.service_tiers))
                .execute(&mut *tx)
                .await?
                .rows_affected();
            inserted += u32::from(affected > 0);
        }
        tx.commit().await?;
        Ok(inserted)
    }

    /// The price in force for a model at `at` — the newest row whose
    /// `effective_from` is not in the future relative to the event, together
    /// with the catalog that published it.
    pub async fn pricing_at(
        &self,
        provider: &str,
        model: &str,
        at: DateTime<Utc>,
    ) -> StorageResult<Option<PricingAt>> {
        let row = sqlx::query(
            r#"
            SELECT input_micro_per_million, output_micro_per_million,
                   cache_read_micro_per_million, cache_write_micro_per_million,
                   reasoning_micro_per_million, tiers_json, service_tiers_json,
                   catalog_version, effective_from
            FROM model_prices
            WHERE provider = ? AND model = ? AND effective_from <= ?
            ORDER BY effective_from DESC
            LIMIT 1
            "#,
        )
        .bind(normalize_model_key(provider))
        .bind(normalize_model_key(model))
        .bind(at.to_rfc3339())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(map_pricing_at))
    }

    /// The oldest price ever recorded for a model. Used only to *estimate* an
    /// event that predates the first sync — models.dev publishes no history,
    /// so this is the closest thing to a period-correct rate that exists.
    pub async fn earliest_pricing(
        &self,
        provider: &str,
        model: &str,
    ) -> StorageResult<Option<PricingAt>> {
        let row = sqlx::query(
            r#"
            SELECT input_micro_per_million, output_micro_per_million,
                   cache_read_micro_per_million, cache_write_micro_per_million,
                   reasoning_micro_per_million, tiers_json, service_tiers_json,
                   catalog_version, effective_from
            FROM model_prices
            WHERE provider = ? AND model = ?
            ORDER BY effective_from ASC
            LIMIT 1
            "#,
        )
        .bind(normalize_model_key(provider))
        .bind(normalize_model_key(model))
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(map_pricing_at))
    }

    /// Newest rate per model, for diffing a fresh sync and for building the
    /// in-memory table used while pricing incoming events.
    pub async fn current_rate_table(&self) -> StorageResult<Vec<ModelPriceRow>> {
        Ok(self
            .current_rate_table_with_effective_from()
            .await?
            .into_iter()
            .map(|(row, _)| row)
            .collect())
    }

    /// As [`Self::current_rate_table`], but keeping each row's
    /// `effective_from`. The caller needs the newest one to know when the
    /// in-memory table stops being authoritative for an older event.
    pub async fn current_rate_table_with_effective_from(
        &self,
    ) -> StorageResult<Vec<(ModelPriceRow, DateTime<Utc>)>> {
        let rows = sqlx::query(
            r#"
            SELECT p.provider, p.model, p.effective_from, p.input_micro_per_million,
                   p.output_micro_per_million, p.cache_read_micro_per_million,
                   p.cache_write_micro_per_million, p.reasoning_micro_per_million,
                   p.tiers_json, p.service_tiers_json
            FROM model_prices p
            WHERE p.effective_from = (
                SELECT MAX(inner_price.effective_from)
                FROM model_prices inner_price
                WHERE inner_price.provider = p.provider
                  AND inner_price.model = p.model
            )
            ORDER BY p.provider ASC, p.model ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let effective: String = row.get("effective_from");
                (
                    ModelPriceRow {
                        provider: row.get("provider"),
                        model: row.get("model"),
                        pricing: map_pricing(&row),
                    },
                    parse_dt(effective),
                )
            })
            .collect())
    }
}

fn map_rates(row: &sqlx::any::AnyRow) -> ModelRates {
    ModelRates {
        input: row.get("input_micro_per_million"),
        output: row.get("output_micro_per_million"),
        cache_read: row.get("cache_read_micro_per_million"),
        cache_write: row.get("cache_write_micro_per_million"),
        reasoning: row.get("reasoning_micro_per_million"),
    }
}

fn map_pricing(row: &sqlx::any::AnyRow) -> ModelPricing {
    ModelPricing {
        base: map_rates(row),
        tiers: decode_list(row.get("tiers_json")),
        service_tiers: decode_list(row.get("service_tiers_json")),
    }
    .sorted()
}

fn map_pricing_at(row: sqlx::any::AnyRow) -> PricingAt {
    let effective_from: String = row.get("effective_from");
    PricingAt {
        pricing: map_pricing(&row),
        catalog_version: row.get("catalog_version"),
        effective_from: parse_dt(effective_from),
    }
}

/// Serialize a band or lane list, storing `NULL` rather than `[]` for the
/// overwhelming majority of models that have neither.
fn encode_list<T: serde::Serialize>(values: &[T]) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    serde_json::to_string(values).ok()
}

/// Decode a band or lane list, treating unreadable JSON as "no bands".
///
/// Failing the whole price lookup would be worse: a model would go from
/// priced-at-the-headline-rate to entirely unpriced because of one malformed
/// column, and `unpriced_model_calls` would blame a missing rate rather than
/// a decode fault. Upstream shapes change; the headline rates are stored in
/// their own columns and stay usable regardless.
fn decode_list<T: serde::de::DeserializeOwned>(raw: Option<String>) -> Vec<T> {
    raw.as_deref()
        .filter(|value| !value.trim().is_empty())
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or_default()
}

fn map_catalog(row: sqlx::any::AnyRow) -> StorageResult<PriceCatalogSnapshot> {
    let fetched_at: String = row.try_get("fetched_at")?;
    Ok(PriceCatalogSnapshot {
        version: row.try_get("version")?,
        source: row.try_get("source")?,
        source_url: row.try_get("source_url")?,
        fetched_at: parse_dt(fetched_at),
        model_count: non_negative(row.try_get("model_count")?),
        priced_model_count: non_negative(row.try_get("priced_model_count")?),
        changed_model_count: non_negative(row.try_get("changed_model_count")?),
    })
}

fn non_negative(value: i64) -> u32 {
    value.clamp(0, i64::from(u32::MAX)) as u32
}
