//! A limit is only useful if the number it compares against is the same one
//! the member is shown. These tests pin that it uses the authoritative cost
//! expression, respects its own period window, and scopes correctly.

mod support;

use chrono::{Duration, Utc};
use conductor_domain::{LimitPeriod, LimitScope, LimitState, PrimaryRole};
use conductor_storage::repos::UpsertSpendLimit;
use conductor_storage::Db;
use support::{connect_test_db, seed_active_user};
use uuid::Uuid;

async fn seed_project(db: &Db) -> Uuid {
    let id = Uuid::new_v4();
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        INSERT INTO instance (
            id, project_name, bind_host, bind_port, collection_level,
            setup_completed, jwt_secret, created_at, updated_at
        ) VALUES (?, 'Limits test', '127.0.0.1', 4700, 'L1', 1, 'unused', ?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(&now)
    .bind(&now)
    .execute(db.pool())
    .await
    .expect("seed project");
    id
}

/// Insert a model call with an explicit cost split between Conductor's own
/// figure and the client's, plus an explicit `received_at`.
#[allow(clippy::too_many_arguments)]
async fn seed_spend(
    db: &Db,
    project_id: Uuid,
    user_id: Uuid,
    role: PrimaryRole,
    server_cost: Option<i64>,
    client_cost: Option<i64>,
    received_at: chrono::DateTime<Utc>,
) {
    sqlx::query(
        r#"
        INSERT INTO telemetry_events (
            id, project_id, user_id, request_id, event_type, sequence,
            provider, model, tokens_in, tokens_out, cache_read_tokens,
            cache_write_tokens, reasoning_tokens, tool_use_tokens, duration_ms,
            status, estimated_cost_usd_micros, server_cost_usd_micros,
            primary_role_snapshot, reported_at, received_at
        ) VALUES (?, ?, ?, 'req', 'model_call', 1, 'anthropic', 'claude-sonnet-4',
                  1000, 100, 0, 0, 0, 0, 10, 'success', ?, ?, ?, ?, ?)
        "#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(project_id.to_string())
    .bind(user_id.to_string())
    .bind(client_cost)
    .bind(server_cost)
    .bind(role.as_str())
    .bind(received_at.to_rfc3339())
    .bind(received_at.to_rfc3339())
    .execute(db.pool())
    .await
    .expect("seed spend");
}

fn member_limit(subject: Uuid, limit: u64, warn: u32) -> UpsertSpendLimit {
    UpsertSpendLimit {
        scope: LimitScope::Member,
        subject_id: subject.to_string(),
        period: LimitPeriod::Month,
        limit_usd_micros: limit,
        warn_percent: warn,
        enabled: true,
    }
}

#[tokio::test]
async fn a_member_limit_sums_only_that_members_spend() {
    let db = connect_test_db().await;
    let project_id = seed_project(&db).await;
    let watched = seed_active_user(&db, PrimaryRole::User).await;
    let other = seed_active_user(&db, PrimaryRole::User).await;

    seed_spend(
        &db,
        project_id,
        watched.id,
        PrimaryRole::User,
        Some(30_000_000),
        None,
        Utc::now(),
    )
    .await;
    seed_spend(
        &db,
        project_id,
        other.id,
        PrimaryRole::User,
        Some(90_000_000),
        None,
        Utc::now(),
    )
    .await;

    db.spend_limits()
        .upsert(project_id, &member_limit(watched.id, 100_000_000, 80))
        .await
        .expect("upsert");

    let statuses = db
        .spend_limits()
        .statuses(project_id, Utc::now())
        .await
        .expect("statuses");
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].evaluation.spent_usd_micros, 30_000_000);
    assert_eq!(statuses[0].evaluation.state, LimitState::Within);
}

/// The limit reads the same authoritative figure the dashboard shows, and
/// there is only one: Conductor's own price. A row it never priced carries no
/// cost at all now that clients no longer report one, so an allowance counts
/// what was priced and nothing else.
#[tokio::test]
async fn spend_counts_only_what_conductor_priced() {
    let db = connect_test_db().await;
    let project_id = seed_project(&db).await;
    let member = seed_active_user(&db, PrimaryRole::User).await;

    seed_spend(
        &db,
        project_id,
        member.id,
        PrimaryRole::User,
        Some(60_000_000),
        Some(1),
        Utc::now(),
    )
    .await;
    // A row Conductor never priced. It used to be charged at the client's
    // figure; there is no such figure any more.
    seed_spend(
        &db,
        project_id,
        member.id,
        PrimaryRole::User,
        None,
        Some(25_000_000),
        Utc::now(),
    )
    .await;

    db.spend_limits()
        .upsert(project_id, &member_limit(member.id, 100_000_000, 80))
        .await
        .expect("upsert");
    let statuses = db
        .spend_limits()
        .statuses(project_id, Utc::now())
        .await
        .expect("statuses");
    assert_eq!(
        statuses[0].evaluation.spent_usd_micros, 60_000_000,
        "only the priced row counts towards the allowance"
    );
    assert_eq!(statuses[0].evaluation.state, LimitState::Within);
}

