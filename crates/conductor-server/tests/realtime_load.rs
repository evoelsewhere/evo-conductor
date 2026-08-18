mod support;

use std::mem::MaybeUninit;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::{header, StatusCode};
use conductor_auth::hash_token;
use conductor_domain::{PrimaryRole, SecretScope};
use conductor_server::RealtimeConfig;
use serde_json::Value;
use support::{test_app_with_realtime_config, TestApp};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Barrier, Semaphore};
use tokio::task::{JoinHandle, JoinSet};

const TARGET_CONNECTIONS: usize = 2_000;
const CONNECTIONS_PER_SECRET: usize = 4;
const LOAD_SECRET_COUNT: usize = TARGET_CONNECTIONS / CONNECTIONS_PER_SECRET;
const MAX_IN_FLIGHT_OPENS: usize = 128;
const HEARTBEAT_CYCLES: usize = 2;
const OPEN_TIMEOUT: Duration = Duration::from_secs(120);
const HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(60);
const CONNECTION_COUNT_TIMEOUT: Duration = Duration::from_secs(10);
const FD_HEADROOM: usize = 512;

struct NetworkStream {
    token_index: usize,
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
    let app = test_app_with_realtime_config(load_config(CONNECTIONS_PER_SECRET, 4)).await;
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

    let mut streams = Vec::with_capacity(CONNECTIONS_PER_SECRET);
    for _ in 0..CONNECTIONS_PER_SECRET {
        streams.push(open_network_stream_now(&client, &events_url, 0, token).await);
    }
    wait_for_connection_count(&app, CONNECTIONS_PER_SECRET).await;
    for stream in &mut streams {
        assert_next_network_event(stream, "control.heartbeat").await;
    }

    drop(streams);
    wait_for_connection_count(&app, 0).await;
    server.stop().await;
}

