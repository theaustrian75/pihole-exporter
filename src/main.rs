mod config;
mod metrics;
mod pihole;
mod server;

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use tracing_subscriber::EnvFilter;

use crate::config::EnvConfig;
use crate::pihole::PiHoleClientHandle;
use crate::server::{router, AppState};

#[tokio::main]
async fn main() {
    let (env_config, client_configs) = match EnvConfig::load() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("failed to load configuration: {err}");
            std::process::exit(1);
        }
    };

    init_logging(env_config.debug);

    tracing::info!("starting pihole-exporter");
    metrics::init();

    let clients: Result<Vec<PiHoleClientHandle>, _> = client_configs
        .into_iter()
        .map(|config| {
            PiHoleClientHandle::new(
                config,
                env_config.timeout,
                env_config.skip_tls_verification,
            )
        })
        .collect();

    let clients = match clients {
        Ok(clients) => clients,
        Err(err) => {
            eprintln!("failed to create Pi-hole client: {err}");
            std::process::exit(1);
        }
    };

    let app = router(AppState {
        clients: Arc::new(clients),
    });

    let ip: IpAddr = env_config
        .bind_addr
        .parse()
        .expect("invalid bind address");
    let addr = SocketAddr::from((ip, env_config.port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind HTTP server");

    tracing::info!(%addr, "Starting HTTP server");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("HTTP server error");

    tracing::info!("pihole-exporter HTTP server stopped");
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
        .init();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C handler");
    tracing::info!("shutdown signal received");
}
