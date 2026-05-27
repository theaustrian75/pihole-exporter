use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::config::ClientConfig;
use crate::metrics;

use super::api_client::{ApiClient, ApiError};
use super::info::{
    ComponentVersions, DatabaseInfo, FtlResponse, SensorsResponse, SystemResponse, VersionDetail,
    VersionResponse,
};
use super::model::{
    merge_clients, normalize_status_label, BlockingStatus, HistoryResponse, HistorySlot,
    PiHoleClient, StatsSummary, TopClients, TopDomains, Upstreams,
};

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("authentication failed: {0}")]
    Auth(#[source] ApiError),
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

struct CollectedStats {
    stats: StatsSummary,
    blocked_domains: TopDomains,
    permitted_domains: TopDomains,
    clients: Vec<PiHoleClient>,
    upstreams: Upstreams,
    pi_hole_status: BlockingStatus,
    history: Option<HistoryResponse>,
    version: Option<VersionResponse>,
    ftl: Option<FtlResponse>,
    system: Option<SystemResponse>,
    database: Option<DatabaseInfo>,
    sensors: Option<SensorsResponse>,
}

#[derive(Clone)]
pub struct PiHoleClientHandle {
    config: ClientConfig,
    api_client: Arc<ApiClient>,
}

impl PiHoleClientHandle {
    pub fn new(
        config: ClientConfig,
        timeout: Duration,
        skip_tls_verification: bool,
    ) -> Result<Self, crate::config::ConfigError> {
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
        let hostname = self.hostname().to_string();
        let start = Instant::now();
        let result = self.collect_and_set_metrics().await;
        let elapsed = start.elapsed().as_secs_f64();

        metrics::SCRAPE_DURATION_SECONDS
            .with_label_values(&[hostname.as_str()])
            .set(elapsed);

        match &result {
            Ok(()) => {
                metrics::SCRAPE_SUCCESS
                    .with_label_values(&[hostname.as_str()])
                    .set(1.0);
                metrics::LAST_SCRAPE_TIMESTAMP
                    .with_label_values(&[hostname.as_str()])
                    .set(unix_now());
            }
            Err(_) => {
                metrics::SCRAPE_SUCCESS
                    .with_label_values(&[hostname.as_str()])
                    .set(0.0);
            }
        }

        result
    }

    async fn collect_and_set_metrics(&self) -> Result<(), ClientError> {
        let data = self.fetch_all().await?;
        self.set_metrics(&data);

        tracing::debug!(
            host = %self.config.pihole_hostname,
            summary = %data.stats.summary_line(),
            "New tick of statistics"
        );

        Ok(())
    }

    async fn fetch_all(&self) -> Result<CollectedStats, ClientError> {
        self.api_client
            .ensure_session()
            .await
            .map_err(ClientError::Auth)?;

        let (
            stats,
            blocked_domains,
            permitted_domains,
            blocked_clients,
            permitted_clients,
            upstreams,
            pi_hole_status,
            history,
            version,
            ftl,
            system,
            database,
            sensors,
        ) = tokio::join!(
            self.api_client.fetch_data::<StatsSummary>("/api/stats/summary"),
            self.api_client.fetch_data::<TopDomains>("/api/stats/top_domains?blocked=true&count=10"),
            self.api_client.fetch_data::<TopDomains>("/api/stats/top_domains?blocked=false&count=10"),
            self.api_client.fetch_data::<TopClients>("/api/stats/top_clients?blocked=true&count=10"),
            self.api_client.fetch_data::<TopClients>("/api/stats/top_clients?blocked=false&count=10"),
            self.api_client.fetch_data::<Upstreams>("/api/stats/upstreams"),
            self.api_client.fetch_data::<BlockingStatus>("/api/dns/blocking"),
            self.fetch_optional::<HistoryResponse>("/api/history"),
            self.fetch_optional::<VersionResponse>("/api/info/version"),
            self.fetch_optional::<FtlResponse>("/api/info/ftl"),
            self.fetch_optional::<SystemResponse>("/api/info/system"),
            self.fetch_optional::<DatabaseInfo>("/api/info/database"),
            self.fetch_optional::<SensorsResponse>("/api/info/sensors"),
        );

        Ok(CollectedStats {
            stats: stats.map_err(ClientError::StatsSummary)?,
            blocked_domains: blocked_domains.map_err(ClientError::BlockedDomains)?,
            permitted_domains: permitted_domains.map_err(ClientError::PermittedDomains)?,
            clients: merge_clients(
                &permitted_clients.map_err(ClientError::PermittedClients)?.clients,
                &blocked_clients.map_err(ClientError::BlockedClients)?.clients,
            ),
            upstreams: upstreams.map_err(ClientError::Upstreams)?,
            pi_hole_status: pi_hole_status.map_err(ClientError::Status)?,
            history,
            version,
            ftl,
            system,
            database,
            sensors,
        })
    }

    async fn fetch_optional<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
    ) -> Option<T> {
        match self.api_client.fetch_data(endpoint).await {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!(
                    host = %self.config.pihole_hostname,
                    endpoint,
                    error = %err,
                    "Optional Pi-hole API fetch failed"
                );
                None
            }
        }
    }

    fn set_metrics(&self, data: &CollectedStats) {
        let hostname = self.config.pihole_hostname.as_str();
        let stats = &data.stats;

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
        metrics::API_SUMMARY_TOOK_SECONDS
            .with_label_values(&[hostname])
            .set(stats.took);

        if stats.gravity.last_update > 0 {
            metrics::GRAVITY_LAST_UPDATE
                .with_label_values(&[hostname])
                .set(stats.gravity.last_update as f64);
            metrics::GRAVITY_AGE_SECONDS
                .with_label_values(&[hostname])
                .set((unix_now() as i64 - stats.gravity.last_update) as f64);
        }

        for (status, value) in &stats.queries.status {
            metrics::QUERY_STATUS
                .with_label_values(&[hostname, &normalize_status_label(status)])
                .set(*value as f64);
        }

        let status = if data.pi_hole_status.blocking == "enabled" {
            1.0
        } else {
            0.0
        };
        metrics::STATUS.with_label_values(&[hostname]).set(status);

        let blocking_timer = data.pi_hole_status.timer.unwrap_or(0.0);
        metrics::BLOCKING_TIMER_SECONDS
            .with_label_values(&[hostname])
            .set(blocking_timer);

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

        for domain in &data.permitted_domains.domains {
            metrics::TOP_QUERIES
                .with_label_values(&[hostname, &domain.domain])
                .set(domain.count as f64);
        }

        for domain in &data.blocked_domains.domains {
            metrics::TOP_ADS
                .with_label_values(&[hostname, &domain.domain])
                .set(domain.count as f64);
        }

        for client in &data.clients {
            metrics::TOP_SOURCES
                .with_label_values(&[hostname, &client.ip, &client.name])
                .set(client.count as f64);
        }

        metrics::UPSTREAM_FORWARDED_QUERIES
            .with_label_values(&[hostname])
            .set(data.upstreams.forwarded_queries as f64);
        metrics::UPSTREAM_TOTAL_QUERIES
            .with_label_values(&[hostname])
            .set(data.upstreams.total_queries as f64);

        for upstream in &data.upstreams.upstreams {
            let port = upstream.port_label();
            metrics::FORWARD_DESTINATIONS
                .with_label_values(&[hostname, &upstream.ip, &upstream.name, &port])
                .set(upstream.count as f64);
            metrics::FORWARD_DESTINATIONS_RESPONSE_TIME
                .with_label_values(&[hostname, &upstream.ip, &upstream.name, &port])
                .set(upstream.statistics.response);
            metrics::FORWARD_DESTINATIONS_RESPONSE_VARIANCE
                .with_label_values(&[hostname, &upstream.ip, &upstream.name, &port])
                .set(upstream.statistics.variance);
        }

        for (query_type, value) in &stats.queries.types {
            metrics::QUERY_TYPES
                .with_label_values(&[hostname, query_type])
                .set(*value);
        }

        if let Some(history) = &data.history {
            self.set_history_metrics(hostname, history);
        }

        if let Some(version) = &data.version {
            set_component_versions(hostname, "core", &version.version.core);
            set_component_versions(hostname, "web", &version.version.web);
            set_component_versions(hostname, "ftl", &version.version.ftl);
            if let Some(docker) = &version.version.docker {
                set_component_versions(hostname, "docker", docker);
            }
        }

        if let Some(ftl) = &data.ftl {
            metrics::FTL_UPTIME_SECONDS
                .with_label_values(&[hostname])
                .set(ftl.ftl.uptime / 1000.0);
            metrics::FTL_PID
                .with_label_values(&[hostname])
                .set(ftl.ftl.pid as f64);
            metrics::FTL_MEM_PERCENT
                .with_label_values(&[hostname])
                .set(ftl.ftl.mem_percent);
            metrics::FTL_CPU_PERCENT
                .with_label_values(&[hostname])
                .set(ftl.ftl.cpu_percent);
            metrics::FTL_QUERY_FREQUENCY
                .with_label_values(&[hostname])
                .set(ftl.ftl.query_frequency);
            metrics::FTL_PRIVACY_LEVEL
                .with_label_values(&[hostname])
                .set(ftl.ftl.privacy_level as f64);

            if let Some(db) = &ftl.ftl.database {
                metrics::FTL_DATABASE_GRAVITY
                    .with_label_values(&[hostname])
                    .set(db.gravity as f64);
                metrics::FTL_DATABASE_GROUPS
                    .with_label_values(&[hostname])
                    .set(db.groups as f64);
                metrics::FTL_DATABASE_LISTS
                    .with_label_values(&[hostname])
                    .set(db.lists as f64);
                metrics::FTL_DATABASE_CLIENTS
                    .with_label_values(&[hostname])
                    .set(db.clients as f64);
            }
        }

        if let Some(system) = &data.system {
            let sys = &system.system;
            metrics::SYSTEM_UPTIME_SECONDS
                .with_label_values(&[hostname])
                .set(sys.uptime);
            metrics::SYSTEM_RAM_TOTAL_KB
                .with_label_values(&[hostname])
                .set(sys.memory.ram.total);
            metrics::SYSTEM_RAM_FREE_KB
                .with_label_values(&[hostname])
                .set(sys.memory.ram.free);
            metrics::SYSTEM_RAM_USED_KB
                .with_label_values(&[hostname])
                .set(sys.memory.ram.used);
            metrics::SYSTEM_RAM_AVAILABLE_KB
                .with_label_values(&[hostname])
                .set(sys.memory.ram.available);
            metrics::SYSTEM_RAM_USED_PERCENT
                .with_label_values(&[hostname])
                .set(sys.memory.ram.used_percent);
            metrics::SYSTEM_SWAP_TOTAL_KB
                .with_label_values(&[hostname])
                .set(sys.memory.swap.total);
            metrics::SYSTEM_SWAP_FREE_KB
                .with_label_values(&[hostname])
                .set(sys.memory.swap.free);
            metrics::SYSTEM_SWAP_USED_KB
                .with_label_values(&[hostname])
                .set(sys.memory.swap.used);
            metrics::SYSTEM_SWAP_USED_PERCENT
                .with_label_values(&[hostname])
                .set(sys.memory.swap.used_percent);
            metrics::SYSTEM_CPU_PERCENT
                .with_label_values(&[hostname])
                .set(sys.cpu.cpu_percent);
            metrics::SYSTEM_CPU_NPROCS
                .with_label_values(&[hostname])
                .set(sys.cpu.nprocs as f64);
            metrics::SYSTEM_FTL_MEM_PERCENT
                .with_label_values(&[hostname])
                .set(sys.ftl.mem_percent);
            metrics::SYSTEM_FTL_CPU_PERCENT
                .with_label_values(&[hostname])
                .set(sys.ftl.cpu_percent);

            for (idx, period) in ["1", "5", "15"].iter().enumerate() {
                if let Some(load) = sys.cpu.load.raw.get(idx) {
                    metrics::SYSTEM_LOAD
                        .with_label_values(&[hostname, period])
                        .set(*load);
                }
                if let Some(load) = sys.cpu.load.percent.get(idx) {
                    metrics::SYSTEM_LOAD
                        .with_label_values(&[hostname, &format!("{period}_percent")])
                        .set(*load);
                }
            }
        }

        if let Some(database) = &data.database {
            metrics::DATABASE_SIZE_BYTES
                .with_label_values(&[hostname])
                .set(database.size as f64);
            metrics::DATABASE_QUERIES
                .with_label_values(&[hostname])
                .set(database.queries as f64);
            metrics::DATABASE_EARLIEST_TIMESTAMP
                .with_label_values(&[hostname])
                .set(database.earliest_timestamp);
            metrics::DATABASE_QUERIES_DISK
                .with_label_values(&[hostname])
                .set(database.queries_disk as f64);
            metrics::DATABASE_EARLIEST_TIMESTAMP_DISK
                .with_label_values(&[hostname])
                .set(database.earliest_timestamp_disk);
        }

        if let Some(sensors) = &data.sensors {
            let unit = sensors.sensors.unit.as_deref().unwrap_or("C");
            if let Some(temp) = sensors.sensors.cpu_temp {
                metrics::CPU_TEMP
                    .with_label_values(&[hostname, unit])
                    .set(temp);
            }
            if let Some(limit) = sensors.sensors.hot_limit {
                metrics::CPU_TEMP_HOT_LIMIT
                    .with_label_values(&[hostname, unit])
                    .set(limit);
            }
        }
    }

    fn set_history_metrics(&self, hostname: &str, history: &HistoryResponse) {
        for slot in &history.history {
            let timestamp = slot.timestamp.to_string();
            for (field, value) in history_fields(slot) {
                metrics::HISTORY
                    .with_label_values(&[hostname, &timestamp, field])
                    .set(value);
            }
        }

        if let Some(latest) = latest_history_slot(&history.history) {
            metrics::QUERIES_LAST_10MIN
                .with_label_values(&[hostname])
                .set(latest.total as f64);
            metrics::ADS_LAST_10MIN
                .with_label_values(&[hostname])
                .set(latest.blocked as f64);
        }
    }
}

fn history_fields(slot: &HistorySlot) -> [(&str, f64); 4] {
    [
        ("total", slot.total as f64),
        ("blocked", slot.blocked as f64),
        ("cached", slot.cached as f64),
        ("forwarded", slot.forwarded as f64),
    ]
}

fn latest_history_slot(history: &[HistorySlot]) -> Option<&HistorySlot> {
    history.iter().max_by_key(|slot| slot.timestamp)
}

fn set_component_versions(hostname: &str, component: &str, versions: &ComponentVersions) {
    set_version_metrics(hostname, component, "local", versions.local.as_ref());
    set_version_metrics(hostname, component, "remote", versions.remote.as_ref());
}

fn set_version_metrics(
    hostname: &str,
    component: &str,
    scope: &str,
    detail: Option<&VersionDetail>,
) {
    let Some(detail) = detail else {
        return;
    };

    let version = detail.version.as_deref().unwrap_or("unknown");
    let branch = detail.branch.as_deref().unwrap_or("");
    let hash = detail.hash.as_deref().unwrap_or("");
    let component = format!("{component}_{scope}");
    metrics::VERSION_INFO
        .with_label_values(&[hostname, &component, version, branch, hash])
        .set(1.0);
}

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}
