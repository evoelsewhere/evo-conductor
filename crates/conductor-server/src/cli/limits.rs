use chrono::Utc;
use conductor_domain::{validate_warn_percent, LimitPeriod, LimitScope, PrimaryRole};
use conductor_storage::repos::{parse_role_subject, SpendLimit, UpsertSpendLimit};
use uuid::Uuid;

use crate::AppState;

use super::{micros_to_usd, parse_usd_micros, project_id, Flags};

/// Warn at four fifths of the allowance unless told otherwise: late enough
/// not to be noise, early enough to still act on.
const DEFAULT_WARN_PERCENT: u32 = 80;

const SET_FLAGS: &[&str] = &["scope", "subject", "period", "limit", "warn", "disabled"];
const RM_FLAGS: &[&str] = &["scope", "subject", "period"];

pub async fn run(args: &[String], state: &AppState) -> anyhow::Result<()> {
    match args.first().map(String::as_str) {
        None => report(state).await,
        Some("list") => list(state).await,
        Some("set") => set(&args[1..], state).await,
        Some("rm") => remove(&args[1..], state).await,
        Some(other) => {
            anyhow::bail!("unknown limits subcommand {other:?}; supported: list, set, rm")
        }
    }
}

/// Report every configured limit against its own current period.
///
/// Read-only on purpose. A limit detects; it cannot stop spending, because
/// EvoFlux calls providers directly — see `conductor_domain::spend`.
async fn report(state: &AppState) -> anyhow::Result<()> {
    let statuses = state
        .db
        .spend_limits()
        .statuses(project_id(state).await?, Utc::now())
        .await?;
    if statuses.is_empty() {
        println!("no enabled spend limits; `limits list` shows disabled ones");
        return Ok(());
    }
    for status in statuses {
        println!(
            "{:<8} {:<40} {:<6} {:>10} / {:>10} usd  {:>4}%  {}",
            status.limit.scope.as_str(),
            subject_column(&status.limit),
            status.limit.period.as_str(),
            micros_to_usd(status.evaluation.spent_usd_micros),
            micros_to_usd(status.evaluation.limit_usd_micros),
            status.evaluation.used_percent,
            status.evaluation.state.as_str(),
        );
    }
    Ok(())
}

/// Every allowance as configured, including the disabled ones the report
/// skips — otherwise a limit switched off is indistinguishable from one that
/// was never created.
async fn list(state: &AppState) -> anyhow::Result<()> {
    let limits = state
        .db
        .spend_limits()
        .list(project_id(state).await?)
        .await?;
    if limits.is_empty() {
        println!("no spend limits configured");
        return Ok(());
    }
    for limit in limits {
        println!(
            "{:<8} {:<40} {:<6} {:>10} usd  warn {:>3}%  {}",
            limit.scope.as_str(),
            subject_column(&limit),
            limit.period.as_str(),
            micros_to_usd(limit.limit_usd_micros),
            limit.warn_percent,
            if limit.enabled { "enabled" } else { "disabled" },
        );
    }
    Ok(())
}

/// Create or replace one allowance.
///
/// A replace, not a merge: re-running `set` without `--disabled` enables the
/// limit again, and without `--warn` restores the default threshold. Every
/// field the command accepts is written, so what was typed is what is stored.
async fn set(args: &[String], state: &AppState) -> anyhow::Result<()> {
    let flags = Flags::parse(args, SET_FLAGS)?;
    let scope = parse_scope(flags.required("scope")?)?;
    let period = parse_period(flags.required("period")?)?;
    let limit_usd_micros = parse_usd_micros(flags.required("limit")?)?;
    let warn_percent = match flags.optional("warn")? {
        Some(value) => value
            .parse::<u32>()
            .map_err(|_| anyhow::anyhow!("--warn takes a whole percentage, got {value:?}"))?,
        None => DEFAULT_WARN_PERCENT,
    };
    validate_warn_percent(warn_percent).map_err(|message| anyhow::anyhow!(message))?;

    let subject = resolve_subject(state, parse_subject(scope, flags.optional("subject")?)?).await?;
    let enabled = !flags.is_set("disabled");
    let stored = state
        .db
        .spend_limits()
        .upsert(
            project_id(state).await?,
            &UpsertSpendLimit {
                scope,
                subject_id: subject.id,
                period,
                limit_usd_micros,
                warn_percent,
                enabled,
            },
        )
        .await?;

    // Echoed back so the operator can see how the amount parsed before
    // discovering it at the wrong order of magnitude a month later.
    println!(
        "set {} {} {}: {} usd, warn at {}%, {}",
        stored.scope.as_str(),
        subject.label,
        stored.period.as_str(),
        micros_to_usd(stored.limit_usd_micros),
        stored.warn_percent,
        if stored.enabled {
            "enabled"
        } else {
            "disabled"
        },
    );
    Ok(())
}

