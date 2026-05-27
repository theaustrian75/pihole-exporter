use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BlockingStatus {
    pub blocking: String,
    #[serde(default)]
    pub timer: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct Upstreams {
    pub upstreams: Vec<Upstream>,
    pub forwarded_queries: i64,
    pub total_queries: i64,
}

#[derive(Debug, Deserialize)]
pub struct Upstream {
    pub ip: String,
    pub name: String,
    #[serde(default = "default_upstream_port")]
    pub port: i64,
    pub count: i64,
    pub statistics: UpstreamStatistics,
}

fn default_upstream_port() -> i64 {
    -1
}

impl Upstream {
    pub fn port_label(&self) -> String {
        if self.port < 0 {
            String::new()
        } else {
            self.port.to_string()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpstreamStatistics {
    pub response: f64,
    pub variance: f64,
}

#[derive(Debug, Deserialize)]
pub struct TopDomains {
    pub domains: Vec<DomainCount>,
}

#[derive(Debug, Deserialize)]
pub struct DomainCount {
    pub domain: String,
    pub count: i64,
}

#[derive(Debug, Deserialize)]
pub struct TopClients {
    pub clients: Vec<PiHoleClient>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PiHoleClient {
    pub ip: String,
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Deserialize)]
pub struct StatsSummary {
    pub queries: QueryStats,
    pub clients: ClientStats,
    pub gravity: GravityStats,
    #[serde(default)]
    pub took: f64,
}

#[derive(Debug, Deserialize)]
pub struct QueryStats {
    pub total: i64,
    pub blocked: i64,
    pub percent_blocked: f64,
    pub unique_domains: i64,
    pub forwarded: i64,
    pub cached: i64,
    pub frequency: f64,
    #[serde(default)]
    pub types: HashMap<String, f64>,
    #[serde(default)]
    pub status: HashMap<String, i64>,
    pub replies: ReplyStats,
}

#[derive(Debug, Default, Deserialize)]
#[allow(non_snake_case)]
pub struct ReplyStats {
    #[serde(default)]
    pub UNKNOWN: i64,
    #[serde(default)]
    pub NODATA: i64,
    #[serde(default)]
    pub NXDOMAIN: i64,
    #[serde(default)]
    pub CNAME: i64,
    #[serde(default)]
    pub IP: i64,
    #[serde(default)]
    pub DOMAIN: i64,
    #[serde(default)]
    pub RRNAME: i64,
    #[serde(default)]
    pub SERVFAIL: i64,
    #[serde(default)]
    pub REFUSED: i64,
    #[serde(default)]
    pub NOTIMP: i64,
    #[serde(default)]
    pub OTHER: i64,
    #[serde(default)]
    pub DNSSEC: i64,
    #[serde(default)]
    pub NONE: i64,
    #[serde(default)]
    pub BLOB: i64,
}

#[derive(Debug, Deserialize)]
pub struct ClientStats {
    pub active: i64,
    pub total: i64,
}

#[derive(Debug, Deserialize)]
pub struct GravityStats {
    pub domains_being_blocked: i64,
    #[serde(default)]
    pub last_update: i64,
}

#[derive(Debug, Deserialize)]
pub struct HistoryResponse {
    pub history: Vec<HistorySlot>,
}

#[derive(Debug, Deserialize)]
pub struct HistorySlot {
    pub timestamp: i64,
    pub total: i64,
    pub cached: i64,
    pub blocked: i64,
    pub forwarded: i64,
}

impl StatsSummary {
    pub fn summary_line(&self) -> String {
        format!(
            "{} ads blocked / {} total DNS queries",
            self.queries.blocked, self.queries.total
        )
    }
}

pub fn merge_clients(clients1: &[PiHoleClient], clients2: &[PiHoleClient]) -> Vec<PiHoleClient> {
    let mut client_map: HashMap<String, PiHoleClient> = HashMap::new();

    for client in clients1.iter().chain(clients2.iter()) {
        client_map
            .entry(client.ip.clone())
            .and_modify(|existing| existing.count += client.count)
            .or_insert_with(|| client.clone());
    }

    client_map.into_values().collect()
}

pub fn normalize_status_label(status: &str) -> String {
    status.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_clients_combines_counts_by_ip() {
        let permitted = vec![PiHoleClient {
            ip: "192.168.1.10".to_string(),
            name: "host-a".to_string(),
            count: 5,
        }];
        let blocked = vec![PiHoleClient {
            ip: "192.168.1.10".to_string(),
            name: "host-a".to_string(),
            count: 3,
        }];

        let merged = merge_clients(&permitted, &blocked);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].count, 8);
    }

    #[test]
    fn normalize_status_label_lowercases() {
        assert_eq!(normalize_status_label("GRAVITY_CNAME"), "gravity_cname");
    }
}
