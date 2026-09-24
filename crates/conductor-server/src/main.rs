use conductor_server::cli;
use conductor_server::core::constants::server::{ENV_HOST, ENV_PORT};
use conductor_server::core::model_pricing;
use conductor_server::http::realtime::{RealtimeHub, RealtimeSignal};
use conductor_server::{build_router, AppState, Config};
use std::net::SocketAddr;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Any argument means an operator command rather than a server run: it
    // does its work and exits without ever binding the listener. Usage is
    // answered before the database is touched, so it still works on a host
    // that cannot reach one.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if cli::wants_usage(&args) {
        print!("{}", cli::USAGE);
        return Ok(());
    }

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "evo_conductor=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let state = AppState::new(&config.database_url, config.realtime.clone()).await?;

    if !args.is_empty() {
        return cli::run(&args, &state).await;
    }
    let realtime = state.realtime.clone();
    spawn_model_pricing_sync(state.clone(), &config);
    spawn_jira_report_sync(state.clone());
    spawn_jira_task_sync(state.clone());
    let app = build_router(state.clone(), &config);

    let addr = bind_addr(&config, &state).await?;
    tracing::info!("evo-conductor listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(realtime))
        .await?;
    Ok(())
}

/// Keep the price catalog current in the background.
///
/// Detached on purpose: an unreachable models.dev must not delay or fail
/// startup. A failed sync leaves the previous rate rows in place, so pricing
/// degrades to "slightly stale" rather than to "unpriced".
fn spawn_model_pricing_sync(state: AppState, config: &Config) {
    if !config.model_pricing.enabled {
        tracing::info!("model pricing sync disabled");
        return;
    }
    let url = config.model_pricing.url.clone();
    let period = Duration::from_secs(config.model_pricing.refresh_hours * 3600);
    tokio::spawn(async move {
        loop {
            match model_pricing::sync_from_models_dev(&state.db, &url).await {
                Ok(outcome) if outcome.unchanged => {
                    tracing::debug!(version = %outcome.version, "price catalog unchanged");
                }
                Ok(outcome) => {
                    tracing::info!(
                        version = %outcome.version,
                        models = outcome.model_count,
                        priced = outcome.priced_model_count,
                        changed = outcome.changed_model_count,
                        "price catalog synced"
                    );
                    // Ingest prices from the in-memory table, so it has to be
                    // rebuilt or the new rates would not reach new events.
                    match state.model_rates.reload(&state.db).await {
                        Ok(loaded) => tracing::info!(models = loaded, "model rates reloaded"),
                        Err(error) => {
                            tracing::warn!(%error, "could not reload model rates after sync")
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "price catalog sync failed; keeping existing rates");
                }
            }
            tokio::time::sleep(period).await;
        }
    });
}

/// Post an automatic Jira usage-report comment on a cadence the admin sets
/// from the Settings UI (`JiraSettings.report_interval_hours`), not an
/// environment variable — unlike `spawn_model_pricing_sync`, there is no
/// startup config gate here at all: whether this does anything is entirely
/// database-driven (`jira.enabled`), re-read every tick, so toggling it in
/// the UI takes effect on the next cycle without a restart.
fn spawn_jira_report_sync(state: AppState) {
    tokio::spawn(async move {
        loop {
            let sleep_hours = match run_jira_report_tick(&state).await {
                Ok(Some(interval_hours)) => interval_hours,
                Ok(None) => 1,
                Err(error) => {
                    tracing::warn!(%error, "jira usage report failed; will retry next cycle");
                    1
                }
            };
            tokio::time::sleep(Duration::from_secs(u64::from(sleep_hours) * 3600)).await;
        }
    });
}

/// One check-and-maybe-post cycle. Returns the interval to sleep for next —
/// the configured cadence once Jira reporting is enabled, or a one-hour
/// recheck while it's off/unconfigured, so enabling it doesn't wait for a
/// stale, much-longer interval to elapse first.
async fn run_jira_report_tick(state: &AppState) -> anyhow::Result<Option<u32>> {
    let jira = state.db.instance().jira_settings().await?;
    if !jira.enabled || jira.report_issue_key.trim().is_empty() {
        return Ok(None);
    }
    let to = chrono::Utc::now();
    let from = to - chrono::Duration::hours(i64::from(jira.report_interval_hours));
    let period_start = from.format("%Y-%m-%d").to_string();
    // T4.4's idempotency guard: a restart that resets this loop's sleep
    // timer must not re-post a period a manual "post now" (or an earlier
    // cycle) already covered.
    if jira.last_reported_period.as_deref() == Some(period_start.as_str()) {
        tracing::debug!(period = %period_start, "jira usage report already posted for this period");
        return Ok(Some(jira.report_interval_hours));
    }
    let instance = state
        .db
        .instance()
        .get()
        .await?
        .ok_or_else(|| anyhow::anyhow!("Conductor setup is not complete"))?;
    let project_id = state
        .db
        .instance()
        .authorization_project_id()
        .await?
        .ok_or_else(|| anyhow::anyhow!("Conductor setup is not complete"))?;
    let posted_period = conductor_server::core::jira::post_usage_report(
        state,
        project_id,
        &instance.project_name,
        from,
        to,
    )
    .await?;
    let mut updated = jira;
    let interval_hours = updated.report_interval_hours;
    updated.last_reported_period = Some(posted_period);
    state.db.instance().update_jira_settings(&updated).await?;
    tracing::info!(period = %period_start, "jira usage report posted");
    Ok(Some(interval_hours))
}

/// Keeps the local `jira_tasks` mirror current so the task report page and
/// the task-picker suggestion lookup never call Jira synchronously. Runs on
/// a much shorter cadence than `spawn_jira_report_sync` -- that one posts a
/// comment on a deliberately slow rhythm, this one feeds an interactive
/// page and should reflect a newly created or renamed task within minutes.
const JIRA_TASK_SYNC_INTERVAL_SECS: u64 = 900;

fn spawn_jira_task_sync(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(error) = run_jira_task_sync_tick(&state).await {
                tracing::warn!(%error, "jira task sync failed; will retry next cycle");
            }
            tokio::time::sleep(Duration::from_secs(JIRA_TASK_SYNC_INTERVAL_SECS)).await;
        }
    });
}

