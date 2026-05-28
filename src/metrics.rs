use once_cell::sync::Lazy;
use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};

pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

pub static DOMAINS_BLOCKED: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "domains_being_blocked",
        "This represent the number of domains being blocked",
        &["hostname"],
    )
});

pub static DNS_QUERIES_TODAY: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "dns_queries_today",
        "This represent the number of DNS queries made over the current day",
        &["hostname"],
    )
});

pub static ADS_BLOCKED_TODAY: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ads_blocked_today",
        "This represent the number of ads blocked over the current day",
        &["hostname"],
    )
});

pub static ADS_PERCENTAGE_TODAY: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ads_percentage_today",
        "This represent the percentage of ads blocked over the current day",
        &["hostname"],
    )
});

pub static UNIQUE_DOMAINS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "unique_domains",
        "This represent the number of unique domains seen",
        &["hostname"],
    )
});

pub static QUERIES_FORWARDED: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "queries_forwarded",
        "This represent the number of queries forwarded",
        &["hostname"],
    )
});

pub static QUERIES_CACHED: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "queries_cached",
        "This represent the number of queries cached",
        &["hostname"],
    )
});

pub static CLIENTS_EVER_SEEN: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "clients_ever_seen",
        "This represent the number of clients ever seen",
        &["hostname"],
    )
});

pub static UNIQUE_CLIENTS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "unique_clients",
        "This represent the number of unique clients seen in the last 24h",
        &["hostname"],
    )
});

pub static REQUEST_RATE: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "request_rate",
        "This represent the number of requests per second",
        &["hostname"],
    )
});

pub static DNS_QUERIES_ALL_TYPES: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "dns_queries_all_types",
        "This represent the number of DNS queries made for all types",
        &["hostname"],
    )
});

pub static REPLY: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "reply",
        "This represent the number of replies made for all types",
        &["hostname", "type"],
    )
});

pub static TOP_QUERIES: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "top_queries",
        "This represent the number of top queries made by Pi-hole by domain",
        &["hostname", "domain"],
    )
});

pub static TOP_ADS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "top_ads",
        "This represent the number of top ads made by Pi-hole by domain",
        &["hostname", "domain"],
    )
});

pub static TOP_SOURCES: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "top_sources",
        "This represent the number of top sources requests made by Pi-hole by source host",
        &["hostname", "source", "source_name"],
    )
});

pub static FORWARD_DESTINATIONS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "forward_destinations",
        "This represent the number of forward destinations requests made by Pi-hole by destination",
        &["hostname", "destination", "destination_name", "port"],
    )
});

pub static FORWARD_DESTINATIONS_RESPONSE_TIME: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "forward_destinations_responsetime",
        "This represent the seconds a forward destinations took to process a requests made by Pi-hole",
        &["hostname", "destination", "destination_name", "port"],
    )
});

pub static FORWARD_DESTINATIONS_RESPONSE_VARIANCE: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "forward_destinations_responsevariance",
        "This represent the variants in response time a forward destinations took to process a requests made by Pi-hole",
        &["hostname", "destination", "destination_name", "port"],
    )
});

pub static QUERY_TYPES: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "querytypes",
        "This represent the number of queries made by Pi-hole by type",
        &["hostname", "type"],
    )
});

pub static STATUS: Lazy<GaugeVec> =
    Lazy::new(|| register_metric("status", "This if Pi-hole is enabled", &["hostname"]));

pub static QUERY_STATUS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "query_status",
        "This represent the number of queries by Pi-hole processing status",
        &["hostname", "status"],
    )
});

pub static GRAVITY_LAST_UPDATE: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "gravity_last_update",
        "Unix timestamp of the last gravity update",
        &["hostname"],
    )
});

pub static GRAVITY_AGE_SECONDS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "gravity_age_seconds",
        "Seconds since the last gravity update",
        &["hostname"],
    )
});

pub static QUERIES_LAST_10MIN: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "queries_last_10min",
        "This represent the number of queries in the last full slot of 10 minutes",
        &["hostname"],
    )
});

pub static ADS_LAST_10MIN: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ads_last_10min",
        "This represent the number of ads in the last full slot of 10 minutes",
        &["hostname"],
    )
});

pub static HISTORY: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "history",
        "Historical query counts per 10-minute slot over the last 24 hours",
        &["hostname", "timestamp", "field"],
    )
});

pub static BLOCKING_TIMER_SECONDS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "blocking_timer_seconds",
        "Seconds until Pi-hole blocking mode reverts when temporarily disabled",
        &["hostname"],
    )
});

pub static UPSTREAM_FORWARDED_QUERIES: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "upstream_forwarded_queries",
        "Total forwarded queries reported by upstream stats",
        &["hostname"],
    )
});

pub static UPSTREAM_TOTAL_QUERIES: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "upstream_total_queries",
        "Total queries reported by upstream stats",
        &["hostname"],
    )
});

