//! Coverage separates "reported and used nothing" from "never reported".
//!
//! Without that distinction a silent client makes project spend look cheaper
//! rather than incomplete, and nothing in the data says which happened. These
//! tests pin the distinction itself, the closed-day rule that keeps the
//! signal usable, and the registration rule that keeps a new machine from
//! retroactively faulting history it could not have reported.

mod support;

use chrono::{DateTime, Duration, Utc};
use conductor_domain::{PrimaryRole, SetupRequest, UsageCoverageState};
use conductor_storage::Db;
use sqlx::Executor;
use support::{connect_test_db, seed_active_user, PLACEHOLDER_PASSWORD_HASH};
use uuid::Uuid;

/// A db with an instance, returning the project id every installation and
/// coverage query is scoped to.
async fn setup(name: &str) -> (Db, Uuid) {
    let db = connect_test_db().await;
    let (project, _admin) = db
        .instance()
        .complete_setup(
            &SetupRequest {
                project_name: name.into(),
                display_name: Some(name.into()),
                bind_host: "127.0.0.1".into(),
                bind_port: 4700,
                public_url: None,
                admin_email: "admin@example.test".into(),
                admin_display_name: "Admin".into(),
                admin_password: "unused".into(),
                sso: None,
            },
            PLACEHOLDER_PASSWORD_HASH,
            "jwt-test-secret",
            None,
        )
        .await
        .expect("complete setup");
    (db, project.id)
}

/// An installation whose `connected_at` we control, inserted directly because
/// `register` always stamps "now" and these tests are about days past.
async fn seed_installation(db: &Db, project_id: Uuid, connected_at: DateTime<Utc>) -> Uuid {
    let owner = seed_active_user(db, PrimaryRole::User).await;
    let id = Uuid::new_v4();
    let at = connected_at.to_rfc3339();
    let sql = format!(
        "INSERT INTO client_installations (
             id, instance_id, user_id, installation_key, display_name, platform,
             evoflux_version, connected_at, last_seen_at,
             created_at, updated_at
         ) VALUES ('{id}', '{project_id}', '{user}', '{key}', 'test', 'linux',
                   '1.0.0', '{at}', '{at}', '{at}', '{at}')",
        user = owner.id,
        key = Uuid::new_v4(),
    );
    db.pool().execute(sql.as_str()).await.expect("seed");
    id
}

fn days_ago(days: i64) -> DateTime<Utc> {
    Utc::now() - Duration::days(days)
}

/// The whole point. A machine that was running and did no AI work all week is
/// a real zero; the totals are complete. Reading its silence as missing data
/// would make an idle member look like a broken client.
#[tokio::test]
async fn an_installation_that_only_heartbeats_is_fully_covered() {
    let (db, project) = setup("coverage").await;
    let installation = seed_installation(&db, project, days_ago(5)).await;
    let contacts = db.installation_contacts();

    for day in 1..=3 {
        contacts
            .record(installation, project, days_ago(day), 0, 1)
            .await
            .expect("heartbeat");
    }

    let coverage = contacts
        .coverage(project, days_ago(3), Utc::now(), Utc::now())
        .await
        .expect("coverage");

    assert_eq!(coverage.state, UsageCoverageState::Complete);
    assert_eq!(coverage.expected_installation_days, 3);
    assert_eq!(coverage.covered_installation_days, 3);
    assert_eq!(coverage.missing_installation_days, 0);
    assert_eq!(coverage.silent_installations, 0);
    assert_eq!(coverage.covered_bps, 10_000);
}

/// The other half of the distinction: identical zero usage, but the client
/// never checked in on one of the days.
#[tokio::test]
async fn a_day_with_no_contact_is_missing_not_zero() {
    let (db, project) = setup("coverage").await;
    let installation = seed_installation(&db, project, days_ago(5)).await;
    let contacts = db.installation_contacts();

    // Day 2 is skipped.
    for day in [3, 1] {
        contacts
            .record(installation, project, days_ago(day), 0, 1)
            .await
            .expect("heartbeat");
    }

    let coverage = contacts
        .coverage(project, days_ago(3), Utc::now(), Utc::now())
        .await
        .expect("coverage");

    assert_eq!(coverage.state, UsageCoverageState::Incomplete);
    assert_eq!(coverage.expected_installation_days, 3);
    assert_eq!(coverage.covered_installation_days, 2);
    assert_eq!(coverage.missing_installation_days, 1);
    assert_eq!(
        coverage.silent_installations, 0,
        "it reported on other days, so it is not a silent installation"
    );
    assert!(coverage.covered_bps < 10_000);
}

/// An installation that never checked in at all is the loudest form of
/// missing data and gets its own counter, not just a share of the deficit.
#[tokio::test]
async fn an_installation_that_never_reported_is_counted_as_silent() {
    let (db, project) = setup("coverage").await;
    let reporting = seed_installation(&db, project, days_ago(5)).await;
    seed_installation(&db, project, days_ago(5)).await;
    let contacts = db.installation_contacts();

    for day in 1..=3 {
        contacts
            .record(reporting, project, days_ago(day), 0, 1)
            .await
            .expect("heartbeat");
    }

    let coverage = contacts
        .coverage(project, days_ago(3), Utc::now(), Utc::now())
        .await
        .expect("coverage");

    assert_eq!(coverage.expected_installations, 2);
    assert_eq!(coverage.expected_installation_days, 6);
    assert_eq!(coverage.covered_installation_days, 3);
    assert_eq!(coverage.silent_installations, 1);
    assert_eq!(coverage.state, UsageCoverageState::Incomplete);
    assert_eq!(coverage.covered_bps, 5_000);
}