async fn remove(args: &[String], state: &AppState) -> anyhow::Result<()> {
    let flags = Flags::parse(args, RM_FLAGS)?;
    let scope = parse_scope(flags.required("scope")?)?;
    let period = parse_period(flags.required("period")?)?;
    // Not resolved against the member table: a limit for a member who has
    // since been deleted must still be removable.
    let subject = parse_subject(scope, flags.optional("subject")?)?.id();

    let removed = state
        .db
        .spend_limits()
        .delete(project_id(state).await?, scope, &subject, period)
        .await?;
    if removed {
        println!(
            "removed {} {} {}",
            scope.as_str(),
            display(&subject),
            period.as_str()
        );
    } else {
        println!(
            "no {} {} {} limit to remove",
            scope.as_str(),
            display(&subject),
            period.as_str()
        );
    }
    Ok(())
}

/// What a limit applies to, once the scope and `--subject` agree.
enum SubjectSpec {
    WholeProject,
    Role(String),
    Member(Uuid),
}

impl SubjectSpec {
    fn id(&self) -> String {
        match self {
            Self::WholeProject => String::new(),
            Self::Role(role) => role.clone(),
            Self::Member(id) => id.to_string(),
        }
    }
}

struct Subject {
    id: String,
    label: String,
}

/// Check `--subject` against the scope before anything is written.
///
/// A role that is not a role, or a subject on a project-wide limit, would
/// create an allowance matching nobody: it would report zero spend for ever,
/// which reads exactly like a subject who has not spent.
fn parse_subject(scope: LimitScope, given: Option<&str>) -> anyhow::Result<SubjectSpec> {
    match (scope, given) {
        (LimitScope::Project, None) => Ok(SubjectSpec::WholeProject),
        (LimitScope::Project, Some(value)) => anyhow::bail!(
            "a project limit covers everyone; drop --subject {value:?} or use --scope member"
        ),
        (scope, None) => anyhow::bail!("--subject is required for a {} limit", scope.as_str()),
        (LimitScope::Role, Some(value)) => parse_role_subject(value)
            .map(SubjectSpec::Role)
            .ok_or_else(|| anyhow::anyhow!("{value:?} is not a role; expected {}", role_names())),
        (LimitScope::Member, Some(value)) => Uuid::parse_str(value)
            .map(SubjectSpec::Member)
            .map_err(|_| anyhow::anyhow!("--subject must be a member id, got {value:?}")),
    }
}

/// Confirm a member subject exists, and name it in the confirmation line so a
/// mistyped id is caught while it can still be corrected.
async fn resolve_subject(state: &AppState, spec: SubjectSpec) -> anyhow::Result<Subject> {
    match spec {
        SubjectSpec::WholeProject => Ok(Subject {
            id: String::new(),
            label: "(everyone)".to_string(),
        }),
        SubjectSpec::Role(role) => Ok(Subject {
            label: role.clone(),
            id: role,
        }),
        SubjectSpec::Member(id) => match state.db.users().find_by_id(id).await? {
            Some(user) => Ok(Subject {
                id: user.id.to_string(),
                label: format!("{} <{}>", user.display_name, user.email),
            }),
            None => anyhow::bail!("no member with id {id}"),
        },
    }
}

fn parse_scope(value: &str) -> anyhow::Result<LimitScope> {
    LimitScope::parse(value)
        .ok_or_else(|| anyhow::anyhow!("--scope must be project, member or role, got {value:?}"))
}

fn parse_period(value: &str) -> anyhow::Result<LimitPeriod> {
    LimitPeriod::parse(value)
        .ok_or_else(|| anyhow::anyhow!("--period must be day, week or month, got {value:?}"))
}

fn role_names() -> String {
    PrimaryRole::ALL
        .iter()
        .map(|role| role.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn display(subject_id: &str) -> &str {
    if subject_id.is_empty() {
        "(everyone)"
    } else {
        subject_id
    }
}

fn subject_column(limit: &SpendLimit) -> &str {
    display(&limit.subject_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_limit_takes_no_subject_and_a_member_limit_requires_one() {
        assert!(matches!(
            parse_subject(LimitScope::Project, None).unwrap(),
            SubjectSpec::WholeProject
        ));
        assert!(parse_subject(LimitScope::Project, Some("someone")).is_err());
        assert!(parse_subject(LimitScope::Member, None).is_err());
        assert!(parse_subject(LimitScope::Role, None).is_err());
    }

    /// A role limit stores the role name, so a typo has to be caught here.
    #[test]
    fn a_role_subject_must_name_a_real_role() {
        let spec = parse_subject(LimitScope::Role, Some("admin")).unwrap();
        assert_eq!(spec.id(), PrimaryRole::Admin.as_str());
        assert!(parse_subject(LimitScope::Role, Some("administrator")).is_err());
        assert!(parse_subject(LimitScope::Role, Some("Admin")).is_err());
    }

    #[test]
    fn a_member_subject_must_be_an_id_rather_than_a_name() {
        let id = Uuid::new_v4();
        assert_eq!(
            parse_subject(LimitScope::Member, Some(&id.to_string()))
                .unwrap()
                .id(),
            id.to_string()
        );
        assert!(parse_subject(LimitScope::Member, Some("nguyen@example.com")).is_err());
    }

    #[test]
    fn a_project_subject_is_the_empty_string_the_unique_key_expects() {
        assert_eq!(parse_subject(LimitScope::Project, None).unwrap().id(), "");
    }
}
