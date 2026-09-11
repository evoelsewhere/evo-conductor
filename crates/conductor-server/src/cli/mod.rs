//! Operator subcommands.
//!
//! Each runs once and exits rather than starting the listener. Spend limits
//! and repricing are also reachable over HTTP; these stay because an operator
//! locked out of the console still needs them, and because repricing a long
//! backlog outlives the patience of a browser or a proxy.

mod limits;
mod reprice;
mod route_inventory;

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::AppState;

const MICROS_PER_USD: u64 = 1_000_000;

pub const USAGE: &str = "\
evo-conductor <command>

  reprice [--estimate-pre-catalog]
      Put a cost on model calls Conductor has none for yet.

  limits
      Report every enabled limit against its own current period.

  limits list
      Every configured limit, disabled ones included.

  limits set --scope <project|member|role> [--subject <member-id|role>]
             --period <day|week|month> --limit <usd> [--warn <0-100>] [--disabled]
      Create or replace one allowance.

  limits rm --scope <project|member|role> [--subject <member-id|role>]
            --period <day|week|month>
      Remove one allowance.

  route-inventory [--write]
      Render the authorization route inventory; --write updates the
      snapshot the drift test compares against.
";

/// Whether the arguments only ask what the commands are, which must be
/// answerable without a database.
pub fn wants_usage(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h")
    )
}

pub async fn run(args: &[String], state: &AppState) -> anyhow::Result<()> {
    match args.first().map(String::as_str) {
        Some("reprice") => reprice::run(&args[1..], state).await,
        Some("limits") => limits::run(&args[1..], state).await,
        Some("route-inventory") => route_inventory::run(&args[1..]).await,
        Some("help" | "--help" | "-h") | None => {
            print!("{USAGE}");
            Ok(())
        }
        Some(other) => anyhow::bail!("unknown command {other:?}\n\n{USAGE}"),
    }
}

/// Every operator command is scoped to the one project this instance serves.
pub async fn project_id(state: &AppState) -> anyhow::Result<Uuid> {
    state
        .db
        .instance()
        .authorization_project_id()
        .await?
        .ok_or_else(|| anyhow::anyhow!("no project configured; complete setup first"))
}

/// A `--key value` / `--key=value` parser, sized for these few commands.
///
/// Unknown keys are refused rather than ignored: a typo like `--warm 90`
/// would otherwise leave the default in place while the operator believed
/// they had changed it.
#[derive(Debug)]
pub struct Flags(BTreeMap<String, Option<String>>);

impl Flags {
    pub fn parse(args: &[String], allowed: &[&str]) -> anyhow::Result<Self> {
        let mut parsed: BTreeMap<String, Option<String>> = BTreeMap::new();
        let mut index = 0;
        while index < args.len() {
            let token = &args[index];
            let Some(rest) = token.strip_prefix("--") else {
                anyhow::bail!("unexpected argument {token:?}; flags look like --key value");
            };
            let (key, value) = match rest.split_once('=') {
                Some((key, value)) => {
                    index += 1;
                    (key, Some(value.to_string()))
                }
                None => match args.get(index + 1) {
                    // A value never starts with a dash pair, so the next
                    // token belongs to this flag only if it is not itself one.
                    Some(next) if !next.starts_with("--") => {
                        index += 2;
                        (rest, Some(next.clone()))
                    }
                    _ => {
                        index += 1;
                        (rest, None)
                    }
                },
            };
            if !allowed.contains(&key) {
                anyhow::bail!("unknown flag --{key}; supported: --{}", allowed.join(" --"));
            }
            if parsed.insert(key.to_string(), value).is_some() {
                anyhow::bail!("--{key} given twice");
            }
        }
        Ok(Self(parsed))
    }

    pub fn required(&self, key: &str) -> anyhow::Result<&str> {
        self.optional(key)?
            .ok_or_else(|| anyhow::anyhow!("--{key} is required"))
    }

    pub fn optional(&self, key: &str) -> anyhow::Result<Option<&str>> {
        match self.0.get(key) {
            Some(Some(value)) => Ok(Some(value.as_str())),
            Some(None) => anyhow::bail!("--{key} needs a value"),
            None => Ok(None),
        }
    }

    pub fn is_set(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }
}

/// Parse a plain decimal dollar amount into micro-USD.
///
/// Refuses anything finer than a micro-dollar instead of rounding it away,
/// and reads the digits directly rather than going through `f64`: an
/// allowance stored as a slightly different number than the one typed would
/// only surface when the limit fired at the wrong point.
pub fn parse_usd_micros(input: &str) -> anyhow::Result<u64> {
    let text = input.trim();
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let is_digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    if whole.is_empty() && fraction.is_empty() {
        anyhow::bail!("expected a dollar amount, got {input:?}");
    }
    if !is_digits(whole) || !is_digits(fraction) {
        anyhow::bail!("expected a plain dollar amount like 250 or 12.50, got {input:?}");
    }
    if fraction.len() > 6 {
        anyhow::bail!("{input:?} is finer than a micro-dollar; use at most 6 decimals");
    }
    let too_large = || anyhow::anyhow!("{input:?} is larger than Conductor can store");
    let dollars: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| too_large())?
    };
    let micros: u64 = format!("{fraction:0<6}").parse().unwrap_or(0);
    dollars
        .checked_mul(MICROS_PER_USD)
        .and_then(|value| value.checked_add(micros))
        .ok_or_else(too_large)
}

