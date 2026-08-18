mod support;

use std::time::Duration;

use axum::http::{header, StatusCode};
use conductor_auth::hash_token;
use conductor_domain::{PrimaryRole, SecretScope};
use conductor_server::RealtimeConfig;
use support::{test_app_with_realtime_config, TestApp};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

const NETWORK_SMOKE_STREAMS: usize = 4;

struct NetworkStream {
    response: reqwest::Response,
    pending: Vec<u8>,
}

struct RunningServer {
    base_url: String,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<std::io::Result<()>>>,
}

impl RunningServer {
    async fn start(app: &TestApp) -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("bind realtime acceptance listener");
        let address = listener.local_addr().expect("load-test listener address");
        let router = app.router.clone();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
        });
        Self {
            base_url: format!("http://{address}"),
            shutdown: Some(shutdown_tx),
            task: Some(task),
        }
    }

    async fn stop(mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let task = self.task.take().expect("load-test server task");
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .expect("load-test server shutdown timeout")
            .expect("join load-test server")
            .expect("serve load-test router");
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[tokio::test]
async fn authenticated_sse_streams_survive_a_loopback_heartbeat() {
    let app = test_app_with_realtime_config(load_config(NETWORK_SMOKE_STREAMS, 4)).await;
    app.seed_project_identity().await;
    let owner = app.seed_user(PrimaryRole::User).await;
    let token = "evc_realtime_loopback_smoke";
    app.state
        .db
        .secrets()
        .insert(
            owner.id,
            "Realtime loopback smoke",
            "evc_smoke",
            &hash_token(token),
            &[SecretScope::SubscribeResources],
            None,
        )
        .await
        .expect("seed smoke-test connection secret");

    let server = RunningServer::start(&app).await;
    let events_url = format!("{}/api/v1/realtime/events", server.base_url);
    let client = reqwest::Client::builder()
        .http1_only()
        .pool_max_idle_per_host(0)
        .tcp_nodelay(true)
        .build()
        .expect("build realtime load client");

    let mut streams = Vec::with_capacity(NETWORK_SMOKE_STREAMS);
    for _ in 0..NETWORK_SMOKE_STREAMS {
        streams.push(open_network_stream(&client, &events_url, token).await);
    }
    wait_for_connection_count(&app, NETWORK_SMOKE_STREAMS).await;

    for stream in &mut streams {
        assert_next_event(stream, "control.heartbeat").await;
    }

    drop(streams);
    wait_for_connection_count(&app, 0).await;
    server.stop().await;
}

async fn open_network_stream(
    client: &reqwest::Client,
    events_url: &str,
    token: &str,
) -> NetworkStream {
    let response = client
        .get(events_url)
        .bearer_auth(token)
        .header(header::ACCEPT.as_str(), "text/event-stream")
        .send()
        .await
        .expect("open realtime TCP connection");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/event-stream")));
    let mut stream = NetworkStream {
        response,
        pending: Vec::new(),
    };
    assert_next_event(&mut stream, "control.hello").await;
    assert_next_event(&mut stream, "resources.head").await;
    stream
}

async fn assert_next_event(stream: &mut NetworkStream, expected: &str) {
    let event = next_sse_event(stream).await;
    assert!(
        event.contains(&format!("event: {expected}")),
        "expected {expected}, received {event}"
    );
}

async fn next_sse_event(stream: &mut NetworkStream) -> String {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(end) = find_complete_sse_event(&stream.pending) {
                let event = stream.pending.drain(..end).collect::<Vec<_>>();
                return String::from_utf8(event).expect("UTF-8 SSE event");
            }
            let chunk = stream
                .response
                .chunk()
                .await
                .expect("read network SSE chunk")
                .expect("network SSE stream ended");
            stream.pending.extend_from_slice(&chunk);
            assert!(
                stream.pending.len() <= 1_048_576,
                "SSE event exceeded one MiB"
            );
        }
    })
    .await
    .expect("network SSE event timeout")
}

fn find_complete_sse_event(buffer: &[u8]) -> Option<usize> {
    buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|position| position + 2)
}

async fn wait_for_connection_count(app: &TestApp, expected: usize) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if app.state.realtime.active_connections() == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "realtime connection count did not reach {expected}; actual={}",
            app.state.realtime.active_connections()
        )
    });
}

fn load_config(max_connections: usize, max_concurrent_handshakes: usize) -> RealtimeConfig {
    RealtimeConfig {
        max_connections,
        max_connections_per_secret: NETWORK_SMOKE_STREAMS,
        max_concurrent_handshakes,
        broadcast_capacity: 512,
        heartbeat_seconds: 1,
    }
}