/// Today is still happening. Faulting a client for not having reported a day
/// that has not finished would leave every live project permanently
/// incomplete, which teaches operators to ignore the indicator.
#[tokio::test]
async fn the_current_day_is_not_judged() {
    let (db, project) = setup("coverage").await;
    let installation = seed_installation(&db, project, days_ago(5)).await;
    let contacts = db.installation_contacts();

    contacts
        .record(installation, project, days_ago(1), 0, 1)
        .await
        .expect("heartbeat");

    let coverage = contacts
        .coverage(project, days_ago(1), Utc::now(), Utc::now())
        .await
        .expect("coverage");

    assert_eq!(
        coverage.expected_installation_days, 1,
        "yesterday only -- today is not yet owed"
    );
    assert_eq!(coverage.state, UsageCoverageState::Complete);
    assert_eq!(
        coverage.through_day,
        Some((Utc::now() - Duration::days(1)).date_naive().to_string())
    );
}

/// A window containing no closed day cannot be complete or incomplete. Saying
/// "complete" would be a claim nothing supports.
#[tokio::test]
async fn a_window_with_no_closed_day_is_unknown() {
    let (db, project) = setup("coverage").await;
    seed_installation(&db, project, days_ago(5)).await;

    let coverage = db
        .installation_contacts()
        .coverage(project, Utc::now(), Utc::now(), Utc::now())
        .await
        .expect("coverage");

    assert_eq!(coverage.state, UsageCoverageState::Unknown);
    assert_eq!(coverage.expected_installation_days, 0);
    assert_eq!(coverage.through_day, None);
}

/// Onboarding a machine today must not put last month in deficit: it could
/// not have reported days it did not exist for.
#[tokio::test]
async fn an_installation_is_only_expected_from_the_day_it_registered() {
    let (db, project) = setup("coverage").await;
    let old = seed_installation(&db, project, days_ago(10)).await;
    let recent = seed_installation(&db, project, days_ago(2)).await;
    let contacts = db.installation_contacts();

    for day in 1..=3 {
        contacts
            .record(old, project, days_ago(day), 0, 1)
            .await
            .expect("heartbeat");
    }
    for day in 1..=2 {
        contacts
            .record(recent, project, days_ago(day), 0, 1)
            .await
            .expect("heartbeat");
    }

    let coverage = contacts
        .coverage(project, days_ago(3), Utc::now(), Utc::now())
        .await
        .expect("coverage");

    // 3 days owed by the old installation, 2 by the one registered two days
    // ago -- not 3, which would fault it for a day before it existed.
    assert_eq!(coverage.expected_installation_days, 5);
    assert_eq!(coverage.covered_installation_days, 5);
    assert_eq!(coverage.state, UsageCoverageState::Complete);
}

/// Coverage is scoped by project on both sides of the ratio. One project's
/// installations must never be owed by, or credited to, another.
#[tokio::test]
async fn coverage_is_scoped_to_one_project() {
    let (db, mine) = setup("coverage").await;
    let installation = seed_installation(&db, mine, days_ago(5)).await;
    let contacts = db.installation_contacts();

    contacts
        .record(installation, mine, days_ago(1), 0, 1)
        .await
        .expect("heartbeat");

    let coverage = contacts
        .coverage(mine, days_ago(1), Utc::now(), Utc::now())
        .await
        .expect("coverage");
    assert_eq!(coverage.state, UsageCoverageState::Complete);
    assert_eq!(coverage.expected_installations, 1);

    let other = contacts
        .coverage(Uuid::new_v4(), days_ago(1), Utc::now(), Utc::now())
        .await
        .expect("coverage");
    assert_eq!(other.expected_installations, 0);
    assert_eq!(other.covered_installation_days, 0);
    assert_eq!(
        other.state,
        UsageCoverageState::Unknown,
        "a project with no installations has nothing to be complete about"
    );
}

/// Several contacts in one day are one covered day, and the counters
/// accumulate rather than overwrite -- a replayed batch must not be able to
/// rewrite when the day started.
#[tokio::test]
async fn repeated_contact_in_a_day_accumulates_into_one_row() {
    let (db, project) = setup("coverage").await;
    let installation = seed_installation(&db, project, days_ago(5)).await;
    let contacts = db.installation_contacts();

    let morning = days_ago(1)
        .date_naive()
        .and_hms_opt(1, 0, 0)
        .unwrap()
        .and_utc();
    let evening = days_ago(1)
        .date_naive()
        .and_hms_opt(23, 0, 0)
        .unwrap()
        .and_utc();
    contacts
        .record(installation, project, morning, 4, 1)
        .await
        .expect("first");
    contacts
        .record(installation, project, evening, 6, 1)
        .await
        .expect("second");

    let row = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT first_contact_at, last_contact_at, telemetry_events, heartbeats
         FROM installation_contact_days WHERE installation_id = ?",
    )
    .bind(installation.to_string())
    .fetch_one(db.pool())
    .await
    .expect("one row per day");

    assert_eq!(
        row.0,
        morning.to_rfc3339(),
        "the day's start is not rewritten"
    );
    assert_eq!(row.1, evening.to_rfc3339());
    assert_eq!(row.2, 10);
    assert_eq!(row.3, 2);

    let coverage = contacts
        .coverage(project, days_ago(1), Utc::now(), Utc::now())
        .await
        .expect("coverage");
    assert_eq!(coverage.covered_installation_days, 1);
}
