use std::time::Duration;

use clap::Parser;
use thiserror::Error;

#[derive(Debug, Clone, Parser)]
#[command(name = "pihole-exporter", about = "Prometheus exporter for Pi-hole")]
pub struct Cli {
    /// Protocol(s) used to reach Pi-hole (http or https). Comma-separated for multiple hosts.
    #[arg(long = "pihole_protocol", env = "PIHOLE_PROTOCOL", value_delimiter = ',', default_value = "http")]
    pub pihole_protocol: Vec<String>,

    /// Hostname(s) where Pi-hole is installed. Comma-separated for multiple hosts.
    #[arg(long = "pihole_hostname", env = "PIHOLE_HOSTNAME", value_delimiter = ',', default_value = "127.0.0.1")]
    pub pihole_hostname: Vec<String>,

    /// Port(s) used by Pi-hole. Comma-separated for multiple hosts.
    #[arg(long = "pihole_port", env = "PIHOLE_PORT", value_delimiter = ',', default_value = "80")]
    pub pihole_port: Vec<u16>,

    /// Pi-hole web password or API token. Comma-separated for multiple hosts.
    #[arg(long = "pihole_password", env = "PIHOLE_PASSWORD", value_delimiter = ',', default_value = "")]
    pub pihole_password: Vec<String>,

    /// Address the exporter listens on.
    #[arg(long = "bind_addr", env = "BIND_ADDR", default_value = "0.0.0.0")]
    pub bind_addr: String,

    /// Port the exporter listens on.
    #[arg(long = "port", env = "PORT", default_value = "9617")]
    pub port: u16,

    /// Timeout for connecting to and retrieving data from Pi-hole.
    #[arg(long = "timeout", env = "TIMEOUT", value_parser = humantime::parse_duration, default_value = "5s")]
    pub timeout: Duration,

    /// Skip TLS certificate verification (do not use on untrusted networks).
    #[arg(long = "skip_tls_verification", env = "SKIP_TLS_VERIFICATION", default_value_t = false)]
    pub skip_tls_verification: bool,

    /// Enable debug (verbose) output.
    #[arg(long = "debug", env = "DEBUG", default_value_t = false)]
    pub debug: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvConfig {
    pub pihole_protocol: Vec<String>,
    pub pihole_hostname: Vec<String>,
    pub pihole_port: Vec<u16>,
    pub pihole_password: Vec<String>,
    pub bind_addr: String,
    pub port: u16,
    pub timeout: Duration,
    pub skip_tls_verification: bool,
    pub debug: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    pub pihole_protocol: String,
    pub pihole_hostname: String,
    pub pihole_port: u16,
    pub pihole_password: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid protocol {0}: must be http or https")]
    InvalidProtocol(String),
    #[error("wrong number of ports: must be empty, single value or one per host")]
    WrongPortCount,
    #[error("wrong number of PIHoleProtocol: must be empty, single value or one per host")]
    WrongProtocolCount,
    #[error("wrong number of PIHolePassword: must be empty, single value or one per host")]
    WrongPasswordCount,
    #[error("invalid base URL {0}: hostname and port may be duplicated (use either host:port or separate PIHOLE_PORT)")]
    InvalidBaseUrl(String),
}

impl From<Cli> for EnvConfig {
    fn from(cli: Cli) -> Self {
        Self {
            pihole_protocol: cli.pihole_protocol,
            pihole_hostname: cli.pihole_hostname,
            pihole_port: cli.pihole_port,
            pihole_password: cli.pihole_password,
            bind_addr: cli.bind_addr,
            port: cli.port,
            timeout: cli.timeout,
            skip_tls_verification: cli.skip_tls_verification,
            debug: cli.debug,
        }
    }
}

impl EnvConfig {
    pub fn load() -> Result<(Self, Vec<ClientConfig>), ConfigError> {
        let cli = Cli::parse();
        let env = EnvConfig::from(cli);
        env.log_debug();
        let clients = env.split()?;
        Ok((env, clients))
    }

