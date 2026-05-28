use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use pihole_exporter::config::{Cli, EnvConfig};
use pihole_exporter::pihole::PiHoleClientHandle;
use pihole_exporter::server::{router, scrape_and_update_health, AppState};
use pihole_exporter::upstream::{SharedUpstreamHealth, UpstreamHealth};
use tracing_subscriber::EnvFilter;

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
    pihole_exporter::metrics::init();

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

    let upstream: SharedUpstreamHealth =
        Arc::new(std::sync::RwLock::new(UpstreamHealth::default()));
    let clients = Arc::new(clients);

    if let Err(err) = probe_pihole_targets(clients.as_ref(), Arc::clone(&upstream)).await {
        tracing::error!("{err}");
        std::process::exit(1);
    }

    let app = router(AppState {
        clients: Arc::clone(&clients),
        upstream: Arc::clone(&upstream),
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
    if env_config.tls_enabled() {
        let cert_file = env_config
            .tls_cert_file
            .as_ref()
            .expect("tls_cert_file set when tls_enabled");
        let key_file = env_config
            .tls_key_file
            .as_ref()
            .expect("tls_key_file set when tls_enabled");

        pihole_exporter::tls::serve(addr, app, cert_file, key_file, shutdown_signal()).await;
        return;
    }

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            tracing::error!(%addr, error = %err, "failed to bind HTTP server");
            std::process::exit(1);
        }
    };

    tracing::info!(%addr, "metrics server listening");
    tracing::info!("available endpoints: /metrics /healthz /readiness /liveness");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("HTTP server error");

    tracing::info!("pihole-exporter HTTP server stopped");
}

async fn probe_pihole_targets(
    clients: &[PiHoleClientHandle],
    upstream: SharedUpstreamHealth,
) -> Result<(), String> {
    tracing::info!(
        count = clients.len(),
        attempts = STARTUP_MAX_ATTEMPTS,
        "checking Pi-hole connectivity"
    );

    let mut last_error = "no successful fetch yet".to_string();
    for attempt in 1..=STARTUP_MAX_ATTEMPTS {
        if attempt > 1 {
            let delay = STARTUP_RETRY_DELAY * (attempt - 1);
            tracing::warn!(
                attempt,
                max_attempts = STARTUP_MAX_ATTEMPTS,
                retry_in = ?delay,
                "retrying Pi-hole fetch"
            );
            tokio::time::sleep(delay).await;
        }

        tracing::info!(
            attempt,
            max_attempts = STARTUP_MAX_ATTEMPTS,
            "fetching Pi-hole metrics"
        );

        if scrape_and_update_health(clients, &upstream).await.is_ok() {
            tracing::info!(attempt, "Pi-hole fetch successful");
            return Ok(());
        }

        last_error = upstream
            .read()
            .ok()
            .and_then(|health| health.status().err())
            .unwrap_or_else(|| "Pi-hole fetch failed".to_string());
    }

    Err(format!(
        "failed to fetch Pi-hole data after {STARTUP_MAX_ATTEMPTS} attempts: {last_error}"
    ))
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
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C handler");
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
