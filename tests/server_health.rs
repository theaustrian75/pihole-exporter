//! HTTP probe and scrape semantics when Pi-hole fetch succeeds or fails.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use pihole_exporter::{
    config::ClientConfig,
    pihole::PiHoleClientHandle,
    server::{router, AppState},
    upstream::UpstreamHealth,
};
use tower::ServiceExt;

fn empty_app() -> axum::Router {
    router(AppState {
        clients: Arc::new(Vec::new()),
        upstream: Arc::new(RwLock::new(UpstreamHealth::default())),
    })
}

fn app_with_unreachable_client() -> axum::Router {
    let config = ClientConfig {
        pihole_protocol: "http".to_string(),
        pihole_hostname: "127.0.0.1".to_string(),
        pihole_port: 1,
        pihole_password: String::new(),
    };
    let client =
        PiHoleClientHandle::new(config, Duration::from_millis(200), false).expect("valid client");

    router(AppState {
        clients: Arc::new(vec![client]),
        upstream: Arc::new(RwLock::new(UpstreamHealth::default())),
    })
}

#[tokio::test]
async fn liveness_always_ok() {
    let response = empty_app()
        .oneshot(
            Request::builder()
                .uri("/liveness")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn readiness_and_healthz_fail_until_first_successful_fetch() {
    let app = empty_app();

    for path in ["/healthz", "/readiness"] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{path} should be unavailable before first fetch"
        );
    }
}

#[tokio::test]
async fn metrics_returns_503_when_upstream_unreachable() {
    let response = app_with_unreachable_client()
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn successful_fetch_enables_probes_and_metrics() {
    let upstream = Arc::new(RwLock::new(UpstreamHealth::default()));
    upstream.write().expect("lock").mark_success();

    let app = router(AppState {
        clients: Arc::new(Vec::new()),
        upstream: Arc::clone(&upstream),
    });

    for path in ["/healthz", "/readiness", "/metrics"] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .expect("response");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{path} should succeed after fetch"
        );
    }
}

#[tokio::test]
async fn fetch_failure_marks_upstream_unhealthy_again() {
    let upstream = Arc::new(RwLock::new(UpstreamHealth::default()));
    upstream.write().expect("lock").mark_success();
    upstream
        .write()
        .expect("lock")
        .record_failure("authentication failed: token expired");

    let app = router(AppState {
        clients: Arc::new(Vec::new()),
        upstream,
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("response");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
