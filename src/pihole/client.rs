use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

use crate::config::ClientConfig;
use crate::metrics;

use super::api_client::{ApiClient, ApiError};
use super::model::{
    merge_clients, BlockingStatus, PiHoleClient, StatsSummary, TopClients, TopDomains, Upstreams,
};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("error fetching stats summary: {0}")]
    StatsSummary(#[source] ApiError),
    #[error("error fetching blocked domains: {0}")]
    BlockedDomains(#[source] ApiError),
    #[error("error fetching permitted domains: {0}")]
    PermittedDomains(#[source] ApiError),
    #[error("error fetching blocked clients: {0}")]
    BlockedClients(#[source] ApiError),
    #[error("error fetching permitted clients: {0}")]
    PermittedClients(#[source] ApiError),
    #[error("error fetching upstream stats: {0}")]
    Upstreams(#[source] ApiError),
    #[error("error fetching status: {0}")]
    Status(#[source] ApiError),
}

#[derive(Clone)]
pub struct PiHoleClientHandle {
    config: ClientConfig,
    api_client: Arc<ApiClient>,
}

impl PiHoleClientHandle {
    pub fn new(config: ClientConfig, timeout: Duration, skip_tls_verification: bool) -> Result<Self, crate::config::ConfigError> {
        config.validate()?;

        tracing::debug!(
            host = %config.pihole_hostname,
            protocol = %config.pihole_protocol,
            port = %config.pihole_port,
            "Creating client"
        );

        Ok(Self {
            api_client: Arc::new(ApiClient::new(
                config.base_url(),
                config.pihole_password.clone(),
                timeout,
                skip_tls_verification,
            )),
            config,
        })
    }

    pub fn hostname(&self) -> &str {
        &self.config.pihole_hostname
    }

    pub async fn collect_metrics(&self) -> Result<(), ClientError> {
        let (
            stats,
            blocked_domains,
            permitted_domains,
            clients,
            upstreams,
            pi_hole_status,
        ) = self.get_statistics().await?;

        self.set_metrics(
            &stats,
            &blocked_domains,
            &permitted_domains,
            &clients,
            &upstreams,
            &pi_hole_status,
        );

        tracing::debug!(
            host = %self.config.pihole_hostname,
            summary = %stats.summary_line(),
            "New tick of statistics"
        );

        Ok(())
    }

    fn set_metrics(
        &self,
        stats: &StatsSummary,
        blocked_domains: &TopDomains,
        permitted_domains: &TopDomains,
        clients: &[PiHoleClient],
        upstreams: &Upstreams,
        pi_hole_status: &BlockingStatus,
    ) {
        let hostname = &self.config.pihole_hostname;

        metrics::DOMAINS_BLOCKED
            .with_label_values(&[hostname])
            .set(stats.gravity.domains_being_blocked as f64);
        metrics::DNS_QUERIES_TODAY
            .with_label_values(&[hostname])
            .set(stats.queries.total as f64);
        metrics::ADS_BLOCKED_TODAY
            .with_label_values(&[hostname])
            .set(stats.queries.blocked as f64);
        metrics::ADS_PERCENTAGE_TODAY
            .with_label_values(&[hostname])
            .set(stats.queries.percent_blocked);
        metrics::UNIQUE_DOMAINS
            .with_label_values(&[hostname])
            .set(stats.queries.unique_domains as f64);
        metrics::QUERIES_FORWARDED
            .with_label_values(&[hostname])
            .set(stats.queries.forwarded as f64);
        metrics::QUERIES_CACHED
            .with_label_values(&[hostname])
            .set(stats.queries.cached as f64);
        metrics::REQUEST_RATE
            .with_label_values(&[hostname])
            .set(stats.queries.frequency);
        metrics::CLIENTS_EVER_SEEN
            .with_label_values(&[hostname])
            .set(stats.clients.total as f64);
        metrics::UNIQUE_CLIENTS
            .with_label_values(&[hostname])
            .set(stats.clients.active as f64);
        metrics::DNS_QUERIES_ALL_TYPES
            .with_label_values(&[hostname])
            .set(stats.queries.total as f64);

        let status = if pi_hole_status.blocking == "enabled" {
            1.0
        } else {
            0.0
        };
        metrics::STATUS.with_label_values(&[hostname]).set(status);

        let replies = &stats.queries.replies;
        for (reply_type, value) in [
            ("unknown", replies.UNKNOWN),
            ("no_data", replies.NODATA),
            ("nx_domain", replies.NXDOMAIN),
            ("cname", replies.CNAME),
            ("ip", replies.IP),
            ("domain", replies.DOMAIN),
            ("rr_name", replies.RRNAME),
            ("serv_fail", replies.SERVFAIL),
            ("refused", replies.REFUSED),
            ("not_imp", replies.NOTIMP),
            ("other", replies.OTHER),
            ("dnssec", replies.DNSSEC),
            ("none", replies.NONE),
            ("blob", replies.BLOB),
        ] {
            metrics::REPLY
                .with_label_values(&[hostname, reply_type])
                .set(value as f64);
        }

        for domain in &permitted_domains.domains {
            metrics::TOP_QUERIES
                .with_label_values(&[hostname, &domain.domain])
                .set(domain.count as f64);
        }

        for domain in &blocked_domains.domains {
            metrics::TOP_ADS
                .with_label_values(&[hostname, &domain.domain])
                .set(domain.count as f64);
        }

        for client in clients {
            metrics::TOP_SOURCES
                .with_label_values(&[hostname, &client.ip, &client.name])
                .set(client.count as f64);
        }

        for upstream in &upstreams.upstreams {
            metrics::FORWARD_DESTINATIONS
                .with_label_values(&[hostname, &upstream.ip, &upstream.name])
                .set(upstream.count as f64);
            metrics::FORWARD_DESTINATIONS_RESPONSE_TIME
                .with_label_values(&[hostname, &upstream.ip, &upstream.name])
                .set(upstream.statistics.response);
            metrics::FORWARD_DESTINATIONS_RESPONSE_VARIANCE
                .with_label_values(&[hostname, &upstream.ip, &upstream.name])
                .set(upstream.statistics.variance);
        }

        for (query_type, value) in &stats.queries.types {
            metrics::QUERY_TYPES
                .with_label_values(&[hostname, query_type])
                .set(*value);
        }
    }

    async fn get_statistics(
        &self,
    ) -> Result<
        (
            StatsSummary,
            TopDomains,
            TopDomains,
            Vec<PiHoleClient>,
            Upstreams,
            BlockingStatus,
        ),
        ClientError,
    > {
        let stats_summary: StatsSummary = self
            .api_client
            .fetch_data("/api/stats/summary")
            .await
            .map_err(ClientError::StatsSummary)?;

        let blocked_domains: TopDomains = self
            .api_client
            .fetch_data("/api/stats/top_domains?blocked=true&count=10")
            .await
            .map_err(ClientError::BlockedDomains)?;

        let permitted_domains: TopDomains = self
            .api_client
            .fetch_data("/api/stats/top_domains?blocked=false&count=10")
            .await
            .map_err(ClientError::PermittedDomains)?;

        let blocked_clients: TopClients = self
            .api_client
            .fetch_data("/api/stats/top_clients?blocked=true&count=10")
            .await
            .map_err(ClientError::BlockedClients)?;

        let permitted_clients: TopClients = self
            .api_client
            .fetch_data("/api/stats/top_clients?blocked=false&count=10")
            .await
            .map_err(ClientError::PermittedClients)?;

        let clients = merge_clients(&permitted_clients.clients, &blocked_clients.clients);

        let upstreams: Upstreams = self
            .api_client
            .fetch_data("/api/stats/upstreams")
            .await
            .map_err(ClientError::Upstreams)?;

        let pi_hole_status: BlockingStatus = self
            .api_client
            .fetch_data("/api/dns/blocking")
            .await
            .map_err(ClientError::Status)?;

        Ok((
            stats_summary,
            blocked_domains,
            permitted_domains,
            clients,
            upstreams,
            pi_hole_status,
        ))
    }
}
