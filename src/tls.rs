use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum_server::{bind_rustls, tls_rustls::RustlsConfig, Handle};

pub async fn serve(
    addr: SocketAddr,
    app: Router,
    cert_file: &Path,
    key_file: &Path,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) {
    install_crypto_provider();

    let tls_config = match RustlsConfig::from_pem_file(cert_file, key_file).await {
        Ok(config) => config,
        Err(err) => {
            tracing::error!(
                cert_file = %cert_file.display(),
                key_file = %key_file.display(),
                error = %err,
                "failed to load TLS certificate or key"
            );
            std::process::exit(1);
        }
    };

    let handle = Handle::new();
    let shutdown_handle = handle.clone();
    tokio::spawn(async move {
        shutdown.await;
        shutdown_handle.graceful_shutdown(None);
    });

    tracing::info!(%addr, "HTTPS server listening");
    tracing::info!("available endpoints: /metrics /healthz /readiness /liveness");

    if let Err(err) = bind_rustls(addr, tls_config)
        .handle(handle)
        .serve(app.into_make_service())
        .await
    {
        tracing::error!(error = %err, "HTTPS server error");
        std::process::exit(1);
    }

    tracing::info!("pihole-exporter HTTPS server stopped");
}

fn install_crypto_provider() {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .expect("failed to install rustls ring crypto provider");
    }
}