    fn log_debug(&self) {
        tracing::debug!("------------------------------------");
        tracing::debug!("-  Pi-hole exporter configuration  -");
        tracing::debug!("------------------------------------");
        tracing::debug!(pihole_protocol = ?self.pihole_protocol);
        tracing::debug!(pihole_hostname = ?self.pihole_hostname);
        tracing::debug!(pihole_port = ?self.pihole_port);
        if !self.pihole_password.is_empty() {
            tracing::debug!("Pi-hole Authentication Method: PIHolePassword");
        }
        tracing::debug!(bind_addr = %self.bind_addr);
        tracing::debug!(port = %self.port);
        tracing::debug!(timeout = ?self.timeout);
        tracing::debug!(skip_tls_verification = %self.skip_tls_verification);
        tracing::debug!(debug = %self.debug);
        tracing::debug!("------------------------------------");
    }

    pub fn split(&self) -> Result<Vec<ClientConfig>, ConfigError> {
        let hosts_count = self.pihole_hostname.len();
        let mut result = Vec::with_capacity(hosts_count);

        for (i, hostname) in self.pihole_hostname.iter().enumerate() {
            let mut config = ClientConfig {
                pihole_protocol: String::new(),
                pihole_hostname: hostname.trim().to_string(),
                pihole_port: 0,
                pihole_password: String::new(),
            };

            match self.pihole_port.len() {
                0 => {}
                1 => config.pihole_port = self.pihole_port[0],
                n if n == hosts_count => config.pihole_port = self.pihole_port[i],
                _ => return Err(ConfigError::WrongPortCount),
            }

            match extract_string_config(&self.pihole_protocol, i, hosts_count) {
                ExtractResult::Value(v) => config.pihole_protocol = v,
                ExtractResult::Empty => {}
                ExtractResult::Invalid => return Err(ConfigError::WrongProtocolCount),
            }

            match extract_string_config(&self.pihole_password, i, hosts_count) {
                ExtractResult::Value(v) => config.pihole_password = v,
                ExtractResult::Empty => {}
                ExtractResult::Invalid => return Err(ConfigError::WrongPasswordCount),
            }

            if config.pihole_protocol.is_empty() {
                config.pihole_protocol = "http".to_string();
            }

            let (host, port) = parse_hostname_port(&config.pihole_hostname, config.pihole_port);
            config.pihole_hostname = host;
            config.pihole_port = port;

            config.validate()?;
            result.push(config);
        }

        Ok(result)
    }
}

impl ClientConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.pihole_protocol != "http" && self.pihole_protocol != "https" {
            return Err(ConfigError::InvalidProtocol(self.pihole_protocol.clone()));
        }

        let url = self.base_url();
        reqwest::Url::parse(&url).map_err(|_| ConfigError::InvalidBaseUrl(url))?;

        Ok(())
    }

    pub fn base_url(&self) -> String {
        format!(
            "{}://{}:{}",
            self.pihole_protocol, self.pihole_hostname, self.pihole_port
        )
    }
}

/// Split `host:port` or `[ipv6]:port` embedded in a hostname value.
fn parse_hostname_port(hostname: &str, configured_port: u16) -> (String, u16) {
    let hostname = hostname.trim();

    if let Some(rest) = hostname.strip_prefix('[') {
        if let Some((ipv6, after)) = rest.split_once(']') {
            let host = format!("[{ipv6}]");
            if let Some(port_str) = after.strip_prefix(':') {
                if let Ok(port) = port_str.parse::<u16>() {
                    return (host, port);
                }
            }
            return (host, configured_port);
        }
    }

    if let Some((host, port_str)) = hostname.rsplit_once(':') {
        if !host.is_empty() && !host.contains(':') {
            if let Ok(port) = port_str.parse::<u16>() {
                return (host.to_string(), port);
            }
        }
    }

    (hostname.to_string(), configured_port)
}

