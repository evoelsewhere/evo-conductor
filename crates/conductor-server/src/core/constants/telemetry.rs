//! Telemetry transport limits and query defaults.

pub const MIN_BATCH_SIZE: usize = 1;
pub const MAX_BATCH_SIZE: usize = 100;
pub const MAX_RESOURCE_ATTRIBUTIONS_PER_EVENT: usize = 16;
pub const MIN_LABEL_LENGTH: usize = 1;
pub const MAX_LABEL_LENGTH: usize = 256;
pub const DEFAULT_RANGE_DAYS: i64 = 30;
pub const MIN_ACTIVITY_LIMIT: u32 = 1;
pub const DEFAULT_ACTIVITY_LIMIT: u32 = 50;
pub const MAX_ACTIVITY_LIMIT: u32 = 100;
pub const MAX_FUTURE_CLOCK_SKEW_MINUTES: i64 = 5;
