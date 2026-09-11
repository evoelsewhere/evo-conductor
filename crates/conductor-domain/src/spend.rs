//! Spend limits.
//!
//! These **detect**, they do not enforce. EvoFlux calls model providers
//! directly with its own credentials, so nothing Conductor does can stop a
//! call from being made — not even revoking a connection secret, which only
//! cuts off sync and telemetry. A limit therefore produces a status an
//! operator can act on, and leaves the acting to them. Client-side
//! enforcement would need EvoFlux to consult its allowance before calling,
//! which is a contract that does not exist yet.

use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitScope {
    /// Everything in the project, whoever spent it.
    Project,
    /// One member.
    Member,
    /// Everyone holding a primary role.
    Role,
}

impl LimitScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Member => "member",
            Self::Role => "role",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "project" => Some(Self::Project),
            "member" => Some(Self::Member),
            "role" => Some(Self::Role),
            _ => None,
        }
    }

    /// Whether the scope names a subject. Project-wide limits do not, and a
    /// member or role limit is meaningless without one.
    pub fn requires_subject(self) -> bool {
        !matches!(self, Self::Project)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitPeriod {
    Day,
    Week,
    Month,
}

impl LimitPeriod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "day" => Some(Self::Day),
            "week" => Some(Self::Week),
            "month" => Some(Self::Month),
            _ => None,
        }
    }

    /// Start of the period containing `now`, in UTC.
    ///
    /// UTC rather than a project timezone: telemetry `received_at` is UTC, and
    /// a limit whose window disagreed with the data it sums would drift by a
    /// day's spend at every boundary.
    pub fn start(self, now: DateTime<Utc>) -> DateTime<Utc> {
        let date = now.date_naive();
        let start_of_day = |d: chrono::NaiveDate| {
            Utc.from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default())
        };
        match self {
            Self::Day => start_of_day(date),
            // ISO weeks start on Monday.
            Self::Week => {
                let weekday = date.weekday().num_days_from_monday() as i64;
                start_of_day(date) - Duration::days(weekday)
            }
            Self::Month => start_of_day(date.with_day(1).unwrap_or(date)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitState {
    Within,
    Warning,
    Exceeded,
}

impl LimitState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Within => "within",
            Self::Warning => "warning",
            Self::Exceeded => "exceeded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitEvaluation {
    pub state: LimitState,
    pub spent_usd_micros: u64,
    pub limit_usd_micros: u64,
    /// Share of the allowance consumed, rounded down. Capped at a sane
    /// display value so a runaway member cannot render as 4 billion percent.
    pub used_percent: u32,
}

/// Reject a warning threshold that cannot mean what it says.
///
/// Above 100% the warning state is unreachable, so accepting `800` would
/// silently disable warnings for a limit whose owner believed they had set
/// one. Refusing it at the point of entry is the only place that can tell a
/// typo from an intent.
pub fn validate_warn_percent(warn_percent: u32) -> Result<(), String> {
    if warn_percent > 100 {
        return Err(format!(
            "warn_percent must be 0-100, got {warn_percent}; above 100 no warning \
             can fire before the limit is exceeded"
        ));
    }
    Ok(())
}

/// Compare spend against an allowance.
///
/// `spent >= limit` counts as exceeded, not as "exactly within": the
/// allowance is fully consumed at that point, and for a management signal it
/// is better to report that a period early than a period late.
pub fn evaluate_limit(
    spent_usd_micros: u64,
    limit_usd_micros: u64,
    warn_percent: u32,
) -> LimitEvaluation {
    let used_percent = if limit_usd_micros == 0 {
        // A zero allowance is "no spending permitted"; any spend is over it,
        // and dividing by it would panic.
        if spent_usd_micros > 0 {
            100
        } else {
            0
        }
    } else {
        let ratio = (u128::from(spent_usd_micros) * 100) / u128::from(limit_usd_micros);
        ratio.min(u128::from(u32::MAX)) as u32
    };

    let warn_at = warn_percent.min(100);
    let state = if spent_usd_micros >= limit_usd_micros {
        LimitState::Exceeded
    } else if used_percent >= warn_at {
        LimitState::Warning
    } else {
        LimitState::Within
    };

    LimitEvaluation {
        state,
        spent_usd_micros,
        limit_usd_micros,
        used_percent,
    }
}

/// Create or replace one allowance.
///
/// A replace, not a patch: every field is written, so `enabled` and
/// `warn_percent` returning to their defaults when omitted is deliberate — a
/// caller that sends half a limit gets a whole one it can see.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UpsertSpendLimitRequest {
    pub scope: LimitScope,
    /// Member id for a member limit, role name for a role limit; absent or
    /// empty for a project limit, which covers everyone.
    #[serde(default)]
    pub subject_id: Option<String>,
    pub period: LimitPeriod,
    pub limit_usd_micros: u64,
    #[serde(default = "default_warn_percent")]
    pub warn_percent: u32,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn default_warn_percent() -> u32 {
    80
}

fn enabled_by_default() -> bool {
    true
}

/// Which allowance to remove.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpendLimitSelector {
    pub scope: LimitScope,
    #[serde(default)]
    pub subject_id: Option<String>,
    pub period: LimitPeriod,
}

