mod config;
mod metrics;
mod pihole;
mod server;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::config::{Cli, EnvConfig};
use crate::pihole::PiHoleClientHandle;
use crate::server::{router, AppState};

const STARTUP_MAX_ATTEMPTS: u32 = 3;
const STARTUP_RETRY_DELAY: Duration = Duration::from_millis(500);

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_logging(cli.debug);

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "starting pihole-exporter"
    );

    let (env_config, client_configs) = match EnvConfig::from_cli(cli) {
        Ok(config) => config,
        Err(err) => {
            tracing::error!("failed to load configuration: {err}");
            std::process::exit(1);
        }
    };

    tracing::info!("registering prometheus metrics");
    metrics::init();

    let clients: Result<Vec<PiHoleClientHandle>, _> = client_configs
        .into_iter()
        .map(|config| {
            PiHoleClientHandle::new(config, env_config.timeout, env_config.skip_tls_verification)
        })
        .collect();

    let clients = match clients {
        Ok(clients) => clients,
        Err(err) => {
            tracing::error!("failed to create Pi-hole client: {err}");
            std::process::exit(1);
        }
    };

    if let Err(err) = probe_pihole_targets(&clients, env_config.timeout).await {
        tracing::error!("{err}");
        std::process::exit(1);
    }

    let app = router(AppState {
        clients: Arc::new(clients),
    });

    let ip: IpAddr = match env_config.bind_addr.parse() {
        Ok(ip) => ip,
        Err(err) => {
            tracing::error!(
                bind_addr = %env_config.bind_addr,
                error = %err,
                "invalid bind address"
            );
            std::process::exit(1);
        }
    };
    let addr = SocketAddr::from((ip, env_config.port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!(%addr, error = %err, "failed to bind HTTP server");
            std::process::exit(1);
        }
    };

    tracing::info!(%addr, "HTTP server listening");
    tracing::info!("available endpoints: /metrics /healthz /readiness /liveness");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("HTTP server error");

    tracing::info!("pihole-exporter HTTP server stopped");
}

async fn probe_pihole_targets(
    clients: &[PiHoleClientHandle],
    connection_timeout: Duration,
) -> Result<(), String> {
    let probe_timeout = startup_probe_timeout(connection_timeout);

    tracing::info!(
        count = clients.len(),
        attempts = STARTUP_MAX_ATTEMPTS,
        timeout = ?probe_timeout,
        "checking Pi-hole connectivity"
    );

    for client in clients {
        let hostname = client.hostname().to_string();
        let mut last_error = String::new();
        let mut connected = false;

        for attempt in 1..=STARTUP_MAX_ATTEMPTS {
            if attempt > 1 {
                let delay = STARTUP_RETRY_DELAY * (attempt - 1);
                tracing::warn!(
                    host = %hostname,
                    attempt,
                    max_attempts = STARTUP_MAX_ATTEMPTS,
                    retry_in = ?delay,
                    "retrying Pi-hole connection"
                );
                tokio::time::sleep(delay).await;
            }

            tracing::info!(
                host = %hostname,
                attempt,
                max_attempts = STARTUP_MAX_ATTEMPTS,
                timeout = ?probe_timeout,
                "connecting to Pi-hole"
            );

            match tokio::time::timeout(probe_timeout, client.check_connection()).await {
                Ok(Ok(())) => {
                    tracing::info!(
                        host = %hostname,
                        attempt,
                        "Pi-hole connection successful"
                    );
                    connected = true;
                    break;
                }
                Ok(Err(err)) => {
                    last_error = err.to_string();
                    tracing::error!(
                        host = %hostname,
                        attempt,
                        max_attempts = STARTUP_MAX_ATTEMPTS,
                        error = %err,
                        "Pi-hole connection failed"
                    );
                }
                Err(_) => {
                    last_error = format!("connection timed out after {}s", probe_timeout.as_secs());
                    tracing::error!(
                        host = %hostname,
                        attempt,
                        max_attempts = STARTUP_MAX_ATTEMPTS,
                        timeout = ?probe_timeout,
                        "Pi-hole connection timed out"
                    );
                }
            }
        }

        if !connected {
            return Err(format!(
                "failed to connect to Pi-hole host {hostname} after {STARTUP_MAX_ATTEMPTS} attempts: {last_error}"
            ));
        }
    }

    Ok(())
}

fn startup_probe_timeout(connection_timeout: Duration) -> Duration {
    // Allow time for session authentication plus a follow-up API call.
    connection_timeout
        .saturating_mul(2)
        .max(Duration::from_secs(5))
}

fn init_logging(debug: bool) {
    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
    tracing::info!("shutdown signal received");
}
