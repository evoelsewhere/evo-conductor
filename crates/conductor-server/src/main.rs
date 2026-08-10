use conductor_server::core::constants::server::{ENV_HOST, ENV_PORT};
use conductor_server::http::realtime::{RealtimeHub, RealtimeSignal};
use conductor_server::{build_router, AppState, Config};
use std::net::SocketAddr;
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "evo_conductor=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env();
    let state = AppState::new(&config.database_url, config.realtime.clone()).await?;
    let realtime = state.realtime.clone();
    let app = build_router(state.clone(), &config);

    let addr = bind_addr(&config, &state).await?;
    tracing::info!("evo-conductor listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(realtime))
        .await?;
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
