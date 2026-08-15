/// Redfish (DMTF DSP0266) BMC management REST API client.
/// Uses reqwest with optional TLS verification skip for self-signed BMC certs.
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RedfishError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("HTTP error {0}: {1}")]
    Http(u16, String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Request error: {0}")]
    Request(String),
}

impl Serialize for RedfishError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedfishConfig {
    pub host: String,
    /// Default 443 (HTTPS) or 8080 (HTTP dev)
    pub port: u16,
    pub username: String,
    pub password: String,
    pub use_tls: bool,
    /// Accept self-signed BMC certificates
    pub verify_tls: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedfishSession {
    pub id: String,
    pub host: String,
    pub service_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedfishSystem {
    pub id: String,
    pub name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub power_state: Option<String>,
    pub bios_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum RedfishPowerAction {
    On,
    ForceOff,
    GracefulShutdown,
    GracefulRestart,
    ForceRestart,
    Nmi,
}

pub struct RedfishState {
    sessions: Mutex<HashMap<String, (RedfishConfig, reqwest::Client, String)>>,
}

impl RedfishState {
    pub fn new() -> Self {
        Self { sessions: Mutex::new(HashMap::new()) }
    }
}

fn build_client(verify_tls: bool) -> Result<reqwest::Client, RedfishError> {
    let mut b = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15));
    if !verify_tls {
        b = b.danger_accept_invalid_certs(true);
    }
    b.build().map_err(|e| RedfishError::Request(e.to_string()))
}

fn base_url(cfg: &RedfishConfig) -> String {
    let scheme = if cfg.use_tls { "https" } else { "http" };
    format!("{scheme}://{}:{}/redfish/v1", cfg.host, cfg.port)
}

#[tauri::command]
pub async fn redfish_connect(
    config: RedfishConfig,
    state: tauri::State<'_, RedfishState>,
) -> Result<String, RedfishError> {
    let client = build_client(config.verify_tls)?;
    let url = base_url(&config);

    let resp = client
        .get(&url)
        .header(ACCEPT, "application/json")
        .basic_auth(&config.username, Some(&config.password))
        .send()
        .await
        .map_err(|e| RedfishError::Request(e.to_string()))?;

    let status = resp.status().as_u16();
    if status == 401 { return Err(RedfishError::AuthFailed); }
    if status >= 400 {
        let body = resp.text().await.unwrap_or_default();
        return Err(RedfishError::Http(status, body));
    }

    let id = Uuid::new_v4().to_string();
    let session = (config, client, url);
    state.sessions.lock().unwrap().insert(id.clone(), session);
    Ok(id)
}

#[tauri::command]
pub async fn redfish_get_systems(
    id: String,
    state: tauri::State<'_, RedfishState>,
) -> Result<Vec<RedfishSystem>, RedfishError> {
    let (cfg, client, base) = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| RedfishError::NotFound(id.clone()))?;

    let systems_url = format!("{base}/Systems");
    let members: Value = client
        .get(&systems_url)
        .basic_auth(&cfg.username, Some(&cfg.password))
        .header(ACCEPT, "application/json")
        .send().await.map_err(|e| RedfishError::Request(e.to_string()))?
        .json().await.map_err(|e| RedfishError::Request(e.to_string()))?;

    let mut out = Vec::new();
    if let Some(arr) = members["Members"].as_array() {
        for member in arr {
            if let Some(href) = member["@odata.id"].as_str() {
                let sys_url = format!("{}:{}{}", 
                    if cfg.use_tls {"https"} else {"http"}, 
                    format!("//{}:{}", cfg.host, cfg.port), 
                    href);
                if let Ok(resp) = client.get(&sys_url)
                    .basic_auth(&cfg.username, Some(&cfg.password))
                    .header(ACCEPT, "application/json")
                    .send().await {
                    if let Ok(sys) = resp.json::<Value>().await {
                        out.push(RedfishSystem {
                            id: sys["Id"].as_str().unwrap_or("").to_string(),
                            name: sys["Name"].as_str().unwrap_or("").to_string(),
                            manufacturer: sys["Manufacturer"].as_str().map(str::to_string),
                            model: sys["Model"].as_str().map(str::to_string),
                            serial: sys["SerialNumber"].as_str().map(str::to_string),
                            power_state: sys["PowerState"].as_str().map(str::to_string),
                            bios_version: sys["BiosVersion"].as_str().map(str::to_string),
                        });
                    }
                }
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn redfish_power_control(
    id: String,
    system_id: String,
    action: RedfishPowerAction,
    state: tauri::State<'_, RedfishState>,
) -> Result<(), RedfishError> {
    let (cfg, client, base) = state.sessions.lock().unwrap()
        .get(&id).cloned().ok_or_else(|| RedfishError::NotFound(id.clone()))?;

    let action_url = format!("{base}/Systems/{system_id}/Actions/ComputerSystem.Reset");
    let body = serde_json::json!({ "ResetType": format!("{action:?}") });

    let resp = client
        .post(&action_url)
        .basic_auth(&cfg.username, Some(&cfg.password))
        .header(CONTENT_TYPE, "application/json")
        .json(&body)
        .send().await
        .map_err(|e| RedfishError::Request(e.to_string()))?;

    let status = resp.status().as_u16();
    if status >= 400 {
        let body = resp.text().await.unwrap_or_default();
        return Err(RedfishError::Http(status, body));
    }
    Ok(())
}

#[tauri::command]
pub fn redfish_disconnect(id: String, state: tauri::State<'_, RedfishState>) -> Result<(), RedfishError> {
    state.sessions.lock().unwrap().remove(&id).ok_or_else(|| RedfishError::NotFound(id))?;
    Ok(())
}

#[tauri::command]
pub fn redfish_list(state: tauri::State<'_, RedfishState>) -> Vec<RedfishSession> {
    state.sessions.lock().unwrap().iter().map(|(id, (cfg, _, base))| RedfishSession {
        id: id.clone(), host: cfg.host.clone(), service_root: base.clone(),
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config(use_tls: bool) -> RedfishConfig {
        RedfishConfig {
            host: "10.0.0.5".into(),
            port: if use_tls { 443 } else { 8080 },
            username: "admin".into(),
            password: "hunter2".into(),
            use_tls,
            verify_tls: false,
        }
    }

    #[test]
    fn base_url_uses_https_and_the_configured_port_when_tls_is_enabled() {
        assert_eq!(base_url(&sample_config(true)), "https://10.0.0.5:443/redfish/v1");
    }

    #[test]
    fn base_url_uses_http_when_tls_is_disabled() {
        assert_eq!(base_url(&sample_config(false)), "http://10.0.0.5:8080/redfish/v1");
    }

    #[test]
    fn build_client_succeeds_with_and_without_tls_verification() {
        assert!(build_client(true).is_ok());
        assert!(build_client(false).is_ok());
    }
}
