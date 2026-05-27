# Pi-hole Prometheus Exporter (Rust)

A Prometheus exporter for [Pi-hole](https://pi-hole.net/)'s ad blocker. This is a Rust rewrite of [eko/pihole-exporter](https://github.com/eko/pihole-exporter), preserving the same metrics, configuration options, and HTTP endpoints for drop-in compatibility with existing Prometheus and Grafana setups.

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.75 or newer

## Installation

### From source

```bash
git clone https://github.com/eko/pihole-exporter.git
cd pihole-exporter
cargo build --release
```

The binary is written to `target/release/pihole-exporter`.

### Using Docker

```bash
docker build -t pihole-exporter .
docker run \
  -e 'PIHOLE_HOSTNAME=192.168.1.2' \
  -e 'PIHOLE_PASSWORD=mypassword' \
  -e 'PORT=9617' \
  -p 9617:9617 \
  pihole-exporter:latest
```

Or use Pi-hole's `WEBPASSWORD` as an API token:

```bash
API_TOKEN=$(awk -F= -v key="WEBPASSWORD" '$1==key {print $2}' /etc/pihole/setupVars.conf)
docker run \
  -e 'PIHOLE_HOSTNAME=192.168.1.2' \
  -e "PIHOLE_PASSWORD=$API_TOKEN" \
  -e 'PORT=9617' \
  -p 9617:9617 \
  pihole-exporter:latest
```

## Usage

Using a password:

```bash
./pihole-exporter --pihole-hostname 192.168.1.10 --pihole-password azerty
```

Or with an API token:

```bash
API_TOKEN=$(awk -F= -v key="WEBPASSWORD" '$1==key {print $2}' /etc/pihole/setupVars.conf)
./pihole-exporter --pihole-hostname 192.168.1.10 --pihole-password "$API_TOKEN"
```

Configure Prometheus to scrape the exporter:

```yaml
scrape_configs:
  - job_name: "pihole"
    static_configs:
      - targets: ["localhost:9617"]
```

## Configuration

All options can be set via CLI flags or environment variables.

| Flag | Environment variable | Default | Description |
|------|---------------------|---------|-------------|
| `--pihole-hostname` | `PIHOLE_HOSTNAME` | `127.0.0.1` | Pi-hole hostname(s), comma-separated |
| `--pihole-password` | `PIHOLE_PASSWORD` | | Pi-hole password or API token(s) |
| `--pihole-protocol` | `PIHOLE_PROTOCOL` | `http` | `http` or `https` |
| `--pihole-port` | `PIHOLE_PORT` | `80` | Pi-hole port(s) |
| `--bind-addr` | `BIND_ADDR` | `0.0.0.0` | Exporter listen address |
| `--port` | `PORT` | `9617` | Exporter listen port |
| `--timeout` | `TIMEOUT` | `5s` | Pi-hole request timeout |
| `--skip-tls-verification` | `SKIP_TLS_VERIFICATION` | `false` | Skip TLS certificate verification |
| `--debug` | `DEBUG` | `false` | Enable verbose logging |

A single exporter instance can monitor multiple Pi-hole hosts by providing comma-separated values. When port, protocol, and password are the same for all instances, specify them once.

## HTTP endpoints

| Path | Description |
|------|-------------|
| `/` | Service index |
| `/metrics` | Prometheus metrics |
| `/healthz` | Liveness probe (returns `ok`) |
| `/readiness` | Readiness probe |
| `/liveness` | Liveness probe |

## Prometheus metrics

All metrics use the `pihole_` namespace and a `hostname` label.

| Metric | Description |
|--------|-------------|
| `pihole_domains_being_blocked` | Domains being blocked |
| `pihole_dns_queries_today` | DNS queries today |
| `pihole_ads_blocked_today` | Ads blocked today |
| `pihole_ads_percentage_today` | Ads blocked percentage today |
| `pihole_unique_domains` | Unique domains seen |
| `pihole_queries_forwarded` | Queries forwarded |
| `pihole_queries_cached` | Queries cached |
| `pihole_clients_ever_seen` | Clients ever seen |
| `pihole_unique_clients` | Unique clients in the last 24h |
| `pihole_dns_queries_all_types` | DNS queries for all types |
| `pihole_request_rate` | Requests per second |
| `pihole_reply` | Replies by type |
| `pihole_top_queries` | Top permitted domains |
| `pihole_top_ads` | Top blocked domains |
| `pihole_top_sources` | Top client sources |
| `pihole_forward_destinations` | Forward destinations |
| `pihole_forward_destinations_responsetime` | Forward destination response time |
| `pihole_forward_destinations_responsevariance` | Forward destination response variance |
| `pihole_querytypes` | Queries by DNS type |
| `pihole_status` | Pi-hole blocking enabled (1) or disabled (0) |

### v6 API metrics

| Metric | Description |
|--------|-------------|
| `pihole_query_status` | Queries by processing status (gravity, forwarded, cache, etc.) |
| `pihole_gravity_last_update` | Unix timestamp of last gravity update |
| `pihole_gravity_age_seconds` | Seconds since last gravity update |
| `pihole_queries_last_10min` | Queries in the latest 10-minute history slot |
| `pihole_ads_last_10min` | Blocked queries in the latest 10-minute slot |
| `pihole_history` | 24h history per 10-minute slot (`field=total\|blocked\|cached\|forwarded`) |
| `pihole_blocking_timer_seconds` | Seconds until blocking mode reverts |
| `pihole_upstream_forwarded_queries` | Forwarded queries from upstream stats |
| `pihole_upstream_total_queries` | Total queries from upstream stats |
| `pihole_api_summary_took_seconds` | Pi-hole stats summary generation time |
| `pihole_version_info` | Component versions (`component=core_local`, etc.) |
| `pihole_ftl_*` | FTL uptime, CPU/memory, privacy level, database counts |
| `pihole_system_*` | Host uptime, RAM, swap, CPU, load averages |
| `pihole_database_*` | Query database size and record counts |
| `pihole_cpu_temp` | CPU temperature from Pi-hole sensors |
| `pihole_scrape_success` | Last scrape succeeded (1/0) |
| `pihole_scrape_duration_seconds` | Last scrape duration |
| `pihole_last_scrape_timestamp` | Unix timestamp of last successful scrape |

## License

MIT — see [LICENSE](LICENSE).
