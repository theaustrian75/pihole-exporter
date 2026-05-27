use once_cell::sync::Lazy;
use prometheus::{Encoder, GaugeVec, Opts, Registry, TextEncoder};

pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

pub static DOMAINS_BLOCKED: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric("domains_being_blocked", "This represent the number of domains being blocked", &["hostname"])
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
        &["hostname", "destination", "destination_name"],
    )
});

pub static FORWARD_DESTINATIONS_RESPONSE_TIME: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "forward_destinations_responsetime",
        "This represent the seconds a forward destinations took to process a requests made by Pi-hole",
        &["hostname", "destination", "destination_name"],
    )
});

pub static FORWARD_DESTINATIONS_RESPONSE_VARIANCE: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "forward_destinations_responsevariance",
        "This represent the variants in response time a forward destinations took to process a requests made by Pi-hole",
        &["hostname", "destination", "destination_name"],
    )
});

pub static QUERY_TYPES: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric(
        "querytypes",
        "This represent the number of queries made by Pi-hole by type",
        &["hostname", "type"],
    )
});

pub static STATUS: Lazy<GaugeVec> = Lazy::new(|| {
    register_metric("status", "This if Pi-hole is enabled", &["hostname"])
});

fn register_metric(name: &str, help: &str, label_names: &[&str]) -> GaugeVec {
    let metric = GaugeVec::new(
        Opts::new(name, help).namespace("pihole"),
        label_names,
    )
    .expect("valid metric options");

    REGISTRY
        .register(Box::new(metric.clone()))
        .expect("metric must register once");

    tracing::debug!("New Prometheus metric registered: {}", name);
    metric
}

pub fn init() {
    Lazy::force(&DOMAINS_BLOCKED);
    Lazy::force(&DNS_QUERIES_TODAY);
    Lazy::force(&ADS_BLOCKED_TODAY);
    Lazy::force(&ADS_PERCENTAGE_TODAY);
    Lazy::force(&UNIQUE_DOMAINS);
    Lazy::force(&QUERIES_FORWARDED);
    Lazy::force(&QUERIES_CACHED);
    Lazy::force(&CLIENTS_EVER_SEEN);
    Lazy::force(&UNIQUE_CLIENTS);
    Lazy::force(&REQUEST_RATE);
    Lazy::force(&DNS_QUERIES_ALL_TYPES);
    Lazy::force(&REPLY);
    Lazy::force(&TOP_QUERIES);
    Lazy::force(&TOP_ADS);
    Lazy::force(&TOP_SOURCES);
    Lazy::force(&FORWARD_DESTINATIONS);
    Lazy::force(&FORWARD_DESTINATIONS_RESPONSE_TIME);
    Lazy::force(&FORWARD_DESTINATIONS_RESPONSE_VARIANCE);
    Lazy::force(&QUERY_TYPES);
    Lazy::force(&STATUS);
}

pub fn encode_metrics() -> Result<String, prometheus::Error> {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8(buffer).expect("prometheus output is valid UTF-8"))
}
