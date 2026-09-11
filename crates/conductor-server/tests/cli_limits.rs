//! `limits set` is the only way to create an allowance, so these exercise the
//! command end to end: what it writes, and — more important — what it refuses
//! to write. A limit aimed at nobody reports zero spend for ever, which reads
//! exactly like a subject who has not spent.

mod support;

use chrono::Utc;
use conductor_domain::{LimitPeriod, LimitScope, PrimaryRole};
use conductor_server::cli;
use support::{test_app, TestApp};
use uuid::Uuid;

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

async fn run(app: &TestApp, values: &[&str]) -> anyhow::Result<()> {
    cli::run(&args(values), &app.state).await
}

async fn limit_count(app: &TestApp, project_id: Uuid) -> usize {
    app.state
        .db
        .spend_limits()
        .list(project_id)
        .await
        .expect("list")
        .len()
}

#[tokio::test]
async fn set_creates_a_member_allowance_the_report_then_evaluates() {
    let app = test_app().await;
    let project_id = app.seed_project_identity().await;
    let member = app.seed_user(PrimaryRole::User).await;

    run(
        &app,
        &[
            "limits",
            "set",
            "--scope",
            "member",
            "--subject",
            &member.id.to_string(),
            "--period",
            "month",
            "--limit",
            "12.50",
        ],
    )
    .await
    .expect("set");

    let statuses = app
        .state
        .db
        .spend_limits()
        .statuses(project_id, Utc::now())
        .await
        .expect("statuses");
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].limit.subject_id, member.id.to_string());
    assert_eq!(
        statuses[0].evaluation.limit_usd_micros, 12_500_000,
        "12.50 dollars must land as micro-dollars, not as 12 or 1250"
    );
    assert_eq!(statuses[0].limit.warn_percent, 80);
    assert!(statuses[0].limit.enabled);
}

/// The failure this guards against is silent: a limit on an id nobody holds
/// never sums anything, so it looks like a member who simply behaves.
#[tokio::test]
async fn a_member_who_does_not_exist_is_refused_before_anything_is_written() {
    let app = test_app().await;
    let project_id = app.seed_project_identity().await;

    let error = run(
        &app,
        &[
            "limits",
            "set",
            "--scope",
            "member",
            "--subject",
            &Uuid::new_v4().to_string(),
            "--period",
            "day",
            "--limit",
            "5",
        ],
    )
    .await
    .expect_err("an unknown member must not get an allowance");
    assert!(error.to_string().contains("no member"));
    assert_eq!(limit_count(&app, project_id).await, 0);
}

#[tokio::test]
async fn a_project_limit_needs_no_subject_and_refuses_one() {
    let app = test_app().await;
    let project_id = app.seed_project_identity().await;

    run(
        &app,
        &[
            "limits", "set", "--scope", "project", "--period", "week", "--limit", "1000",
        ],
    )
    .await
    .expect("project limit");
    let limits = app
        .state
        .db
        .spend_limits()
        .list(project_id)
        .await
        .expect("list");
    assert_eq!(limits.len(), 1);
    assert_eq!(limits[0].scope, LimitScope::Project);
    assert_eq!(
        limits[0].subject_id, "",
        "the unique key expects an empty subject for a project limit"
    );

    assert!(run(
        &app,
        &[
            "limits",
            "set",
            "--scope",
            "project",
            "--subject",
            "someone",
            "--period",
            "week",
            "--limit",
            "1000",
        ],
    )
    .await
    .is_err());
    assert_eq!(limit_count(&app, project_id).await, 1, "still just the one");
}

/// `set` replaces rather than merges, so a second run is the way to change an
/// allowance — and re-enabling one is a `set` without `--disabled`.
#[tokio::test]
async fn setting_the_same_scope_and_period_again_replaces_the_allowance() {
    let app = test_app().await;
    let project_id = app.seed_project_identity().await;
    let member = app.seed_user(PrimaryRole::User).await;
    let subject = member.id.to_string();

    run(
        &app,
        &[
            "limits",
            "set",
            "--scope",
            "member",
            "--subject",
            &subject,
            "--period",
            "month",
            "--limit",
            "10",
        ],
    )
    .await
    .expect("first");
    run(
        &app,
        &[
            "limits",
            "set",
            "--scope",
            "member",
            "--subject",
            &subject,
            "--period",
            "month",
            "--limit",
            "250",
            "--warn",
            "50",
            "--disabled",
        ],
    )
    .await
    .expect("second");

    let limits = app
        .state
        .db
        .spend_limits()
        .list(project_id)
        .await
        .expect("list");
    assert_eq!(limits.len(), 1, "replaced, not duplicated");
    assert_eq!(limits[0].limit_usd_micros, 250_000_000);
    assert_eq!(limits[0].warn_percent, 50);
    assert!(!limits[0].enabled);
    assert!(
        app.state
            .db
            .spend_limits()
            .statuses(project_id, Utc::now())
            .await
            .expect("statuses")
            .is_empty(),
        "a disabled limit is configured but not evaluated"
    );
}