pub static API_SUMMARY_TOOK_SECONDS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "api_summary_took_seconds",
        "Time Pi-hole took to generate the stats summary response",
        &["hostname"],
    )
});

pub static VERSION_INFO: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "version_info",
        "Pi-hole component version information (value is always 1)",
        &["hostname", "component", "version", "branch", "hash"],
    )
});

pub static FTL_UPTIME_SECONDS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ftl_uptime_seconds",
        "Pi-hole FTL process uptime in seconds",
        &["hostname"],
    )
});

pub static FTL_PID: Lazy<GaugeVec> =
    Lazy::new(|| register_metric("ftl_pid", "Pi-hole FTL process ID", &["hostname"]));

pub static FTL_MEM_PERCENT: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ftl_mem_percent",
        "Pi-hole FTL memory usage percent",
        &["hostname"],
    )
});

pub static FTL_CPU_PERCENT: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ftl_cpu_percent",
        "Pi-hole FTL CPU usage percent",
        &["hostname"],
    )
});

pub static FTL_QUERY_FREQUENCY: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ftl_query_frequency",
        "Pi-hole FTL query frequency",
        &["hostname"],
    )
});

pub static FTL_PRIVACY_LEVEL: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ftl_privacy_level",
        "Pi-hole FTL privacy level",
        &["hostname"],
    )
});

pub static FTL_DATABASE_GRAVITY: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ftl_database_gravity",
        "Gravity domains in FTL database stats",
        &["hostname"],
    )
});

pub static FTL_DATABASE_GROUPS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ftl_database_groups",
        "Groups in FTL database stats",
        &["hostname"],
    )
});

pub static FTL_DATABASE_LISTS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ftl_database_lists",
        "Lists in FTL database stats",
        &["hostname"],
    )
});

pub static FTL_DATABASE_CLIENTS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "ftl_database_clients",
        "Clients in FTL database stats",
        &["hostname"],
    )
});

pub static SYSTEM_UPTIME_SECONDS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_uptime_seconds",
        "Host system uptime in seconds",
        &["hostname"],
    )
});

pub static SYSTEM_RAM_TOTAL_KB: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_ram_total_kb",
        "Host RAM total in kilobytes",
        &["hostname"],
    )
});

pub static SYSTEM_RAM_FREE_KB: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_ram_free_kb",
        "Host RAM free in kilobytes",
        &["hostname"],
    )
});

pub static SYSTEM_RAM_USED_KB: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_ram_used_kb",
        "Host RAM used in kilobytes",
        &["hostname"],
    )
});

pub static SYSTEM_RAM_AVAILABLE_KB: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_ram_available_kb",
        "Host RAM available in kilobytes",
        &["hostname"],
    )
});

pub static SYSTEM_RAM_USED_PERCENT: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_ram_used_percent",
        "Host RAM used percent",
        &["hostname"],
    )
});

pub static SYSTEM_SWAP_TOTAL_KB: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_swap_total_kb",
        "Host swap total in kilobytes",
        &["hostname"],
    )
});

pub static SYSTEM_SWAP_FREE_KB: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_swap_free_kb",
        "Host swap free in kilobytes",
        &["hostname"],
    )
});

pub static SYSTEM_SWAP_USED_KB: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_swap_used_kb",
        "Host swap used in kilobytes",
        &["hostname"],
    )
});

pub static SYSTEM_SWAP_USED_PERCENT: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_swap_used_percent",
        "Host swap used percent",
        &["hostname"],
    )
});

pub static SYSTEM_CPU_PERCENT: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_cpu_percent",
        "Host CPU usage percent",
        &["hostname"],
    )
});

pub static SYSTEM_CPU_NPROCS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_cpu_nprocs",
        "Host CPU processor count",
        &["hostname"],
    )
});

pub static SYSTEM_LOAD: Lazy<GaugeVec> =
    Lazy::new(|| register_metric("system_load", "Host load average", &["hostname", "period"]));

pub static SYSTEM_FTL_MEM_PERCENT: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_ftl_mem_percent",
        "FTL memory usage percent from system info",
        &["hostname"],
    )
});

pub static SYSTEM_FTL_CPU_PERCENT: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "system_ftl_cpu_percent",
        "FTL CPU usage percent from system info",
        &["hostname"],
    )
});

pub static DATABASE_SIZE_BYTES: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "database_size_bytes",
        "Pi-hole query database size in bytes",
        &["hostname"],
    )
});

pub static DATABASE_QUERIES: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "database_queries",
        "Queries stored in the in-memory database",
        &["hostname"],
    )
});

pub static DATABASE_EARLIEST_TIMESTAMP: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "database_earliest_timestamp",
        "Earliest query timestamp in the in-memory database",
        &["hostname"],
    )
});

pub static DATABASE_QUERIES_DISK: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "database_queries_disk",
        "Queries stored on disk",
        &["hostname"],
    )
});