enum ExtractResult {
    Value(String),
    Empty,
    Invalid,
}

fn extract_string_config(data: &[String], idx: usize, hosts_count: usize) -> ExtractResult {
    match data.len() {
        1 => {
            let v = data[0].trim();
            if !v.is_empty() {
                ExtractResult::Value(v.to_string())
            } else {
                ExtractResult::Empty
            }
        }
        n if n == hosts_count => {
            let v = data[idx].trim();
            if !v.is_empty() {
                ExtractResult::Value(v.to_string())
            } else {
                ExtractResult::Empty
            }
        }
        0 => ExtractResult::Empty,
        _ => ExtractResult::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_env() -> EnvConfig {
        EnvConfig {
            pihole_protocol: vec!["http".to_string()],
            pihole_hostname: vec!["127.0.0.1".to_string()],
            pihole_port: vec![80],
            pihole_password: vec![],
            bind_addr: "0.0.0.0".to_string(),
            port: 9617,
            timeout: Duration::from_secs(5),
            skip_tls_verification: false,
            debug: false,
        }
    }

    #[test]
    fn split_default() {
        let clients = default_env().split().unwrap();
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].pihole_hostname, "127.0.0.1");
        assert_eq!(clients[0].pihole_protocol, "http");
        assert_eq!(clients[0].pihole_port, 80);
        assert!(clients[0].pihole_password.is_empty());
    }

    #[test]
    fn split_multiple_hosts_same_config() {
        let env = EnvConfig {
            pihole_hostname: vec![
                "127.0.0.1".to_string(),
                "127.0.0.2".to_string(),
                "127.0.0.3".to_string(),
            ],
            pihole_port: vec![8080],
            ..default_env()
        };

        let clients = env.split().unwrap();
        assert_eq!(clients.len(), 3);
        for (client, host) in clients.iter().zip(["127.0.0.1", "127.0.0.2", "127.0.0.3"]) {
            assert_eq!(client.pihole_hostname, host);
            assert_eq!(client.pihole_protocol, "http");
            assert_eq!(client.pihole_port, 8080);
            assert!(client.pihole_password.is_empty());
        }
    }

    #[test]
    fn split_multiple_hosts_multiple_configs() {
        let env = EnvConfig {
            pihole_hostname: vec![
                "127.0.0.1".to_string(),
                "127.0.0.2".to_string(),
                "127.0.0.3".to_string(),
            ],
            pihole_password: vec!["".to_string(), "password2".to_string(), "".to_string()],
            pihole_port: vec![8081, 8082, 8083],
            ..default_env()
        };

        let clients = env.split().unwrap();
        assert_eq!(clients.len(), 3);
        assert_eq!(clients[0].pihole_port, 8081);
        assert_eq!(clients[1].pihole_port, 8082);
        assert_eq!(clients[1].pihole_password, "password2");
        assert_eq!(clients[2].pihole_port, 8083);
    }

    #[test]
    fn split_hostname_with_embedded_port() {
        let env = EnvConfig {
            pihole_hostname: vec!["pihole4.example.com:8443".to_string()],
            pihole_port: vec![8443],
            pihole_protocol: vec!["https".to_string()],
            ..default_env()
        };

        let clients = env.split().unwrap();
        assert_eq!(clients[0].pihole_hostname, "pihole4.example.com");
        assert_eq!(clients[0].pihole_port, 8443);
        assert_eq!(clients[0].base_url(), "https://pihole4.example.com:8443");
    }

    #[test]
    fn validate_protocol() {
        let mut client = ClientConfig {
            pihole_protocol: "ftp".to_string(),
            pihole_hostname: "127.0.0.1".to_string(),
            pihole_port: 80,
            pihole_password: String::new(),
        };
        assert!(matches!(
            client.validate(),
            Err(ConfigError::InvalidProtocol(_))
        ));

        client.pihole_protocol = "https".to_string();
        assert!(client.validate().is_ok());
    }
}