/// Resource-intensive network acceptance proof. It intentionally runs only
/// when asked so the ordinary server suite stays fast and deterministic:
///
/// cargo test -p conductor-server --test realtime_load \
///   -- --ignored --exact accepts_and_holds_two_thousand_authenticated_sse_streams --nocapture
///
/// All 2,000 client tasks start behind one barrier. At most 128 perform a TCP +
/// authenticated SSE handshake at once, staying below the application's
/// default 256-handshake admission budget and common host listen backlogs. The
/// accepted response bodies and sockets remain live together at the peak.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "network load acceptance: opens and revalidates 2,000 authenticated SSE streams"]
async fn accepts_and_holds_two_thousand_authenticated_sse_streams() {
    assert_eq!(TARGET_CONNECTIONS % CONNECTIONS_PER_SECRET, 0);
    let fd_soft_limit = assert_fd_budget();
    let app = test_app_with_realtime_config(load_config(TARGET_CONNECTIONS, 256)).await;
    app.seed_project_identity().await;
    let owner = app.seed_user(PrimaryRole::User).await;

    // Five hundred independently authenticated credentials exercise the
    // production per-secret default of four streams. One extra credential is
    // reserved for global-capacity and refill checks.
    let mut tokens = Vec::with_capacity(LOAD_SECRET_COUNT + 1);
    for token_index in 0..=LOAD_SECRET_COUNT {
        let token = format!("evc_realtime_load_{token_index:04}");
        app.state
            .db
            .secrets()
            .insert(
                owner.id,
                &format!("Realtime load {token_index}"),
                "evc_load",
                &hash_token(&token),
                &[SecretScope::SubscribeResources],
                None,
            )
            .await
            .expect("seed load-test connection secret");
        tokens.push(token);
    }

    let server = RunningServer::start(&app).await;
    let events_url = format!("{}/api/v1/realtime/events", server.base_url);
    let client = reqwest::Client::builder()
        .http1_only()
        .pool_max_idle_per_host(0)
        .tcp_nodelay(true)
        .build()
        .expect("build realtime load client");

    let opening_started_at = Instant::now();
    let mut held = open_all_streams(&client, &events_url, &tokens).await;
    let opening_elapsed = opening_started_at.elapsed();

    assert_eq!(held.len(), TARGET_CONNECTIONS);
    wait_for_connection_count(&app, TARGET_CONNECTIONS).await;
    assert_eq!(app.state.realtime.active_owners(), 1);

    // Poll every socket concurrently through two configured heartbeat cycles.
    // Each control.heartbeat is emitted only after durable secret and current-
    // owner revalidation succeeds.
    let heartbeat_started_at = Instant::now();
    let mut tasks = JoinSet::new();
    for stream in held.drain(..) {
        tasks.spawn(read_heartbeat_cycles(stream));
    }

    let mut heartbeat_events = 0usize;
    let heartbeat_result = tokio::time::timeout(HEARTBEAT_TIMEOUT, async {
        while let Some(joined) = tasks.join_next().await {
            let (stream, observed_heartbeats) = joined.expect("heartbeat stream task");
            heartbeat_events += observed_heartbeats;
            held.push(stream);
        }
    })
    .await;
    if heartbeat_result.is_err() {
        let active = app.state.realtime.active_connections();
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        panic!("2,000-stream heartbeat acceptance exceeded {HEARTBEAT_TIMEOUT:?}; active={active}");
    }
    let heartbeat_elapsed = heartbeat_started_at.elapsed();

    assert_eq!(held.len(), TARGET_CONNECTIONS);
    assert!(
        heartbeat_events >= TARGET_CONNECTIONS * HEARTBEAT_CYCLES,
        "observed only {heartbeat_events} heartbeat events"
    );
    wait_for_connection_count(&app, TARGET_CONNECTIONS).await;

    let global_rejection =
        network_realtime_request(&client, &events_url, &tokens[LOAD_SECRET_COUNT]).await;
    assert_network_capacity_response(
        global_rejection,
        StatusCode::SERVICE_UNAVAILABLE,
        "realtime connection capacity reached",
    )
    .await;
    wait_for_connection_count(&app, TARGET_CONNECTIONS).await;

    // Free one global slot without changing token zero's four held streams,
    // then prove the per-secret failure remains a 429 and does not leak the
    // briefly acquired global permit.
    let removable = held
        .iter()
        .position(|stream| stream.token_index != 0)
        .expect("stream from another credential");
    drop(held.swap_remove(removable));
    wait_for_connection_count(&app, TARGET_CONNECTIONS - 1).await;

    let per_secret_rejection = network_realtime_request(&client, &events_url, &tokens[0]).await;
    assert_network_capacity_response(
        per_secret_rejection,
        StatusCode::TOO_MANY_REQUESTS,
        "connection limit reached for this secret",
    )
    .await;
    wait_for_connection_count(&app, TARGET_CONNECTIONS - 1).await;

    held.push(
        open_network_stream_now(
            &client,
            &events_url,
            LOAD_SECRET_COUNT,
            &tokens[LOAD_SECRET_COUNT],
        )
        .await,
    );
    wait_for_connection_count(&app, TARGET_CONNECTIONS).await;

    eprintln!(
        "realtime-load peak={TARGET_CONNECTIONS} credentials={} per_secret={} concurrent_opens={} fd_soft_limit={} open_ms={} heartbeat_cycles={} heartbeat_events={} heartbeat_ms={}",
        LOAD_SECRET_COUNT,
        CONNECTIONS_PER_SECRET,
        MAX_IN_FLIGHT_OPENS,
        fd_soft_limit,
        opening_elapsed.as_millis(),
        HEARTBEAT_CYCLES,
        heartbeat_events,
        heartbeat_elapsed.as_millis(),
    );

    drop(held);
    wait_for_connection_count(&app, 0).await;
    assert_eq!(app.state.realtime.active_owners(), 0);
    drop(client);
    server.stop().await;
    eprintln!("realtime-load cleanup active=0 owners=0 server=stopped");
}