pub static DATABASE_EARLIEST_TIMESTAMP_DISK: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "database_earliest_timestamp_disk",
        "Earliest query timestamp on disk",
        &["hostname"],
    )
});

pub static CPU_TEMP: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "cpu_temp",
        "CPU temperature reported by Pi-hole sensors",
        &["hostname", "unit"],
    )
});

pub static CPU_TEMP_HOT_LIMIT: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "cpu_temp_hot_limit",
        "CPU temperature hot limit",
        &["hostname", "unit"],
    )
});

pub static SCRAPE_SUCCESS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "scrape_success",
        "Whether the last metrics scrape from Pi-hole succeeded (1 = yes, 0 = no)",
        &["hostname"],
    )
});

pub static SCRAPE_DURATION_SECONDS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "scrape_duration_seconds",
        "Duration of the last Pi-hole metrics scrape in seconds",
        &["hostname"],
    )
});

pub static LAST_SCRAPE_TIMESTAMP: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "last_scrape_timestamp",
        "Unix timestamp of the last successful metrics scrape",
        &["hostname"],
    )
});

fn register_metric(name: &str, help: &str, label_names: &[&str]) -> GaugeVec {
    let metric = GaugeVec::new(Opts::new(name, help).namespace("pihole"), label_names)
        .expect("valid metric options");

    REGISTRY
        .register(Box::new(metric.clone()))
        .expect("metric must register once");

    tracing::debug!("New Prometheus metric registered: {}", name);
    metric
}

macro_rules! force_metrics {
    ($($metric:expr),+ $(,)?) => {
        $(Lazy::force(&$metric);)+
    };
}

pub fn init() {
    force_metrics!(
        DOMAINS_BLOCKED,
        DNS_QUERIES_TODAY,
        ADS_BLOCKED_TODAY,
        ADS_PERCENTAGE_TODAY,
        UNIQUE_DOMAINS,
        QUERIES_FORWARDED,
        QUERIES_CACHED,
        CLIENTS_EVER_SEEN,
        UNIQUE_CLIENTS,
        REQUEST_RATE,
        DNS_QUERIES_ALL_TYPES,
        REPLY,
        TOP_QUERIES,
        TOP_ADS,
        TOP_SOURCES,
        FORWARD_DESTINATIONS,
        FORWARD_DESTINATIONS_RESPONSE_TIME,
        FORWARD_DESTINATIONS_RESPONSE_VARIANCE,
        QUERY_TYPES,
        STATUS,
        QUERY_STATUS,
        GRAVITY_LAST_UPDATE,
        GRAVITY_AGE_SECONDS,
        QUERIES_LAST_10MIN,
        ADS_LAST_10MIN,
        HISTORY,
        BLOCKING_TIMER_SECONDS,
        UPSTREAM_FORWARDED_QUERIES,
        UPSTREAM_TOTAL_QUERIES,
        API_SUMMARY_TOOK_SECONDS,
        VERSION_INFO,
        FTL_UPTIME_SECONDS,
        FTL_PID,
        FTL_MEM_PERCENT,
        FTL_CPU_PERCENT,
        FTL_QUERY_FREQUENCY,
        FTL_PRIVACY_LEVEL,
        FTL_DATABASE_GRAVITY,
        FTL_DATABASE_GROUPS,
        FTL_DATABASE_LISTS,
        FTL_DATABASE_CLIENTS,
        SYSTEM_UPTIME_SECONDS,
        SYSTEM_RAM_TOTAL_KB,
        SYSTEM_RAM_FREE_KB,
        SYSTEM_RAM_USED_KB,
        SYSTEM_RAM_AVAILABLE_KB,
        SYSTEM_RAM_USED_PERCENT,
        SYSTEM_SWAP_TOTAL_KB,
        SYSTEM_SWAP_FREE_KB,
        SYSTEM_SWAP_USED_KB,
        SYSTEM_SWAP_USED_PERCENT,
        SYSTEM_CPU_PERCENT,
        SYSTEM_CPU_NPROCS,
        SYSTEM_LOAD,
        SYSTEM_FTL_MEM_PERCENT,
        SYSTEM_FTL_CPU_PERCENT,
        DATABASE_SIZE_BYTES,
        DATABASE_QUERIES,
        DATABASE_EARLIEST_TIMESTAMP,
        DATABASE_QUERIES_DISK,
        DATABASE_EARLIEST_TIMESTAMP_DISK,
        CPU_TEMP,
        CPU_TEMP_HOT_LIMIT,
        SCRAPE_SUCCESS,
        SCRAPE_DURATION_SECONDS,
        LAST_SCRAPE_TIMESTAMP,
    );
    tracing::info!("prometheus metrics registered");
}

pub fn encode_metrics() -> Result<String, prometheus::Error> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8(buffer).expect("prometheus output is valid UTF-8"))
}
