use serde::{Deserialize, Serialize};
use tauri::Emitter;

use super::CloudError;

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcpConfig {
    pub name: String,
    pub project: String,
    pub region: String,
    pub zone: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcpInstance {
    pub id: String,
    pub name: String,
    pub zone: String,
    pub machine_type: String,
    pub status: String,
    pub internal_ip: Option<String>,
    pub external_ip: Option<String>,
    pub network: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcsBucket {
    pub name: String,
    pub location: String,
    pub storage_class: String,
    pub time_created: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcsObject {
    pub name: String,
    pub size: u64,
    pub content_type: String,
    pub time_created: String,
    pub updated: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// List GCP config names from `gcloud config configurations list`.
pub(crate) async fn list_config_names() -> Result<Vec<String>, CloudError> {
    let output = tokio::process::Command::new("gcloud")
        .args([
            "config",
            "configurations",
            "list",
            "--format",
            "json",
        ])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(format!("gcloud config list: {e}")))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let configs: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout)
            .map_err(|e| CloudError::Parse(e.to_string()))?;

    let names = configs
        .iter()
        .filter_map(|v| v.get("name").and_then(|v| v.as_str()).map(String::from))
        .collect();

    Ok(names)
}

/// Parse GCP configs from `gcloud config configurations list --format json`.
fn parse_configs(json: &str) -> Result<Vec<GcpConfig>, CloudError> {
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| CloudError::Parse(e.to_string()))?;

    let configs = arr
        .iter()
        .map(|v| {
            let properties = v.get("properties").cloned().unwrap_or_default();
            let core = properties.get("core").cloned().unwrap_or_default();
            let compute = properties.get("compute").cloned().unwrap_or_default();

            GcpConfig {
                name: v
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                project: core
                    .get("project")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                region: compute
                    .get("region")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                zone: compute
                    .get("zone")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                is_active: v
                    .get("is_active")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            }
        })
        .collect();

    Ok(configs)
}

/// Parse GCP instances from `gcloud compute instances list --format json`.
fn parse_instances(json: &str) -> Result<Vec<GcpInstance>, CloudError> {
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| CloudError::Parse(e.to_string()))?;

    let instances = arr
        .iter()
        .map(|v| {
            // Zone is a full URL like "projects/my-project/zones/us-central1-a"
            let zone_full = v
                .get("zone")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let zone = zone_full
                .rsplit('/')
                .next()
                .unwrap_or(zone_full)
                .to_string();

            // Machine type is also a full URL
            let mt_full = v
                .get("machineType")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let machine_type = mt_full
                .rsplit('/')
                .next()
                .unwrap_or(mt_full)
                .to_string();

            // Network interfaces
            let net_interfaces = v
                .get("networkInterfaces")
                .and_then(|v| v.as_array());

            let internal_ip = net_interfaces
                .and_then(|nis| nis.first())
                .and_then(|ni| ni.get("networkIP"))
                .and_then(|v| v.as_str())
                .map(String::from);

            let external_ip = net_interfaces
                .and_then(|nis| nis.first())
                .and_then(|ni| ni.get("accessConfigs"))
                .and_then(|v| v.as_array())
                .and_then(|acs| acs.first())
                .and_then(|ac| ac.get("natIP"))
                .and_then(|v| v.as_str())
                .map(String::from);

            let network = net_interfaces
                .and_then(|nis| nis.first())
                .and_then(|ni| ni.get("network"))
                .and_then(|v| v.as_str())
                .and_then(|n| n.rsplit('/').next())
                .map(String::from);

            GcpInstance {
                id: v
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: v
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                zone,
                machine_type,
                status: v
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("UNKNOWN")
                    .to_string(),
                internal_ip,
                external_ip,
                network,
            }
        })
        .collect();

    Ok(instances)
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn cloud_gcp_list_configs() -> Result<Vec<GcpConfig>, CloudError> {
    let output = tokio::process::Command::new("gcloud")
        .args([
            "config",
            "configurations",
            "list",
            "--format",
            "json",
        ])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let json = String::from_utf8_lossy(&output.stdout);
    parse_configs(&json)
}

#[tauri::command]
pub async fn cloud_gcp_activate_config(name: String) -> Result<(), CloudError> {
    let output = tokio::process::Command::new("gcloud")
        .args(["config", "configurations", "activate", &name])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(())
}

#[tauri::command]
pub async fn cloud_gcp_list_instances(
    project: String,
    zone: Option<String>,
) -> Result<Vec<GcpInstance>, CloudError> {
    let mut args = vec![
        "compute".to_string(),
        "instances".to_string(),
        "list".to_string(),
        "--project".to_string(),
        project,
        "--format".to_string(),
        "json".to_string(),
    ];

    if let Some(z) = zone {
        args.push("--zones".to_string());
        args.push(z);
    }

    let output = tokio::process::Command::new("gcloud")
        .args(&args)
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let json = String::from_utf8_lossy(&output.stdout);
    parse_instances(&json)
}

#[tauri::command]
pub async fn cloud_gcp_iap_tunnel(
    instance: String,
    project: String,
    zone: String,
) -> Result<String, CloudError> {
    let session_id = uuid::Uuid::new_v4().to_string();

    let _child = tokio::process::Command::new("gcloud")
        .args([
            "compute",
            "ssh",
            &instance,
            "--project",
            &project,
            "--zone",
            &zone,
            "--tunnel-through-iap",
        ])
        .spawn()
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    Ok(session_id)
}

#[tauri::command]
pub async fn cloud_gcp_list_buckets(project: String) -> Result<Vec<GcsBucket>, CloudError> {
    let output = tokio::process::Command::new("gcloud")
        .args([
            "storage",
            "buckets",
            "list",
            "--project",
            &project,
            "--format",
            "json",
        ])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let arr: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;

    let buckets = arr
        .iter()
        .map(|v| GcsBucket {
            name: v
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            location: v
                .get("location")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            storage_class: v
                .get("storageClass")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            time_created: v
                .get("timeCreated")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    Ok(buckets)
}

#[tauri::command]
pub async fn cloud_gcp_list_objects(
    bucket: String,
    prefix: String,
) -> Result<Vec<GcsObject>, CloudError> {
    let path = if prefix.is_empty() {
        format!("gs://{}", bucket)
    } else {
        format!("gs://{}/{}", bucket, prefix)
    };

    let output = tokio::process::Command::new("gcloud")
        .args([
            "storage",
            "objects",
            "list",
            &path,
            "--format",
            "json",
        ])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let arr: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;

    let objects = arr
        .iter()
        .map(|v| GcsObject {
            name: v
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            size: v
                .get("size")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            content_type: v
                .get("contentType")
                .and_then(|v| v.as_str())
                .unwrap_or("application/octet-stream")
                .to_string(),
            time_created: v
                .get("timeCreated")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            updated: v
                .get("updated")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    Ok(objects)
}

#[tauri::command]
pub async fn cloud_gcp_cloud_shell() -> Result<String, CloudError> {
    let session_id = uuid::Uuid::new_v4().to_string();

    // Verify gcloud auth
    let output = tokio::process::Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::AuthRequired(
            "GCP authentication required for Cloud Shell".to_string(),
        ));
    }

    // Cloud Shell is opened via gcloud cloud-shell ssh or the portal
    // Return session ID for frontend to manage
    Ok(session_id)
}

#[tauri::command]
pub async fn cloud_gcp_log_tail(
    app: tauri::AppHandle,
    resource: String,
    project: String,
) -> Result<(), CloudError> {
    let mut child = tokio::process::Command::new("gcloud")
        .args([
            "logging",
            "tail",
            &format!("resource.type={}", resource),
            "--project",
            &project,
            "--format",
            "json",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    let stdout = child.stdout.take();

    if let Some(stdout) = stdout {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        tokio::spawn(async move {
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = app.emit("cloud:log_line", &line);
            }
        });
    }

    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcp_parse_configs() {
        let json = r#"[
            {
                "is_active": true,
                "name": "default",
                "properties": {
                    "compute": {
                        "region": "us-central1",
                        "zone": "us-central1-a"
                    },
                    "core": {
                        "account": "user@example.com",
                        "project": "my-project-123"
                    }
                }
            },
            {
                "is_active": false,
                "name": "staging",
                "properties": {
                    "compute": {
                        "region": "europe-west1",
                        "zone": "europe-west1-b"
                    },
                    "core": {
                        "account": "user@example.com",
                        "project": "staging-project-456"
                    }
                }
            }
        ]"#;

        let configs = parse_configs(json).unwrap();

        assert_eq!(configs.len(), 2);

        assert_eq!(configs[0].name, "default");
        assert_eq!(configs[0].project, "my-project-123");
        assert_eq!(configs[0].region, "us-central1");
        assert_eq!(configs[0].zone, "us-central1-a");
        assert!(configs[0].is_active);

        assert_eq!(configs[1].name, "staging");
        assert_eq!(configs[1].project, "staging-project-456");
        assert_eq!(configs[1].region, "europe-west1");
        assert!(!configs[1].is_active);
    }

    #[test]
    fn test_gcp_parse_instances() {
        let json = r#"[
            {
                "id": "1234567890",
                "name": "web-server",
                "zone": "projects/my-project/zones/us-central1-a",
                "machineType": "projects/my-project/zones/us-central1-a/machineTypes/e2-medium",
                "status": "RUNNING",
                "networkInterfaces": [
                    {
                        "networkIP": "10.128.0.2",
                        "network": "projects/my-project/global/networks/default",
                        "accessConfigs": [
                            {
                                "name": "External NAT",
                                "natIP": "34.68.100.50"
                            }
                        ]
                    }
                ]
            },
            {
                "id": "9876543210",
                "name": "db-server",
                "zone": "projects/my-project/zones/us-central1-b",
                "machineType": "projects/my-project/zones/us-central1-b/machineTypes/n1-standard-4",
                "status": "TERMINATED",
                "networkInterfaces": [
                    {
                        "networkIP": "10.128.0.5",
                        "network": "projects/my-project/global/networks/default"
                    }
                ]
            }
        ]"#;

        let instances = parse_instances(json).unwrap();

        assert_eq!(instances.len(), 2);

        assert_eq!(instances[0].id, "1234567890");
        assert_eq!(instances[0].name, "web-server");
        assert_eq!(instances[0].zone, "us-central1-a");
        assert_eq!(instances[0].machine_type, "e2-medium");
        assert_eq!(instances[0].status, "RUNNING");
        assert_eq!(instances[0].internal_ip, Some("10.128.0.2".to_string()));
        assert_eq!(instances[0].external_ip, Some("34.68.100.50".to_string()));
        assert_eq!(instances[0].network, Some("default".to_string()));

        assert_eq!(instances[1].id, "9876543210");
        assert_eq!(instances[1].name, "db-server");
        assert_eq!(instances[1].zone, "us-central1-b");
        assert_eq!(instances[1].machine_type, "n1-standard-4");
        assert_eq!(instances[1].status, "TERMINATED");
        assert_eq!(instances[1].external_ip, None); // no accessConfigs
    }
}