/// Unpriced volume contributes nothing, and that is a real blind spot rather
/// than a bug: a limit cannot charge for a cost nobody computed. Pinned so
/// the behaviour is a decision rather than a surprise.
#[tokio::test]
async fn unpriced_spend_does_not_count_towards_a_limit() {
    let db = connect_test_db().await;
    let project_id = seed_project(&db).await;
    let member = seed_active_user(&db, PrimaryRole::User).await;

    seed_spend(
        &db,
        project_id,
        member.id,
        PrimaryRole::User,
        None,
        None,
        Utc::now(),
    )
    .await;

    db.spend_limits()
        .upsert(project_id, &member_limit(member.id, 100_000_000, 80))
        .await
        .expect("upsert");
    let statuses = db
        .spend_limits()
        .statuses(project_id, Utc::now())
        .await
        .expect("statuses");
    assert_eq!(statuses[0].evaluation.spent_usd_micros, 0);
    assert_eq!(statuses[0].evaluation.state, LimitState::Within);
}

/// Spend from a closed period must not count against the current one.
#[tokio::test]
async fn a_daily_limit_ignores_spend_from_a_previous_day() {
    let db = connect_test_db().await;
    let project_id = seed_project(&db).await;
    let member = seed_active_user(&db, PrimaryRole::User).await;

    seed_spend(
        &db,
        project_id,
        member.id,
        PrimaryRole::User,
        Some(95_000_000),
        None,
        Utc::now() - Duration::days(2),
    )
    .await;
    seed_spend(
        &db,
        project_id,
        member.id,
        PrimaryRole::User,
        Some(10_000_000),
        None,
        Utc::now(),
    )
    .await;

    db.spend_limits()
        .upsert(
            project_id,
            &UpsertSpendLimit {
                period: LimitPeriod::Day,
                ..member_limit(member.id, 100_000_000, 80)
            },
        )
        .await
        .expect("upsert");
    let statuses = db
        .spend_limits()
        .statuses(project_id, Utc::now())
        .await
        .expect("statuses");
    assert_eq!(statuses[0].evaluation.spent_usd_micros, 10_000_000);
    assert_eq!(statuses[0].evaluation.state, LimitState::Within);
}

#[tokio::test]
async fn a_project_limit_sums_every_member_and_a_role_limit_only_that_role() {
    let db = connect_test_db().await;
    let project_id = seed_project(&db).await;
    let user = seed_active_user(&db, PrimaryRole::User).await;
    let admin = seed_active_user(&db, PrimaryRole::Admin).await;

    seed_spend(
        &db,
        project_id,
        user.id,
        PrimaryRole::User,
        Some(20_000_000),
        None,
        Utc::now(),
    )
    .await;
    seed_spend(
        &db,
        project_id,
        admin.id,
        PrimaryRole::Admin,
        Some(50_000_000),
        None,
        Utc::now(),
    )
    .await;

    let limits = db.spend_limits();
    limits
        .upsert(
            project_id,
            &UpsertSpendLimit {
                scope: LimitScope::Project,
                subject_id: String::new(),
                period: LimitPeriod::Month,
                limit_usd_micros: 100_000_000,
                warn_percent: 80,
                enabled: true,
            },
        )
        .await
        .expect("project limit");
    limits
        .upsert(
            project_id,
            &UpsertSpendLimit {
                scope: LimitScope::Role,
                subject_id: PrimaryRole::Admin.as_str().to_string(),
                period: LimitPeriod::Month,
                limit_usd_micros: 40_000_000,
                warn_percent: 80,
                enabled: true,
            },
        )
        .await
        .expect("role limit");

    let statuses = limits
        .statuses(project_id, Utc::now())
        .await
        .expect("statuses");
    let project = statuses
        .iter()
        .find(|status| status.limit.scope == LimitScope::Project)
        .expect("project status");
    assert_eq!(project.evaluation.spent_usd_micros, 70_000_000);

    let role = statuses
        .iter()
        .find(|status| status.limit.scope == LimitScope::Role)
        .expect("role status");
    assert_eq!(role.evaluation.spent_usd_micros, 50_000_000);
    assert_eq!(
        role.evaluation.state,
        LimitState::Exceeded,
        "$50 against a $40 allowance"
    );
}

