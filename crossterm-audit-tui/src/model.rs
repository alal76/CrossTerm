//! Read-only mirror of the JSON schema `network-explore-cli`/
//! `run_explore_and_dump` write (see `src-tauri/src/network/mod.rs`'s
//! `ExploreDump`/`ExploreResult`/`OpenPort`/`MdnsRecord`/`TlsCertInfo` and
//! `docs/network-explore-cli.md`'s schema table). Deliberately duplicated
//! here rather than depending on `app_lib` — this crate's whole point is to
//! have no dependency on the Tauri-app package's (much heavier) build graph.
//! Only `Deserialize` is derived: this crate reads scan dumps, it never
//! produces them.

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct ExploreDump {
    pub cidr: String,
    #[serde(default)]
    pub bound_interface: Option<String>,
    #[serde(default)]
    pub dns_servers_used: Vec<String>,
    #[serde(default)]
    pub ports_scanned: Vec<u16>,
    pub host_count: usize,
    pub results: Vec<ExploreResult>,
    #[serde(default)]
    pub unmerged_mdns: HashMap<String, Vec<MdnsRecord>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExploreResult {
    pub ip: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub mac_address: Option<String>,
    #[serde(default)]
    pub mac_vendor: Option<String>,
    #[serde(default)]
    pub open_ports: Vec<OpenPort>,
    #[serde(default)]
    pub os_guess: Option<String>,
    #[serde(default)]
    pub suggested_session_type: Option<String>,
    #[serde(default)]
    pub candidate_session_types: Vec<String>,
    #[serde(default)]
    pub ttl: Option<u8>,
    #[serde(default)]
    pub mdns: Vec<MdnsRecord>,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpenPort {
    pub port: u16,
    pub service_name: String,
    pub protocol: String,
    #[serde(default)]
    pub banner: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub http_title: Option<String>,
    #[serde(default)]
    pub tls: Option<TlsCertInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TlsCertInfo {
    #[serde(default)]
    pub subject_cn: Option<String>,
    #[serde(default)]
    pub subject_org: Option<String>,
    #[serde(default)]
    pub issuer_org: Option<String>,
    #[serde(default)]
    pub san: Vec<String>,
    #[serde(default)]
    pub not_after: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MdnsRecord {
    pub service_type: String,
    pub instance_name: String,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub txt: HashMap<String, String>,
}

impl ExploreResult {
    /// Compact "port/service, port/service" summary for the browser table.
    pub fn ports_summary(&self) -> String {
        self.open_ports
            .iter()
            .map(|p| format!("{}/{}", p.port, p.service_name))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_minimal_dump() {
        let json = r#"{
            "cidr": "127.0.0.1/32",
            "host_count": 1,
            "results": [{
                "ip": "127.0.0.1",
                "hostname": "localhost",
                "open_ports": [{"port": 22, "service_name": "ssh", "protocol": "tcp"}],
                "os_guess": "Linux/macOS/BSD-like (TTL 64)",
                "ttl": 64
            }]
        }"#;
        let dump: ExploreDump = serde_json::from_str(json).unwrap();
        assert_eq!(dump.host_count, 1);
        assert_eq!(dump.results.len(), 1);
        let host = &dump.results[0];
        assert_eq!(host.ip, "127.0.0.1");
        assert_eq!(host.hostname.as_deref(), Some("localhost"));
        assert_eq!(host.ports_summary(), "22/ssh");
    }

    #[test]
    fn tolerates_missing_optional_fields() {
        // Every field beyond ip/host_count/results/etc. that this crate
        // doesn't itself write must round-trip through #[serde(default)] -
        // real dumps from older/newer network-explore-cli versions won't
        // always populate every optional field.
        let json = r#"{"cidr": "10.0.0.0/24", "host_count": 0, "results": []}"#;
        let dump: ExploreDump = serde_json::from_str(json).unwrap();
        assert_eq!(dump.host_count, 0);
        assert!(dump.results.is_empty());
        assert!(dump.bound_interface.is_none());
        assert!(dump.dns_servers_used.is_empty());
    }

    #[test]
    fn parses_open_port_without_optional_enrichment_fields() {
        let json = r#"{"port": 80, "service_name": "http", "protocol": "tcp"}"#;
        let port: OpenPort = serde_json::from_str(json).unwrap();
        assert_eq!(port.port, 80);
        assert!(port.banner.is_none());
        assert!(port.tls.is_none());
    }
}
