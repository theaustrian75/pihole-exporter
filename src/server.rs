use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::pihole::PiHoleClientHandle;
use crate::upstream::SharedUpstreamHealth;

const COLLECTION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<Vec<PiHoleClientHandle>>,
    pub upstream: SharedUpstreamHealth,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .route("/metrics", get(metrics_handler))
        .route("/readiness", get(readiness))
        .route("/liveness", get(liveness))
        .with_state(state)
}

async fn index() -> &'static str {
    "pihole-exporter\n\n\
       /metrics    Prometheus text format (503 when upstream fetch failed)\n\
       /healthz    upstream readiness probe (ok or 503)\n\
       /readiness  upstream readiness probe (ok or 503)\n\
       /liveness   process liveness probe (always ok)\n"
}

async fn liveness() -> &'static str {
    "ok"
}

async fn readiness(State(state): State<AppState>) -> Response {
    upstream_health_response(&state.upstream)
}

async fn healthz(State(state): State<AppState>) -> Response {
    upstream_health_response(&state.upstream)
}

fn upstream_health_response(upstream: &SharedUpstreamHealth) -> Response {
    match upstream_status(upstream) {
        Ok(()) => (StatusCode::OK, "ok").into_response(),
        Err(detail) => {
            tracing::error!(detail = %detail, "upstream unhealthy");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("error: upstream unavailable: {detail}"),
            )
                .into_response()
        }
    }
}

fn upstream_status(upstream: &SharedUpstreamHealth) -> Result<(), String> {
    upstream
        .read()
        .map_err(|_| "upstream health lock poisoned".to_string())?
        .status()
}

async fn metrics_handler(State(state): State<AppState>) -> Response {
    match scrape_and_update_health(state.clients.as_ref(), &state.upstream).await {
        Ok(()) => {}
        Err(detail) => {
            tracing::error!(detail = %detail, "refusing metrics scrape");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("failed to fetch upstream data: {detail}"),
            )
                .into_response();
        }
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
        Err(err) => {
            tracing::error!(error = %err, "failed to encode Prometheus metrics");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to encode metrics: {err}"),
            )
                .into_response()
        }
    }
}

/// Scrape all configured Pi-hole hosts and refresh upstream health.
pub async fn scrape_pihole_targets(clients: &[PiHoleClientHandle]) -> Result<(), String> {
    let mut handles = Vec::with_capacity(clients.len());

    for client in clients {
        let client = client.clone();
        handles.push(tokio::spawn(async move {
            let hostname = client.hostname().to_string();
            match tokio::time::timeout(COLLECTION_TIMEOUT, client.collect_metrics()).await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(err)) => {
                    tracing::error!(
                        host = %hostname,
                        error = %err,
                        "failed to collect metrics from Pi-hole"
                    );
                    Err(format!("{hostname}: {err}"))
                }
                Err(_) => {
                    let message = format!("{hostname}: metrics collection timed out");
                    tracing::error!(host = %hostname, "{message}");
                    Err(message)
                }
            }
        }));
    }

    let mut errors = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => errors.push(err),
            Err(err) => {
                let message = format!("metrics collection task failed: {err}");
                tracing::error!(error = %err, "{message}");
                errors.push(message);
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "failed to collect metrics from Pi-hole:\n{}",
            errors.join("\n")
        ))
    }
}

pub async fn scrape_and_update_health(
    clients: &[PiHoleClientHandle],
    upstream: &SharedUpstreamHealth,
) -> Result<(), String> {
    match scrape_pihole_targets(clients).await {
        Ok(()) => {
            if let Ok(mut health) = upstream.write() {
                health.mark_success();
            }
            Ok(())
        }
        Err(err) => {
            if let Ok(mut health) = upstream.write() {
                health.record_failure(&err);
            }
            Err(err)
        }
    }
}
