use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::pihole::PiHoleClientHandle;

const COLLECTION_TIMEOUT: Duration = Duration::from_secs(30);
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<Vec<PiHoleClientHandle>>,
}

struct ProbeResult {
    hostname: String,
    success: bool,
    error: Option<String>,
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
    "pihole-exporter\n\n  /metrics   Prometheus text format\n  /healthz   Pi-hole connectivity probe (ok or error)\n  /readiness Pi-hole connectivity probe (ok or error)\n  /liveness  Pi-hole connectivity probe (ok or error)\n"
}

async fn probe_clients(clients: &[PiHoleClientHandle]) -> Vec<ProbeResult> {
    let mut handles = Vec::with_capacity(clients.len());

    for client in clients.iter().cloned() {
        handles.push(tokio::spawn(async move {
            let hostname = client.hostname().to_string();
            match tokio::time::timeout(HEALTH_CHECK_TIMEOUT, client.check_connection()).await {
                Ok(Ok(())) => ProbeResult {
                    hostname,
                    success: true,
                    error: None,
                },
                Ok(Err(err)) => {
                    tracing::error!(
                        host = %hostname,
                        error = %err,
                        "Pi-hole health check failed"
                    );
                    ProbeResult {
                        hostname,
                        success: false,
                        error: Some(err.to_string()),
                    }
                }
                Err(_) => {
                    let message = format!("connection check to {hostname} timed out");
                    tracing::error!(host = %hostname, "{message}");
                    ProbeResult {
                        hostname,
                        success: false,
                        error: Some(message),
                    }
                }
            }
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(err) => {
                tracing::error!(error = %err, "Pi-hole health check task failed");
                results.push(ProbeResult {
                    hostname: "unknown".to_string(),
                    success: false,
                    error: Some(format!("health check task failed: {err}")),
                });
            }
        }
    }

    results
}

fn format_probe_failures(failures: &[&ProbeResult]) -> String {
    failures
        .iter()
        .map(|result| {
            format!(
                "{}: {}",
                result.hostname,
                result.error.as_deref().unwrap_or("unknown error")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn pihole_health_response(clients: &[PiHoleClientHandle]) -> Response {
    let results = probe_clients(clients).await;
    let failures: Vec<_> = results.iter().filter(|result| !result.success).collect();

    if failures.is_empty() {
        return (StatusCode::OK, "ok").into_response();
    }

    tracing::error!(
        failed_hosts = failures.len(),
        total_hosts = results.len(),
        detail = %format_probe_failures(&failures),
        "Pi-hole unreachable during health check"
    );

    (
        StatusCode::SERVICE_UNAVAILABLE,
        format!(
            "error: Pi-hole unreachable:\n{}",
            format_probe_failures(&failures)
        ),
    )
        .into_response()
}

async fn healthz(State(state): State<AppState>) -> Response {
    pihole_health_response(state.clients.as_ref()).await
}

async fn metrics_handler(State(state): State<AppState>) -> Response {
    let mut handles = Vec::with_capacity(state.clients.len());

    for client in state.clients.iter().cloned() {
        handles.push(tokio::spawn(async move {
            let hostname = client.hostname().to_string();
            match tokio::time::timeout(COLLECTION_TIMEOUT, client.collect_metrics()).await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(err)) => {
                    tracing::error!(
                        host = %hostname,
                        error = %err,
                        "Failed to collect metrics from Pi-hole"
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

    if errors.len() == state.clients.len() {
        let body = format!(
            "failed to collect metrics from all Pi-hole hosts:\n{}",
            errors.join("\n")
        );
        tracing::error!(
            failed_hosts = errors.len(),
            detail = %body,
            "Failed to collect metrics from all Pi-hole hosts"
        );
        return (StatusCode::SERVICE_UNAVAILABLE, body).into_response();
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
            tracing::error!(error = %err, "Failed to encode Prometheus metrics");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to encode metrics: {err}"),
            )
                .into_response()
        }
    }
}

async fn readiness_handler(State(state): State<AppState>) -> Response {
    pihole_health_response(state.clients.as_ref()).await
}

async fn liveness_handler(State(state): State<AppState>) -> Response {
    pihole_health_response(state.clients.as_ref()).await
}