/// Render micro-USD exactly: two decimals where that is the whole of it, six
/// where it is not, so a figure is never shown as one nobody could have typed.
pub fn micros_to_usd(micros: u64) -> String {
    let dollars = micros / MICROS_PER_USD;
    let fraction = micros % MICROS_PER_USD;
    if fraction.is_multiple_of(10_000) {
        format!("{dollars}.{:02}", fraction / 10_000)
    } else {
        format!("{dollars}.{fraction:06}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    #[test]
    fn dollar_amounts_become_micros_without_floating_point_drift() {
        assert_eq!(parse_usd_micros("250").unwrap(), 250_000_000);
        assert_eq!(parse_usd_micros("12.50").unwrap(), 12_500_000);
        assert_eq!(parse_usd_micros("0.07").unwrap(), 70_000);
        assert_eq!(parse_usd_micros(".5").unwrap(), 500_000);
        assert_eq!(parse_usd_micros("  10  ").unwrap(), 10_000_000);
        assert_eq!(parse_usd_micros("0").unwrap(), 0);
    }

    #[test]
    fn anything_that_is_not_a_plain_positive_amount_is_refused() {
        for input in ["", ".", "-5", "1e6", "12,50", "abc", "$50", "1.2.3"] {
            assert!(
                parse_usd_micros(input).is_err(),
                "{input:?} should not parse"
            );
        }
    }

    /// Silently dropping the seventh decimal would store an allowance the
    /// operator never asked for.
    #[test]
    fn precision_finer_than_a_micro_dollar_is_refused_rather_than_rounded() {
        assert_eq!(parse_usd_micros("1.123456").unwrap(), 1_123_456);
        assert!(parse_usd_micros("1.1234567").is_err());
    }

    #[test]
    fn an_amount_too_large_to_store_is_refused_rather_than_wrapping() {
        assert!(parse_usd_micros("99999999999999999999").is_err());
        assert!(parse_usd_micros(&u64::MAX.to_string()).is_err());
    }

    #[test]
    fn micros_render_back_to_the_amount_that_produced_them() {
        for input in ["250", "12.50", "0.07", "0"] {
            let micros = parse_usd_micros(input).unwrap();
            assert_eq!(parse_usd_micros(&micros_to_usd(micros)).unwrap(), micros);
        }
        assert_eq!(micros_to_usd(250_000_000), "250.00");
        assert_eq!(micros_to_usd(70_000), "0.07");
        assert_eq!(micros_to_usd(1), "0.000001");
    }

    #[test]
    fn flags_accept_both_spellings_and_bare_switches() {
        let flags = Flags::parse(
            &args(&["--scope", "member", "--limit=12.50", "--disabled"]),
            &["scope", "limit", "disabled"],
        )
        .unwrap();
        assert_eq!(flags.required("scope").unwrap(), "member");
        assert_eq!(flags.required("limit").unwrap(), "12.50");
        assert!(flags.is_set("disabled"));
        assert!(flags.optional("scope").unwrap().is_some());
    }

    /// A bare switch must not swallow the flag that follows it.
    #[test]
    fn a_switch_does_not_consume_the_next_flag() {
        let flags = Flags::parse(
            &args(&["--disabled", "--scope", "project"]),
            &["disabled", "scope"],
        )
        .unwrap();
        assert!(flags.is_set("disabled"));
        assert_eq!(flags.required("scope").unwrap(), "project");
    }

    #[test]
    fn a_mistyped_flag_is_reported_instead_of_leaving_a_default_in_place() {
        let error = Flags::parse(&args(&["--warm", "90"]), &["warn"])
            .expect_err("a typo must not be silently ignored");
        assert!(error.to_string().contains("--warm"));
    }

    #[test]
    fn a_flag_without_its_value_is_reported_rather_than_treated_as_absent() {
        let flags = Flags::parse(&args(&["--limit"]), &["limit"]).unwrap();
        assert!(flags.required("limit").is_err());
        assert!(flags.optional("limit").is_err());
    }

    #[test]
    fn repeated_and_stray_arguments_are_refused() {
        assert!(
            Flags::parse(&args(&["--scope", "member", "--scope", "role"]), &["scope"]).is_err()
        );
        assert!(Flags::parse(&args(&["set", "--scope", "member"]), &["scope"]).is_err());
    }
}
