//! HTTP probe and scrape semantics when Pi-hole fetch succeeds or fails.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::{get, post},
    Json, Router,
};
use pihole_exporter::{
    config::ClientConfig,
    metrics::Metrics,
    pihole::PiHoleClientHandle,
    server::{router, AppState},
};
use serde_json::{json, Value};
use tokio::io::AsyncReadExt;
use tower::ServiceExt;

fn app(
    clients: Vec<PiHoleClientHandle>,
    metrics: Arc<Metrics>,
    health_check_timeout: Duration,
) -> axum::Router {
    router(AppState {
        clients: Arc::new(clients),
        metrics,
        health_check_timeout,
    })
}

fn empty_app() -> axum::Router {
    app(
        Vec::new(),
        Metrics::new().expect("registry"),
        Duration::from_millis(200),
    )
}

fn unreachable_client(timeout: Duration) -> PiHoleClientHandle {
    let config = ClientConfig {
        pihole_protocol: "http".to_string(),
        pihole_hostname: "127.0.0.1".to_string(),
        pihole_port: 1,
        pihole_password: String::new(),
    };
    PiHoleClientHandle::new(config, timeout, false).expect("valid client")
}

fn client_for_port(port: u16, timeout: Duration) -> PiHoleClientHandle {
    let config = ClientConfig {
        pihole_protocol: "http".to_string(),
        pihole_hostname: "127.0.0.1".to_string(),
        pihole_port: port,
        pihole_password: "secret".to_string(),
    };
    PiHoleClientHandle::new(config, timeout, false).expect("valid client")
}

async fn spawn_pihole_stub() -> u16 {
    async fn auth() -> Json<Value> {
        Json(json!({
            "session": {
                "valid": true,
                "sid": "test-sid",
                "validity": 3600
            }
        }))
    }

    async fn blocking() -> Json<Value> {
        Json(json!({ "blocking": "enabled" }))
    }

    let app = Router::new()
        .route("/api/auth", post(auth))
        .route("/api/dns/blocking", get(blocking));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub");
    let port = listener.local_addr().expect("stub addr").port();
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("stub server");
    });
    port
}

async fn spawn_hanging_server() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind hang");
    let port = listener.local_addr().expect("hang addr").port();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = [0_u8; 1024];
                let _ = socket.read(&mut buf).await;
                std::future::pending::<()>().await;
            });
        }
    });
    port
}

async fn status_of(app: axum::Router, path: &str) -> StatusCode {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .expect("response");
    response.status()
}

async fn body_of(app: axum::Router, path: &str) -> (StatusCode, String) {
    let response = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .expect("response");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let text = String::from_utf8(body.to_vec()).expect("utf8");
    (status, text)
}

#[tokio::test]
async fn liveness_always_ok() {
    assert_eq!(status_of(empty_app(), "/liveness").await, StatusCode::OK);
}

#[tokio::test]
async fn readiness_fails_until_first_successful_fetch() {
    assert_eq!(
        status_of(empty_app(), "/readiness").await,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn healthz_with_no_targets_is_ok() {
    assert_eq!(status_of(empty_app(), "/healthz").await, StatusCode::OK);
}

#[tokio::test]
async fn metrics_returns_503_when_upstream_unreachable() {
    let app = app(
        vec![unreachable_client(Duration::from_millis(200))],
        Metrics::new().expect("registry"),
        Duration::from_millis(200),
    );

    assert_eq!(
        status_of(app, "/metrics").await,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn successful_fetch_enables_readiness_and_metrics() {
    let metrics = Metrics::new().expect("registry");
    metrics.mark_success();

    let app = app(Vec::new(), Arc::clone(&metrics), Duration::from_millis(200));

    for path in ["/readiness", "/metrics"] {
        assert_eq!(
            status_of(app.clone(), path).await,
            StatusCode::OK,
            "{path} should succeed after fetch"
        );
    }
}

#[tokio::test]
async fn fetch_failure_marks_readiness_unhealthy_again() {
    let metrics = Metrics::new().expect("registry");
    metrics.mark_success();
    metrics.record_fetch_failure("authentication failed: token expired");

    let (status, body) = body_of(
        app(Vec::new(), metrics, Duration::from_millis(200)),
        "/readiness",
    )
    .await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("token expired"), "{body}");
}

#[tokio::test]
async fn healthz_live_probes_pihole_even_when_cached_status_failed() {
    let port = spawn_pihole_stub().await;
    let metrics = Metrics::new().expect("registry");
    metrics.record_fetch_failure("last scrape failed");

    let router = app(
        vec![client_for_port(port, Duration::from_secs(1))],
        Arc::clone(&metrics),
        Duration::from_secs(1),
    );

    assert_eq!(
        status_of(router.clone(), "/readiness").await,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(status_of(router, "/healthz").await, StatusCode::OK);
    assert!(
        metrics.upstream_status().is_err(),
        "live probe must not clear a failed scrape"
    );
}

#[tokio::test]
async fn healthz_fails_when_pihole_is_down_without_changing_cached_readiness() {
    let metrics = Metrics::new().expect("registry");
    metrics.mark_success();

    let router = app(
        vec![unreachable_client(Duration::from_millis(200))],
        Arc::clone(&metrics),
        Duration::from_millis(200),
    );

    assert_eq!(
        status_of(router.clone(), "/readiness").await,
        StatusCode::OK
    );

    let (status, body) = body_of(router.clone(), "/healthz").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("127.0.0.1"), "{body}");

    assert_eq!(status_of(router, "/readiness").await, StatusCode::OK);
    assert!(metrics.upstream_status().is_ok());
}

#[tokio::test]
async fn healthz_fails_when_any_target_is_unreachable() {
    let port = spawn_pihole_stub().await;
    let router = app(
        vec![
            client_for_port(port, Duration::from_secs(1)),
            unreachable_client(Duration::from_millis(200)),
        ],
        Metrics::new().expect("registry"),
        Duration::from_secs(1),
    );

    assert_eq!(
        status_of(router, "/healthz").await,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn healthz_times_out_on_slow_pihole() {
    let port = spawn_hanging_server().await;
    let router = app(
        vec![client_for_port(port, Duration::from_secs(2))],
        Metrics::new().expect("registry"),
        Duration::from_millis(200),
    );

    let (status, body) = body_of(router, "/healthz").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("timed out"), "{body}");
}

#[tokio::test]
async fn index_documents_live_healthz() {
    let (status, body) = body_of(empty_app(), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("live Pi-hole connectivity probe"));
    assert!(body.contains("cached upstream readiness"));
}