/// One configured allowance, with its standing against the current period.
///
/// `status` is absent for a disabled limit: it is configured but not
/// evaluated, which is different from evaluating to zero.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendLimitView {
    pub scope: LimitScope,
    pub subject_id: String,
    /// Display name for a member subject, the role for a role limit, absent
    /// for a project limit. Resolved so a UI need not fetch every member.
    pub subject_label: Option<String>,
    pub period: LimitPeriod,
    pub limit_usd_micros: u64,
    pub warn_percent: u32,
    pub enabled: bool,
    pub status: Option<SpendLimitPeriodStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SpendLimitPeriodStatus {
    pub period_start: DateTime<Utc>,
    pub state: LimitState,
    pub spent_usd_micros: u64,
    pub used_percent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendLimitListResponse {
    pub limits: Vec<SpendLimitView>,
    /// Evaluated at this instant; every period window is measured back from it.
    pub evaluated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(text)
            .expect("timestamp")
            .with_timezone(&Utc)
    }

    #[test]
    fn day_period_starts_at_utc_midnight() {
        assert_eq!(
            LimitPeriod::Day.start(at("2026-09-09T13:45:12Z")),
            at("2026-09-09T00:00:00Z")
        );
    }

    #[test]
    fn week_period_starts_on_monday() {
        // 2026-09-09 is a Wednesday; the week began on Monday the 7th.
        assert_eq!(
            LimitPeriod::Week.start(at("2026-09-09T13:45:12Z")),
            at("2026-09-07T00:00:00Z")
        );
        // A Monday is its own start, not the previous week's.
        assert_eq!(
            LimitPeriod::Week.start(at("2026-09-07T00:00:01Z")),
            at("2026-09-07T00:00:00Z")
        );
        // A Sunday belongs to the week that began six days earlier.
        assert_eq!(
            LimitPeriod::Week.start(at("2026-09-13T23:59:59Z")),
            at("2026-09-07T00:00:00Z")
        );
    }

    #[test]
    fn month_period_starts_on_the_first() {
        assert_eq!(
            LimitPeriod::Month.start(at("2026-09-09T13:45:12Z")),
            at("2026-09-01T00:00:00Z")
        );
        assert_eq!(
            LimitPeriod::Month.start(at("2026-01-31T23:59:59Z")),
            at("2026-01-01T00:00:00Z")
        );
    }

    #[test]
    fn spend_below_the_warning_threshold_is_within() {
        let evaluation = evaluate_limit(40_000_000, 100_000_000, 80);
        assert_eq!(evaluation.state, LimitState::Within);
        assert_eq!(evaluation.used_percent, 40);
    }

    #[test]
    fn spend_at_the_warning_threshold_warns() {
        assert_eq!(
            evaluate_limit(80_000_000, 100_000_000, 80).state,
            LimitState::Warning
        );
    }

    /// Consuming the allowance exactly is treated as exceeded, deliberately.
    #[test]
    fn spend_equal_to_the_limit_counts_as_exceeded() {
        let evaluation = evaluate_limit(100_000_000, 100_000_000, 80);
        assert_eq!(evaluation.state, LimitState::Exceeded);
        assert_eq!(evaluation.used_percent, 100);
    }

    #[test]
    fn a_zero_allowance_is_exceeded_by_any_spend_without_dividing_by_zero() {
        assert_eq!(evaluate_limit(0, 0, 80).state, LimitState::Exceeded);
        assert_eq!(evaluate_limit(0, 0, 80).used_percent, 0);
        let over = evaluate_limit(1, 0, 80);
        assert_eq!(over.state, LimitState::Exceeded);
        assert_eq!(over.used_percent, 100);
    }

    #[test]
    fn a_runaway_overspend_does_not_overflow_the_percentage() {
        let evaluation = evaluate_limit(u64::MAX, 1, 80);
        assert_eq!(evaluation.state, LimitState::Exceeded);
        assert_eq!(
            evaluation.used_percent,
            u32::MAX,
            "the ratio saturates rather than wrapping to a small percentage"
        );
    }

    /// A threshold at or above 100% means "do not warn me before I exceed",
    /// which is a coherent request. The evaluator honours it rather than
    /// inventing a warning the operator did not ask for; a typo like `800`
    /// is caught by [`validate_warn_percent`] on the write path instead.
    #[test]
    fn a_warning_threshold_at_or_above_a_hundred_never_warns_early() {
        assert_eq!(
            evaluate_limit(99_999_999, 100_000_000, 150).state,
            LimitState::Within
        );
        assert_eq!(
            evaluate_limit(100_000_000, 100_000_000, 150).state,
            LimitState::Exceeded
        );
    }

    #[test]
    fn a_warning_threshold_outside_the_usable_range_is_rejected_when_set() {
        assert!(validate_warn_percent(0).is_ok());
        assert!(validate_warn_percent(80).is_ok());
        assert!(validate_warn_percent(100).is_ok());
        assert!(validate_warn_percent(101).is_err());
        assert!(validate_warn_percent(800).is_err());
    }

    #[test]
    fn scope_and_period_round_trip_through_their_string_forms() {
        for scope in [LimitScope::Project, LimitScope::Member, LimitScope::Role] {
            assert_eq!(LimitScope::parse(scope.as_str()), Some(scope));
        }
        for period in [LimitPeriod::Day, LimitPeriod::Week, LimitPeriod::Month] {
            assert_eq!(LimitPeriod::parse(period.as_str()), Some(period));
        }
        assert_eq!(LimitScope::parse("everyone"), None);
        assert_eq!(LimitPeriod::parse("year"), None);
    }

    #[test]
    fn only_member_and_role_scopes_name_a_subject() {
        assert!(!LimitScope::Project.requires_subject());
        assert!(LimitScope::Member.requires_subject());
        assert!(LimitScope::Role.requires_subject());
    }
}
