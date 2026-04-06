pub mod aws;
pub mod azure;
pub mod gcp;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

// ── Error ───────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum CloudError {
    #[error("CLI not found: {0}")]
    CliNotFound(String),
    #[error("CLI execution failed: {0}")]
    CliExecution(String),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Authentication required for {0}")]
    AuthRequired(String),
    #[error("Provider not configured: {0}")]
    NotConfigured(String),
    #[error("Resource not found: {0}")]
    ResourceNotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl Serialize for CloudError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudProvider {
    Aws,
    Azure,
    Gcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliStatus {
    Installed { version: String, path: String },
    NotInstalled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudProviderStatus {
    pub provider: CloudProvider,
    pub cli_status: CliStatus,
    pub profiles: Vec<String>,
    pub active_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudAssetType {
    Provider,
    Region,
    ResourceGroup,
    Compute,
    Storage,
    Kubernetes,
    Serverless,
    Database,
    Network,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudAssetNode {
    pub id: String,
    pub name: String,
    pub node_type: CloudAssetType,
    pub provider: CloudProvider,
    pub children: Vec<CloudAssetNode>,
    pub metadata: HashMap<String, String>,
}

// ── State ───────────────────────────────────────────────────────────────

pub struct CloudState {
    pub provider_status: Mutex<HashMap<CloudProvider, CloudProviderStatus>>,
}

impl CloudState {
    pub fn new() -> Self {
        Self {
            provider_status: Mutex::new(HashMap::new()),
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Detect a CLI binary by name, returning its path and version output.
async fn detect_cli(bin: &str, version_flag: &str) -> CliStatus {
    let which_result = tokio::process::Command::new("which")
        .arg(bin)
        .output()
        .await;

    let path = match which_result {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => return CliStatus::NotInstalled,
    };

    let version_result = tokio::process::Command::new(bin)
        .arg(version_flag)
        .output()
        .await;

    let version = match version_result {
        Ok(output) => {
            let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if raw.is_empty() {
                String::from_utf8_lossy(&output.stderr).trim().to_string()
            } else {
                raw
            }
        }
        Err(_) => "unknown".to_string(),
    };

    CliStatus::Installed { version, path }
}

/// Parse the version string from `aws --version` output.
fn parse_aws_version(raw: &str) -> String {
    // e.g. "aws-cli/2.15.0 Python/3.11.6 ..."
    raw.split_whitespace()
        .next()
        .unwrap_or(raw)
        .to_string()
}

/// Parse the version string from `az version` JSON output.
fn parse_az_version(raw: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) {
        if let Some(ver) = v.get("azure-cli").and_then(|v| v.as_str()) {
            return format!("azure-cli/{}", ver);
        }
    }
    raw.lines().next().unwrap_or(raw).to_string()
}

/// Parse the version string from `gcloud version` output.
fn parse_gcloud_version(raw: &str) -> String {
    // e.g. "Google Cloud SDK 456.0.0"
    raw.lines()
        .next()
        .unwrap_or(raw)
        .to_string()
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn cloud_detect_clis(
    state: tauri::State<'_, CloudState>,
) -> Result<Vec<CloudProviderStatus>, CloudError> {
    // Detect AWS CLI
    let aws_cli = detect_cli("aws", "--version").await;
    let aws_cli = match aws_cli {
        CliStatus::Installed { version, path } => CliStatus::Installed {
            version: parse_aws_version(&version),
            path,
        },
        other => other,
    };
    let aws_profiles = match &aws_cli {
        CliStatus::Installed { .. } => aws::list_profile_names().await.unwrap_or_default(),
        CliStatus::NotInstalled => vec![],
    };
    let aws_active = aws_profiles.first().cloned();
    let aws_status = CloudProviderStatus {
        provider: CloudProvider::Aws,
        cli_status: aws_cli,
        profiles: aws_profiles,
        active_profile: aws_active,
    };

    // Detect Azure CLI
    let az_raw = tokio::process::Command::new("az")
        .arg("version")
        .arg("--output")
        .arg("json")
        .output()
        .await;
    let az_cli = match az_raw {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout).to_string();
            let path = detect_cli_path("az").await;
            CliStatus::Installed {
                version: parse_az_version(&raw),
                path,
            }
        }
        _ => CliStatus::NotInstalled,
    };
    let az_profiles = match &az_cli {
        CliStatus::Installed { .. } => azure::list_subscription_names().await.unwrap_or_default(),
        CliStatus::NotInstalled => vec![],
    };
    let az_active = az_profiles.first().cloned();
    let az_status = CloudProviderStatus {
        provider: CloudProvider::Azure,
        cli_status: az_cli,
        profiles: az_profiles,
        active_profile: az_active,
    };

    // Detect GCP CLI
    let gcp_raw = tokio::process::Command::new("gcloud")
        .arg("version")
        .output()
        .await;
    let gcp_cli = match gcp_raw {
        Ok(output) if output.status.success() => {
            let raw = String::from_utf8_lossy(&output.stdout).to_string();
            let path = detect_cli_path("gcloud").await;
            CliStatus::Installed {
                version: parse_gcloud_version(&raw),
                path,
            }
        }
        _ => CliStatus::NotInstalled,
    };
    let gcp_profiles = match &gcp_cli {
        CliStatus::Installed { .. } => gcp::list_config_names().await.unwrap_or_default(),
        CliStatus::NotInstalled => vec![],
    };
    let gcp_active = gcp_profiles.first().cloned();
    let gcp_status = CloudProviderStatus {
        provider: CloudProvider::Gcp,
        cli_status: gcp_cli,
        profiles: gcp_profiles,
        active_profile: gcp_active,
    };

    let results = vec![aws_status.clone(), az_status.clone(), gcp_status.clone()];

    // Cache in state
    let mut cache = state.provider_status.lock().unwrap();
    cache.insert(CloudProvider::Aws, aws_status);
    cache.insert(CloudProvider::Azure, az_status);
    cache.insert(CloudProvider::Gcp, gcp_status);

    Ok(results)
}

/// Helper to get the path of a CLI binary.
async fn detect_cli_path(bin: &str) -> String {
    tokio::process::Command::new("which")
        .arg(bin)
        .output()
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

#[tauri::command]
pub async fn cloud_get_asset_tree(
    state: tauri::State<'_, CloudState>,
) -> Result<Vec<CloudAssetNode>, CloudError> {
    let cache = state.provider_status.lock().unwrap();
    let mut roots = Vec::new();

    for (provider, status) in cache.iter() {
        if matches!(status.cli_status, CliStatus::NotInstalled) {
            continue;
        }

        let provider_name = match provider {
            CloudProvider::Aws => "Amazon Web Services",
            CloudProvider::Azure => "Microsoft Azure",
            CloudProvider::Gcp => "Google Cloud Platform",
        };

        let node = CloudAssetNode {
            id: Uuid::new_v4().to_string(),
            name: provider_name.to_string(),
            node_type: CloudAssetType::Provider,
            provider: *provider,
            children: vec![],
            metadata: {
                let mut m = HashMap::new();
                if let Some(ref profile) = status.active_profile {
                    m.insert("active_profile".to_string(), profile.clone());
                }
                m
            },
        };

        roots.push(node);
    }

    Ok(roots)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_detection() {
        // Test CliStatus variants serialize correctly
        let installed = CliStatus::Installed {
            version: "aws-cli/2.15.0".to_string(),
            path: "/usr/local/bin/aws".to_string(),
        };
        let json = serde_json::to_string(&installed).unwrap();
        assert!(json.contains("installed"));
        assert!(json.contains("aws-cli/2.15.0"));

        let not_installed = CliStatus::NotInstalled;
        let json = serde_json::to_string(&not_installed).unwrap();
        assert!(json.contains("not_installed"));

        // Test version parsing
        assert_eq!(
            parse_aws_version("aws-cli/2.15.0 Python/3.11.6 Darwin/23.0.0"),
            "aws-cli/2.15.0"
        );

        assert_eq!(
            parse_az_version(r#"{"azure-cli": "2.55.0", "azure-cli-core": "2.55.0"}"#),
            "azure-cli/2.55.0"
        );

        assert_eq!(
            parse_gcloud_version("Google Cloud SDK 456.0.0\nbq 2.0.98\ncore 2023.11.10"),
            "Google Cloud SDK 456.0.0"
        );
    }

    #[test]
    fn test_asset_tree_structure() {
        // Build a mock tree with all three providers
        let aws_node = CloudAssetNode {
            id: Uuid::new_v4().to_string(),
            name: "Amazon Web Services".to_string(),
            node_type: CloudAssetType::Provider,
            provider: CloudProvider::Aws,
            children: vec![CloudAssetNode {
                id: Uuid::new_v4().to_string(),
                name: "us-east-1".to_string(),
                node_type: CloudAssetType::Region,
                provider: CloudProvider::Aws,
                children: vec![CloudAssetNode {
                    id: Uuid::new_v4().to_string(),
                    name: "i-abc123".to_string(),
                    node_type: CloudAssetType::Compute,
                    provider: CloudProvider::Aws,
                    children: vec![],
                    metadata: HashMap::new(),
                }],
                metadata: HashMap::new(),
            }],
            metadata: HashMap::new(),
        };

        let azure_node = CloudAssetNode {
            id: Uuid::new_v4().to_string(),
            name: "Microsoft Azure".to_string(),
            node_type: CloudAssetType::Provider,
            provider: CloudProvider::Azure,
            children: vec![],
            metadata: HashMap::new(),
        };

        let gcp_node = CloudAssetNode {
            id: Uuid::new_v4().to_string(),
            name: "Google Cloud Platform".to_string(),
            node_type: CloudAssetType::Provider,
            provider: CloudProvider::Gcp,
            children: vec![],
            metadata: HashMap::new(),
        };

        let tree = vec![aws_node, azure_node, gcp_node];

        // Verify structure
        assert_eq!(tree.len(), 3);
        assert!(matches!(tree[0].node_type, CloudAssetType::Provider));
        assert!(matches!(tree[0].provider, CloudProvider::Aws));
        assert_eq!(tree[0].children.len(), 1);
        assert!(matches!(
            tree[0].children[0].node_type,
            CloudAssetType::Region
        ));
        assert_eq!(tree[0].children[0].children.len(), 1);
        assert!(matches!(
            tree[0].children[0].children[0].node_type,
            CloudAssetType::Compute
        ));

        // Verify serialization round-trip
        let json = serde_json::to_string(&tree).unwrap();
        let deserialized: Vec<CloudAssetNode> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.len(), 3);
        assert_eq!(deserialized[0].name, "Amazon Web Services");
        assert_eq!(deserialized[1].name, "Microsoft Azure");
        assert_eq!(deserialized[2].name, "Google Cloud Platform");
    }
}