#[tokio::test]
async fn rm_removes_the_allowance_it_names() {
    let app = test_app().await;
    let project_id = app.seed_project_identity().await;

    run(
        &app,
        &[
            "limits",
            "set",
            "--scope",
            "role",
            "--subject",
            "admin",
            "--period",
            "day",
            "--limit",
            "40",
        ],
    )
    .await
    .expect("set");
    run(
        &app,
        &[
            "limits",
            "rm",
            "--scope",
            "role",
            "--subject",
            "admin",
            "--period",
            "day",
        ],
    )
    .await
    .expect("rm");
    assert_eq!(limit_count(&app, project_id).await, 0);
    run(
        &app,
        &[
            "limits",
            "rm",
            "--scope",
            "role",
            "--subject",
            "admin",
            "--period",
            "day",
        ],
    )
    .await
    .expect("removing what is gone is not an error");
}

/// A limit written by mistake is worse than one refused: nothing warns that a
/// stored allowance means something other than what was typed.
#[tokio::test]
async fn a_bad_argument_leaves_the_project_without_a_limit() {
    let app = test_app().await;
    let project_id = app.seed_project_identity().await;
    let subject = app.seed_user(PrimaryRole::User).await.id.to_string();

    let rejected: &[&[&str]] = &[
        // Above 100 no warning can ever fire.
        &["--warn", "800"],
        // Finer than a micro-dollar would be stored rounded.
        &["--limit", "1.1234567"],
        // Not a dollar amount at all.
        &["--limit", "$50"],
        // A typo must not quietly leave the default in place.
        &["--warm", "90"],
    ];
    for extra in rejected {
        let mut command = vec![
            "limits",
            "set",
            "--scope",
            "member",
            "--subject",
            &subject,
            "--period",
            "month",
            "--limit",
            "100",
        ];
        command.extend_from_slice(extra);
        assert!(
            run(&app, &command).await.is_err(),
            "{extra:?} should be refused"
        );
    }
    assert_eq!(limit_count(&app, project_id).await, 0);

    for scope in ["everyone", "team"] {
        assert!(run(
            &app,
            &["limits", "set", "--scope", scope, "--period", "month", "--limit", "100",],
        )
        .await
        .is_err());
    }
    for period in ["year", "hour"] {
        assert!(run(
            &app,
            &["limits", "set", "--scope", "project", "--period", period, "--limit", "100",],
        )
        .await
        .is_err());
    }
    assert_eq!(limit_count(&app, project_id).await, 0);
}

/// A role limit stores the role name the spend query matches on, so an
/// unrecognised role would match no telemetry row at all.
#[tokio::test]
async fn a_role_limit_stores_the_canonical_role_and_refuses_anything_else() {
    let app = test_app().await;
    let project_id = app.seed_project_identity().await;

    run(
        &app,
        &[
            "limits",
            "set",
            "--scope",
            "role",
            "--subject",
            "contribute",
            "--period=month",
            "--limit=75.25",
        ],
    )
    .await
    .expect("role limit");
    let limits = app
        .state
        .db
        .spend_limits()
        .list(project_id)
        .await
        .expect("list");
    assert_eq!(limits[0].subject_id, PrimaryRole::Contribute.as_str());
    assert_eq!(limits[0].period, LimitPeriod::Month);
    assert_eq!(limits[0].limit_usd_micros, 75_250_000);

    assert!(run(
        &app,
        &[
            "limits",
            "set",
            "--scope",
            "role",
            "--subject",
            "contributor",
            "--period",
            "month",
            "--limit",
            "75",
        ],
    )
    .await
    .is_err());
    assert_eq!(limit_count(&app, project_id).await, 1);
}

#[tokio::test]
async fn an_operator_command_without_a_project_says_so_rather_than_writing_anything() {
    let app = test_app().await;
    let error = run(
        &app,
        &[
            "limits", "set", "--scope", "project", "--period", "day", "--limit", "10",
        ],
    )
    .await
    .expect_err("setup has not run");
    assert!(error.to_string().contains("no project configured"));
}
