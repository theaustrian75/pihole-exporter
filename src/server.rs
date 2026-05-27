use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::pihole::PiHoleClientHandle;

const COLLECTION_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<Vec<PiHoleClientHandle>>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .route("/metrics", get(metrics_handler))
        .route("/readiness", get(readiness_handler))
        .route("/liveness", get(liveness_handler))
        .with_state(state)
}

async fn index() -> &'static str {
    "pihole-exporter\n\n  /metrics   Prometheus text format\n  /healthz   liveness probe\n  /readiness readiness probe\n  /liveness  liveness probe\n"
}

async fn healthz() -> &'static str {
    "ok"
}

async fn metrics_handler(State(state): State<AppState>) -> Response {
    let mut handles = Vec::with_capacity(state.clients.len());

    for client in state.clients.iter().cloned() {
        handles.push(tokio::spawn(async move {
            let hostname = client.hostname().to_string();
            match tokio::time::timeout(COLLECTION_TIMEOUT, client.collect_metrics()).await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(err)) => {
                    tracing::warn!(
                        host = %hostname,
                        error = %err,
                        "An error occurred while contacting Pi-hole"
                    );
                    Err(err.to_string())
                }
                Err(_) => {
                    let message = format!("metrics collection from {hostname} timed out");
                    tracing::warn!(host = %hostname, "{message}");
                    Err(message)
                }
            }
        }));
    }

    for handle in handles {
        let _ = handle.await;
    }

    match crate::metrics::encode_metrics() {
        Ok(body) => (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4; charset=utf-8",
            )],
            body,
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode metrics: {err}"),
        )
            .into_response(),
    }
}

async fn readiness_handler() -> StatusCode {
    StatusCode::OK
}

async fn liveness_handler() -> StatusCode {
    StatusCode::OK
}
