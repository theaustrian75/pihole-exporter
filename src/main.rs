use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use pihole_exporter::config::{Cli, EnvConfig};
use pihole_exporter::metrics::Metrics;
use pihole_exporter::pihole::PiHoleClientHandle;
use pihole_exporter::server::{
    router, run as serve, scrape_and_update_health, shutdown_signal, AppState,
};

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

    let metrics = match Metrics::new() {
        Ok(metrics) => metrics,
        Err(err) => {
            tracing::error!(error = %err, "failed to set up Prometheus metrics");
            std::process::exit(1);
        }
    };

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

    let clients = Arc::new(clients);

    if let Err(err) = probe_pihole_targets(clients.as_ref(), Arc::clone(&metrics)).await {
        tracing::error!("{err}");
        std::process::exit(1);
    }

    let app = router(AppState {
        clients: Arc::clone(&clients),
        metrics: Arc::clone(&metrics),
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

    if let Err(err) = serve(
        app,
        addr,
        env_config.tls_cert_file.as_deref(),
        env_config.tls_key_file.as_deref(),
        shutdown_signal(),
    )
    .await
    {
        tracing::error!("{err}");
        std::process::exit(1);
    }

    tracing::info!("pihole-exporter stopped");
}

async fn probe_pihole_targets(
    clients: &[PiHoleClientHandle],
    metrics: Arc<Metrics>,
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

        if scrape_and_update_health(clients, metrics.as_ref())
            .await
            .is_ok()
        {
            tracing::info!(attempt, "Pi-hole fetch successful");
            return Ok(());
        }

        last_error = metrics
            .upstream_status()
            .err()
            .unwrap_or_else(|| "Pi-hole fetch failed".to_string());
    }

    Err(format!(
        "failed to fetch Pi-hole data after {STARTUP_MAX_ATTEMPTS} attempts: {last_error}"
    ))
}

fn init_logging(debug: bool) {
    use tracing_subscriber::EnvFilter;

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
