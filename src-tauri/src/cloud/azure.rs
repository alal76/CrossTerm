use serde::{Deserialize, Serialize};

use super::CloudError;

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureSubscription {
    pub id: String,
    pub name: String,
    pub state: String,
    pub tenant_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AzureLoginMethod {
    Interactive,
    DeviceCode,
    ManagedIdentity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureVm {
    pub id: String,
    pub name: String,
    pub resource_group: String,
    pub location: String,
    pub status: String,
    pub public_ip: Option<String>,
    pub private_ip: Option<String>,
    pub size: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureStorageAccount {
    pub name: String,
    pub resource_group: String,
    pub kind: String,
    pub sku: String,
    pub location: String,
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// List Azure subscription names from `az account list`.
pub(crate) async fn list_subscription_names() -> Result<Vec<String>, CloudError> {
    let output = tokio::process::Command::new("az")
        .args(["account", "list", "--output", "json", "--query", "[].name"])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(format!("az account list: {e}")))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let names: Vec<String> = serde_json::from_slice(&output.stdout)
        .map_err(|e| CloudError::Parse(e.to_string()))?;

    Ok(names)
}

/// Parse subscriptions from `az account list --output json`.
fn parse_subscriptions(json: &str) -> Result<Vec<AzureSubscription>, CloudError> {
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| CloudError::Parse(e.to_string()))?;

    let subs = arr
        .iter()
        .map(|v| AzureSubscription {
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
            state: v
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            tenant_id: v
                .get("tenantId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    Ok(subs)
}

/// Parse VMs from `az vm list --output json`.
fn parse_vms(json: &str) -> Result<Vec<AzureVm>, CloudError> {
    let arr: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|e| CloudError::Parse(e.to_string()))?;

    let vms = arr
        .iter()
        .map(|v| {
            // Extract resource group from ID path
            let id = v
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let resource_group = id
                .split('/')
                .collect::<Vec<_>>()
                .windows(2)
                .find(|w| w[0].eq_ignore_ascii_case("resourceGroups"))
                .map(|w| w[1].to_string())
                .unwrap_or_default();

            AzureVm {
                id,
                name: v
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                resource_group,
                location: v
                    .get("location")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                status: v
                    .get("powerState")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string(),
                public_ip: v
                    .get("publicIps")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from),
                private_ip: v
                    .get("privateIps")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from),
                size: v
                    .get("hardwareProfile")
                    .and_then(|v| v.get("vmSize"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            }
        })
        .collect();

    Ok(vms)
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn cloud_azure_list_subscriptions() -> Result<Vec<AzureSubscription>, CloudError> {
    let output = tokio::process::Command::new("az")
        .args(["account", "list", "--output", "json"])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let json = String::from_utf8_lossy(&output.stdout);
    parse_subscriptions(&json)
}

#[tauri::command]
pub async fn cloud_azure_set_subscription(id: String) -> Result<(), CloudError> {
    let output = tokio::process::Command::new("az")
        .args(["account", "set", "--subscription", &id])
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
pub async fn cloud_azure_login(method: AzureLoginMethod) -> Result<(), CloudError> {
    let mut args = vec!["login".to_string()];

    match method {
        AzureLoginMethod::DeviceCode => {
            args.push("--use-device-code".to_string());
        }
        AzureLoginMethod::ManagedIdentity => {
            args.push("--identity".to_string());
        }
        AzureLoginMethod::Interactive => {
            // Default behavior
        }
    }

    let output = tokio::process::Command::new("az")
        .args(&args)
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::AuthRequired(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(())
}

#[tauri::command]
pub async fn cloud_azure_list_vms(
    subscription: String,
    resource_group: Option<String>,
) -> Result<Vec<AzureVm>, CloudError> {
    let mut args = vec![
        "vm".to_string(),
        "list".to_string(),
        "--subscription".to_string(),
        subscription,
        "--show-details".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];

    if let Some(rg) = resource_group {
        args.push("--resource-group".to_string());
        args.push(rg);
    }

    let output = tokio::process::Command::new("az")
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
    parse_vms(&json)
}

#[tauri::command]
pub async fn cloud_azure_bastion_connect(
    vm_id: String,
    auth_type: String,
) -> Result<String, CloudError> {
    let session_id = uuid::Uuid::new_v4().to_string();

    let _child = tokio::process::Command::new("az")
        .args([
            "network",
            "bastion",
            "ssh",
            "--ids",
            &vm_id,
            "--auth-type",
            &auth_type,
        ])
        .spawn()
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    Ok(session_id)
}

#[tauri::command]
pub async fn cloud_azure_cloud_shell(shell_type: String) -> Result<String, CloudError> {
    let session_id = uuid::Uuid::new_v4().to_string();

    // Azure Cloud Shell doesn't have a direct CLI command for local embedding,
    // but we can open it via the REST API or redirect to the portal.
    // For now, start via `az cloud-shell` if available, or return a portal URL.
    let output = tokio::process::Command::new("az")
        .args(["account", "show", "--output", "json"])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::AuthRequired(
            "Azure login required for Cloud Shell".to_string(),
        ));
    }

    // Return session ID; frontend will open Cloud Shell via websocket or portal URL
    Ok(session_id)
}

#[tauri::command]
pub async fn cloud_azure_list_storage(
    subscription: String,
) -> Result<Vec<AzureStorageAccount>, CloudError> {
    let output = tokio::process::Command::new("az")
        .args([
            "storage",
            "account",
            "list",
            "--subscription",
            &subscription,
            "--output",
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

    let accounts = arr
        .iter()
        .map(|v| {
            let id = v
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let resource_group = id
                .split('/')
                .collect::<Vec<_>>()
                .windows(2)
                .find(|w| w[0].eq_ignore_ascii_case("resourceGroups"))
                .map(|w| w[1].to_string())
                .unwrap_or_default();

            AzureStorageAccount {
                name: v
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                resource_group,
                kind: v
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                sku: v
                    .get("sku")
                    .and_then(|v| v.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                location: v
                    .get("location")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            }
        })
        .collect();

    Ok(accounts)
}

#[tauri::command]
pub async fn cloud_azure_log_analytics_query(
    workspace: String,
    query: String,
) -> Result<Vec<serde_json::Value>, CloudError> {
    let output = tokio::process::Command::new("az")
        .args([
            "monitor",
            "log-analytics",
            "query",
            "--workspace",
            &workspace,
            "--analytics-query",
            &query,
            "--output",
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

    let results: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
    Ok(results)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_parse_subscriptions() {
        let json = r#"[
            {
                "cloudName": "AzureCloud",
                "id": "12345678-1234-1234-1234-123456789abc",
                "isDefault": true,
                "name": "Production",
                "state": "Enabled",
                "tenantId": "abcdefgh-abcd-abcd-abcd-abcdefghijkl",
                "user": {
                    "name": "user@example.com",
                    "type": "user"
                }
            },
            {
                "cloudName": "AzureCloud",
                "id": "87654321-4321-4321-4321-cba987654321",
                "isDefault": false,
                "name": "Development",
                "state": "Enabled",
                "tenantId": "abcdefgh-abcd-abcd-abcd-abcdefghijkl",
                "user": {
                    "name": "user@example.com",
                    "type": "user"
                }
            }
        ]"#;

        let subs = parse_subscriptions(json).unwrap();

        assert_eq!(subs.len(), 2);

        assert_eq!(subs[0].id, "12345678-1234-1234-1234-123456789abc");
        assert_eq!(subs[0].name, "Production");
        assert_eq!(subs[0].state, "Enabled");
        assert_eq!(subs[0].tenant_id, "abcdefgh-abcd-abcd-abcd-abcdefghijkl");

        assert_eq!(subs[1].id, "87654321-4321-4321-4321-cba987654321");
        assert_eq!(subs[1].name, "Development");
    }

    #[test]
    fn test_azure_parse_vms() {
        let json = r#"[
            {
                "id": "/subscriptions/12345678/resourceGroups/myRG/providers/Microsoft.Compute/virtualMachines/vm-web-01",
                "name": "vm-web-01",
                "location": "eastus",
                "powerState": "VM running",
                "publicIps": "20.30.40.50",
                "privateIps": "10.0.0.4",
                "hardwareProfile": {
                    "vmSize": "Standard_B2s"
                }
            },
            {
                "id": "/subscriptions/12345678/resourceGroups/devRG/providers/Microsoft.Compute/virtualMachines/vm-db-01",
                "name": "vm-db-01",
                "location": "westus2",
                "powerState": "VM deallocated",
                "publicIps": "",
                "privateIps": "10.1.0.10",
                "hardwareProfile": {
                    "vmSize": "Standard_D4s_v3"
                }
            }
        ]"#;

        let vms = parse_vms(json).unwrap();

        assert_eq!(vms.len(), 2);

        assert_eq!(vms[0].name, "vm-web-01");
        assert_eq!(vms[0].resource_group, "myRG");
        assert_eq!(vms[0].location, "eastus");
        assert_eq!(vms[0].status, "VM running");
        assert_eq!(vms[0].public_ip, Some("20.30.40.50".to_string()));
        assert_eq!(vms[0].private_ip, Some("10.0.0.4".to_string()));
        assert_eq!(vms[0].size, "Standard_B2s");

        assert_eq!(vms[1].name, "vm-db-01");
        assert_eq!(vms[1].resource_group, "devRG");
        assert_eq!(vms[1].status, "VM deallocated");
        assert_eq!(vms[1].public_ip, None); // empty string filtered
        assert_eq!(vms[1].size, "Standard_D4s_v3");
    }
}