async fn run_jira_task_sync_tick(state: &AppState) -> anyhow::Result<()> {
    let jira = state.db.instance().jira_settings().await?;
    if !jira.enabled || jira.default_project_key.trim().is_empty() {
        return Ok(());
    }
    let project_id = state
        .db
        .instance()
        .authorization_project_id()
        .await?
        .ok_or_else(|| anyhow::anyhow!("Conductor setup is not complete"))?;
    let config = conductor_server::core::jira::resolve_jira_config(state).await?;
    let tasks = conductor_server::core::jira::list_project_tasks(
        &config,
        &jira.default_project_key,
        &jira.task_type_rules,
        &jira.project_prefix_rules,
    )
    .await?;
    let count = tasks.len();
    state.db.jira_tasks().upsert_many(project_id, &tasks).await?;
    tracing::info!(count, "jira task sync completed");

    let activated_issue_keys = state
        .db
        .task_activations()
        .distinct_issue_keys(project_id)
        .await?;
    for issue_key in activated_issue_keys {
        match conductor_server::core::jira::fetch_status_history(&config, &issue_key).await {
            Ok(changes) => {
                state
                    .db
                    .jira_status_history()
                    .replace_for_issue(project_id, &issue_key, &changes)
                    .await?;
            }
            Err(error) => {
                tracing::warn!(%error, issue_key, "jira status history sync failed for issue; will retry next cycle");
            }
        }
    }
    Ok(())
}

/// Environment variables win; otherwise fall back to the bind address saved
/// in the network settings (edited via the UI, applied on restart).
async fn bind_addr(config: &Config, state: &AppState) -> anyhow::Result<SocketAddr> {
    let env_host = std::env::var(ENV_HOST).ok();
    let env_port = std::env::var(ENV_PORT).ok();
    if env_host.is_none() || env_port.is_none() {
        if let Some(instance) = state.db.instance().get().await? {
            let host = env_host.unwrap_or(instance.bind_host);
            let port = match env_port {
                Some(value) => value.parse().unwrap_or(instance.bind_port),
                None => instance.bind_port,
            };
            return Ok(format!("{host}:{port}").parse()?);
        }
    }
    Ok(config.bind_addr()?)
}

async fn shutdown_signal(realtime: RealtimeHub) {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "failed to install Ctrl+C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "failed to install terminate handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }

    tracing::info!(
        active_connections = realtime.active_connections(),
        "draining realtime connections"
    );
    realtime.publish(RealtimeSignal::ServerDrain {
        retry_after_ms: 2_000,
    });
    tokio::time::sleep(Duration::from_millis(250)).await;
}
