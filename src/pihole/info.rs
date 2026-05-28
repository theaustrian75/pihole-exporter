use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct VersionResponse {
    pub version: VersionInfo,
}

#[derive(Debug, Deserialize)]
pub struct VersionInfo {
    pub core: ComponentVersions,
    pub web: ComponentVersions,
    pub ftl: ComponentVersions,
    #[serde(default)]
    pub docker: Option<ComponentVersions>,
}

#[derive(Debug, Deserialize)]
pub struct ComponentVersions {
    #[serde(default, deserialize_with = "deserialize_version_entry")]
    pub local: Option<VersionDetail>,
    #[serde(default, deserialize_with = "deserialize_version_entry")]
    pub remote: Option<VersionDetail>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum VersionEntry {
    Detail(VersionDetail),
    VersionOnly(String),
}

fn deserialize_version_entry<'de, D>(deserializer: D) -> Result<Option<VersionDetail>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let entry = Option::<VersionEntry>::deserialize(deserializer)?;
    Ok(entry.map(version_entry_into_detail))
}

fn version_entry_into_detail(entry: VersionEntry) -> VersionDetail {
    match entry {
        VersionEntry::Detail(detail) => detail,
        VersionEntry::VersionOnly(version) => VersionDetail {
            version: Some(version),
            branch: None,
            hash: None,
        },
    }
}

#[derive(Debug, Deserialize)]
pub struct VersionDetail {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub hash: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FtlResponse {
    pub ftl: FtlInfo,
}

#[derive(Debug, Deserialize)]
pub struct FtlInfo {
    pub uptime: f64,
    pub pid: i64,
    #[serde(rename = "%mem")]
    pub mem_percent: f64,
    #[serde(rename = "%cpu")]
    pub cpu_percent: f64,
    pub query_frequency: f64,
    pub privacy_level: i64,
    #[serde(default)]
    pub database: Option<FtlDatabaseInfo>,
}

#[derive(Debug, Deserialize)]
pub struct FtlDatabaseInfo {
    pub gravity: i64,
    pub groups: i64,
    pub lists: i64,
    pub clients: i64,
}

#[derive(Debug, Deserialize)]
pub struct SystemResponse {
    pub system: SystemInfo,
}

#[derive(Debug, Deserialize)]
pub struct SystemInfo {
    pub uptime: f64,
    pub memory: MemoryInfo,
    pub cpu: CpuInfo,
    pub ftl: SystemFtlInfo,
}

#[derive(Debug, Deserialize)]
pub struct MemoryInfo {
    pub ram: RamInfo,
    pub swap: SwapInfo,
}

#[derive(Debug, Deserialize)]
pub struct RamInfo {
    pub total: f64,
    pub free: f64,
    pub used: f64,
    pub available: f64,
    #[serde(rename = "%used")]
    pub used_percent: f64,
}

#[derive(Debug, Deserialize)]
pub struct SwapInfo {
    pub total: f64,
    pub free: f64,
    pub used: f64,
    #[serde(rename = "%used")]
    pub used_percent: f64,
}

#[derive(Debug, Deserialize)]
pub struct CpuInfo {
    pub nprocs: i64,
    #[serde(rename = "%cpu")]
    pub cpu_percent: f64,
    pub load: LoadInfo,
}

#[derive(Debug, Deserialize)]
pub struct LoadInfo {
    pub raw: Vec<f64>,
    pub percent: Vec<f64>,
}

#[derive(Debug, Deserialize)]
pub struct SystemFtlInfo {
    #[serde(rename = "%mem")]
    pub mem_percent: f64,
    #[serde(rename = "%cpu")]
    pub cpu_percent: f64,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseInfo {
    pub size: i64,
    pub queries: i64,
    pub earliest_timestamp: f64,
    pub queries_disk: i64,
    pub earliest_timestamp_disk: f64,
}

#[derive(Debug, Deserialize)]
pub struct SensorsResponse {
    pub sensors: SensorsInfo,
}

#[derive(Debug, Deserialize)]
pub struct SensorsInfo {
    #[serde(default)]
    pub cpu_temp: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
    #[serde(default)]
    pub hot_limit: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_docker_version_strings() {
        let json = r#"{
            "version": {
                "core": { "local": { "version": "v6.0" }, "remote": { "version": "v6.1" } },
                "web": { "local": { "version": "v6.0" }, "remote": { "version": "v6.1" } },
                "ftl": { "local": { "version": "v6.0" }, "remote": { "version": "v6.1" } },
                "docker": { "local": "2026.05.0", "remote": "2026.05.0" }
            }
        }"#;

        let parsed: VersionResponse =
            serde_json::from_str(json).expect("version json should parse");
        let docker = parsed.version.docker.expect("docker section");
        assert_eq!(
            docker.local.and_then(|d| d.version),
            Some("2026.05.0".to_string())
        );
        assert_eq!(
            docker.remote.and_then(|d| d.version),
            Some("2026.05.0".to_string())
        );
    }
}