async fn open_all_streams(
    client: &reqwest::Client,
    events_url: &str,
    tokens: &[String],
) -> Vec<NetworkStream> {
    let start = Arc::new(Barrier::new(TARGET_CONNECTIONS + 1));
    let in_flight = Arc::new(Semaphore::new(MAX_IN_FLIGHT_OPENS));
    let mut tasks = JoinSet::new();

    for (token_index, token) in tokens.iter().take(LOAD_SECRET_COUNT).enumerate() {
        for _ in 0..CONNECTIONS_PER_SECRET {
            let client = client.clone();
            let events_url = events_url.to_string();
            let token = token.clone();
            let start = start.clone();
            let in_flight = in_flight.clone();
            tasks.spawn(async move {
                start.wait().await;
                let _slot = in_flight
                    .acquire_owned()
                    .await
                    .expect("load-test open semaphore");
                open_network_stream_now(&client, &events_url, token_index, &token).await
            });
        }
    }

    start.wait().await;
    let mut held = Vec::with_capacity(TARGET_CONNECTIONS);
    let opening_result = tokio::time::timeout(OPEN_TIMEOUT, async {
        while let Some(joined) = tasks.join_next().await {
            held.push(joined.expect("network stream opening task"));
        }
    })
    .await;
    if opening_result.is_err() {
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        panic!(
            "opening 2,000 network SSE streams exceeded {OPEN_TIMEOUT:?}; opened={}",
            held.len()
        );
    }
    held
}

async fn open_network_stream_now(
    client: &reqwest::Client,
    events_url: &str,
    token_index: usize,
    token: &str,
) -> NetworkStream {
    let response = network_realtime_request(client, events_url, token).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "token {token_index} failed to open SSE"
    );
    assert!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("text/event-stream")),
        "token {token_index} did not receive an SSE response"
    );

    let mut stream = NetworkStream {
        token_index,
        response,
        pending: Vec::new(),
    };
    assert_next_network_event(&mut stream, "control.hello").await;
    assert_next_network_event(&mut stream, "resources.head").await;
    stream
}

async fn read_heartbeat_cycles(mut stream: NetworkStream) -> (NetworkStream, usize) {
    let mut observed = 0usize;
    while observed < HEARTBEAT_CYCLES {
        let event = next_network_sse_event(&mut stream).await;
        observed += event.matches("event: control.heartbeat").count();
    }
    (stream, observed)
}

async fn assert_next_network_event(stream: &mut NetworkStream, expected: &str) {
    let event = next_network_sse_event(stream).await;
    assert!(
        event.contains(&format!("event: {expected}")),
        "expected {expected}, received {event}"
    );
}

async fn next_network_sse_event(stream: &mut NetworkStream) -> String {
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

async fn network_realtime_request(
    client: &reqwest::Client,
    events_url: &str,
    token: &str,
) -> reqwest::Response {
    client
        .get(events_url)
        .bearer_auth(token)
        .header(header::ACCEPT.as_str(), "text/event-stream")
        .send()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "open realtime TCP connection: {error}; verify the process open-file limit is at least {}",
                TARGET_CONNECTIONS * 2 + FD_HEADROOM
            )
        })
}

async fn wait_for_connection_count(app: &TestApp, expected: usize) {
    tokio::time::timeout(CONNECTION_COUNT_TIMEOUT, async {
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
        max_connections_per_secret: CONNECTIONS_PER_SECRET,
        max_concurrent_handshakes,
        broadcast_capacity: 512,
        heartbeat_seconds: 1,
    }
}

async fn assert_network_capacity_response(
    response: reqwest::Response,
    status: StatusCode,
    message: &str,
) {
    assert_eq!(response.status(), status);
    assert_eq!(
        response.headers().get(header::RETRY_AFTER),
        Some(&header::HeaderValue::from_static("5"))
    );
    let json: Value = response.json().await.expect("capacity JSON");
    assert_eq!(json["error"], message);
    assert_eq!(json["retry_after_seconds"], 5);
}

#[cfg(unix)]
fn assert_fd_budget() -> String {
    let mut limit = MaybeUninit::<libc::rlimit>::uninit();
    // SAFETY: getrlimit writes one initialized rlimit into the valid pointer;
    // the return code is checked before assume_init.
    let result = unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, limit.as_mut_ptr()) };
    assert_eq!(
        result,
        0,
        "inspect RLIMIT_NOFILE: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: the successful getrlimit call initialized the value above.
    let limit = unsafe { limit.assume_init() };
    let required = (TARGET_CONNECTIONS * 2 + FD_HEADROOM) as libc::rlim_t;
    assert!(
        limit.rlim_cur >= required,
        "2,000 in-process TCP streams require at least {required} open files (client + server + headroom); soft RLIMIT_NOFILE is {}. Raise it before running this explicit acceptance test",
        limit.rlim_cur
    );
    limit.rlim_cur.to_string()
}

#[cfg(not(unix))]
fn assert_fd_budget() -> String {
    "not_available_on_this_platform".to_string()
}