/// One allowance per (scope, subject, period): a second write replaces it,
/// because two limits over the same window could not both be authoritative.
#[tokio::test]
async fn upserting_the_same_scope_and_period_replaces_the_allowance() {
    let db = connect_test_db().await;
    let project_id = seed_project(&db).await;
    let member = seed_active_user(&db, PrimaryRole::User).await;
    let limits = db.spend_limits();

    limits
        .upsert(project_id, &member_limit(member.id, 100_000_000, 80))
        .await
        .expect("first");
    limits
        .upsert(project_id, &member_limit(member.id, 250_000_000, 50))
        .await
        .expect("second");

    let all = limits.list(project_id).await.expect("list");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].limit_usd_micros, 250_000_000);
    assert_eq!(all[0].warn_percent, 50);
}

#[tokio::test]
async fn a_disabled_limit_is_not_evaluated() {
    let db = connect_test_db().await;
    let project_id = seed_project(&db).await;
    let member = seed_active_user(&db, PrimaryRole::User).await;

    db.spend_limits()
        .upsert(
            project_id,
            &UpsertSpendLimit {
                enabled: false,
                ..member_limit(member.id, 1, 80)
            },
        )
        .await
        .expect("upsert");
    assert!(db
        .spend_limits()
        .statuses(project_id, Utc::now())
        .await
        .expect("statuses")
        .is_empty());
}

#[tokio::test]
async fn deleting_a_limit_removes_it() {
    let db = connect_test_db().await;
    let project_id = seed_project(&db).await;
    let member = seed_active_user(&db, PrimaryRole::User).await;
    let limits = db.spend_limits();

    limits
        .upsert(project_id, &member_limit(member.id, 100, 80))
        .await
        .expect("upsert");
    assert!(limits
        .delete(
            project_id,
            LimitScope::Member,
            &member.id.to_string(),
            LimitPeriod::Month
        )
        .await
        .expect("delete"));
    assert!(limits.list(project_id).await.expect("list").is_empty());
    assert!(
        !limits
            .delete(
                project_id,
                LimitScope::Member,
                &member.id.to_string(),
                LimitPeriod::Month
            )
            .await
            .expect("delete again"),
        "deleting what is gone reports no change"
    );
}

/// Money is micro-USD, so ordinary budgets do not fit in 32 bits.
///
/// A $2,500 monthly allowance is 2.5 billion micros. The columns were declared
/// `INTEGER`, which SQLite widens on its own but Postgres and MySQL read as
/// 32-bit — every allowance above roughly $2,147 was rejected there while
/// passing here. This pins the amounts that used to overflow, on whichever
/// engine the suite runs against.
#[tokio::test]
async fn an_allowance_larger_than_a_32_bit_column_round_trips() {
    let db = connect_test_db().await;
    let project_id = seed_project(&db).await;
    let member = seed_active_user(&db, PrimaryRole::User).await;
    let limits = db.spend_limits();

    // $2,500/month, and a project-wide figure far past it.
    for allowance in [2_500_000_000u64, 900_000_000_000u64] {
        let stored = limits
            .upsert(project_id, &member_limit(member.id, allowance, 80))
            .await
            .expect("upsert a realistic budget");
        assert_eq!(stored.limit_usd_micros, allowance);
        assert_eq!(
            limits
                .find(
                    project_id,
                    LimitScope::Member,
                    &member.id.to_string(),
                    LimitPeriod::Month
                )
                .await
                .expect("find")
                .expect("stored limit")
                .limit_usd_micros,
            allowance,
            "read back unchanged, not truncated to 32 bits"
        );
    }

    // The spend side has to survive the same magnitudes.
    seed_spend(
        &db,
        project_id,
        member.id,
        PrimaryRole::User,
        Some(3_000_000_000),
        None,
        Utc::now(),
    )
    .await;
    let statuses = limits
        .statuses(project_id, Utc::now())
        .await
        .expect("statuses");
    assert_eq!(statuses[0].evaluation.spent_usd_micros, 3_000_000_000);
}
