//! Telemetry transport limits and query defaults.

pub const MAX_BATCH_SIZE: usize = 100;
pub const MAX_RESOURCE_ATTRIBUTIONS_PER_EVENT: usize = 16;
pub const MIN_LABEL_LENGTH: usize = 1;
pub const MAX_LABEL_LENGTH: usize = 256;
pub const DEFAULT_RANGE_DAYS: i64 = 30;
pub const MIN_ACTIVITY_LIMIT: u32 = 1;
pub const DEFAULT_ACTIVITY_LIMIT: u32 = 50;
pub const MAX_ACTIVITY_LIMIT: u32 = 100;
pub const MAX_FUTURE_CLOCK_SKEW_MINUTES: i64 = 5;

/// Per-event ceilings. Without these the wire type is the only bound, so one
/// event reporting `u64::MAX` saturates to `i64::MAX` at insert and every
/// aggregate over that member is wrong permanently.
///
/// Tokens and duration match the caps `/v1/usage/resources` already enforces,
/// so both ingest paths agree.
pub const MAX_TOKENS_PER_EVENT: u64 = 100_000_000;
pub const MAX_DURATION_MS_PER_EVENT: u64 = 86_400_000;
