//! HTTP surface: `/metrics` (Prometheus text format), health probes, and graceful shutdown.

use std::future::Future;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use axum_server::Handle;

use crate::metrics::Metrics;
use crate::pihole::PiHoleClientHandle;

const COLLECTION_TIMEOUT: Duration = Duration::from_secs(30);
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct AppState {
    pub clients: Arc<Vec<PiHoleClientHandle>>,
    pub metrics: Arc<Metrics>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/healthz", get(healthz))
        .route("/readiness", get(readiness))
        .route("/liveness", get(liveness))
        .route("/metrics", get(metrics_handler))
        .with_state(state)
}

/// Bind and serve until `shutdown` completes or the HTTP(S) server exits.
pub async fn run(
    app: Router,
    addr: SocketAddr,
    tls_cert_file: Option<&Path>,
    tls_key_file: Option<&Path>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), String> {
    match (tls_cert_file, tls_key_file) {
        (Some(cert_file), Some(key_file)) => {
            install_crypto_provider();
            let config = axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_file, key_file)
                .await
                .map_err(|err| {
                    format!(
                        "loading TLS certificate {} and key {}: {err}",
                        cert_file.display(),
                        key_file.display()
                    )
                })?;
            let handle = Handle::new();
            let server_handle = handle.clone();
            tokio::spawn(async move {
                shutdown.await;
                tracing::info!("stopping HTTPS server");
                handle.graceful_shutdown(Some(GRACEFUL_SHUTDOWN_TIMEOUT));
            });
            tracing::info!(addr = %addr, "metrics server listening (HTTPS)");
            axum_server::bind_rustls(addr, config)
                .handle(server_handle)
                .serve(app.into_make_service())
                .await
                .map_err(|err| format!("HTTPS server exited: {err}"))?;
        }
        (None, None) => {
            let listener = tokio::net::TcpListener::bind(addr)
                .await
                .map_err(|err| format!("binding {addr}: {err}"))?;
            let bound = listener.local_addr().unwrap_or(addr);
            tracing::info!(addr = %bound, "metrics server listening");
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown)
                .await
                .map_err(|err| format!("HTTP server exited: {err}"))?;
        }
        _ => {
            return Err(
                "TLS requires both tls_cert_file and tls_key_file; set both or omit entirely"
                    .to_string(),
            );
        }
    }
    Ok(())
}

pub async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut terminate =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            () = ctrl_c => {},
            _ = terminate.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await;
    }

    tracing::info!("shutdown signal received");
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
    upstream_health_response(&state.metrics)
}

async fn healthz(State(state): State<AppState>) -> Response {
    upstream_health_response(&state.metrics)
}

fn upstream_health_response(metrics: &Metrics) -> Response {
    match metrics.upstream_status() {
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

async fn metrics_handler(State(state): State<AppState>) -> Response {
    match scrape_and_update_health(state.clients.as_ref(), &state.metrics).await {
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

    match state.metrics.render() {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
            body,
        )
            .into_response(),
        Err(err) => {
            tracing::error!(error = %err, "failed to render metrics");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to render metrics: {err}"),
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
    metrics: &Metrics,
) -> Result<(), String> {
    match scrape_pihole_targets(clients).await {
        Ok(()) => {
            metrics.mark_success();
            Ok(())
        }
        Err(err) => {
            metrics.record_fetch_failure(&err);
            Err(err)
        }
    }
}

fn install_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("failed to install rustls ring crypto provider");
    }
}
