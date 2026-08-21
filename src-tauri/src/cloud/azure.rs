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

            let resource_group = extract_resource_group(&id);

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

/// Extract the resource group segment from an Azure resource ID, e.g.
/// `/subscriptions/.../resourceGroups/myRG/providers/...` -> `myRG`.
fn extract_resource_group(id: &str) -> String {
    id.split('/')
        .collect::<Vec<_>>()
        .windows(2)
        .find(|w| w[0].eq_ignore_ascii_case("resourceGroups"))
        .map(|w| w[1].to_string())
        .unwrap_or_default()
}

/// Parse storage accounts from `az storage account list --output json`.
fn parse_storage_accounts(json: &[u8]) -> Result<Vec<AzureStorageAccount>, CloudError> {
    let arr: Vec<serde_json::Value> = serde_json::from_slice(json)?;

    let accounts = arr
        .iter()
        .map(|v| {
            let id = v
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let resource_group = extract_resource_group(&id);

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

/// Build the `az login` argument list for a given login method. Split out
/// so the method -> flag mapping is unit-testable without invoking `az`.
fn azure_login_args(method: &AzureLoginMethod) -> Vec<String> {
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

    args
}

#[tauri::command]
pub async fn cloud_azure_login(method: AzureLoginMethod) -> Result<(), CloudError> {
    let args = azure_login_args(&method);

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

/// Build the `az vm list` argument list, optionally scoping to a resource
/// group. Split out from the command so the optional-arg branch is
/// unit-testable without invoking `az`.
fn azure_list_vms_args(subscription: &str, resource_group: &Option<String>) -> Vec<String> {
    let mut args = vec![
        "vm".to_string(),
        "list".to_string(),
        "--subscription".to_string(),
        subscription.to_string(),
        "--show-details".to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];

    if let Some(rg) = resource_group {
        args.push("--resource-group".to_string());
        args.push(rg.clone());
    }

    args
}

#[tauri::command]
pub async fn cloud_azure_list_vms(
    subscription: String,
    resource_group: Option<String>,
) -> Result<Vec<AzureVm>, CloudError> {
    let args = azure_list_vms_args(&subscription, &resource_group);

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
pub async fn cloud_azure_start_vm(subscription: String, vm_id: String) -> Result<(), CloudError> {
    let output = tokio::process::Command::new("az")
        .args([
            "vm", "start", "--ids", &vm_id, "--subscription", &subscription,
        ])
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
pub async fn cloud_azure_stop_vm(subscription: String, vm_id: String) -> Result<(), CloudError> {
    let output = tokio::process::Command::new("az")
        .args([
            "vm", "deallocate", "--ids", &vm_id, "--subscription", &subscription,
        ])
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
    let _ = shell_type;
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

    parse_storage_accounts(&output.stdout)
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

// ── P2-CLOUD-16: Azure Storage Explorer ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureBlobEntry {
    pub name: String,
    pub content_length: u64,
    pub content_type: String,
    pub last_modified: String,
    pub blob_type: String,
}

/// Parse blob entries from `az storage blob list --output json`.
fn parse_blob_entries(json: &[u8]) -> Result<Vec<AzureBlobEntry>, CloudError> {
    let arr: Vec<serde_json::Value> = serde_json::from_slice(json)?;

    let entries = arr
        .iter()
        .map(|v| AzureBlobEntry {
            name: v
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            content_length: v
                .get("properties")
                .and_then(|p| p.get("contentLength"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            content_type: v
                .get("properties")
                .and_then(|p| p.get("contentType"))
                .and_then(|v| v.as_str())
                .unwrap_or("application/octet-stream")
                .to_string(),
            last_modified: v
                .get("properties")
                .and_then(|p| p.get("lastModified"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            blob_type: v
                .get("properties")
                .and_then(|p| p.get("blobType"))
                .and_then(|v| v.as_str())
                .unwrap_or("BlockBlob")
                .to_string(),
        })
        .collect();

    Ok(entries)
}

#[tauri::command]
pub async fn cloud_azure_storage_browse(
    account: String,
    container: Option<String>,
) -> Result<Vec<AzureBlobEntry>, CloudError> {
    let container_name = container.unwrap_or_else(|| "$root".to_string());

    let output = tokio::process::Command::new("az")
        .args([
            "storage",
            "blob",
            "list",
            "--account-name",
            &account,
            "--container-name",
            &container_name,
            "--output",
            "json",
        ])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(format!("az storage blob list: {e}")))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    parse_blob_entries(&output.stdout)
}

// ── P2-CLOUD-17: AKS kubectl integration ───────────────────────────────

#[tauri::command]
pub async fn cloud_azure_aks_get_credentials(
    cluster: String,
    resource_group: String,
) -> Result<String, CloudError> {
    let output = tokio::process::Command::new("az")
        .args([
            "aks",
            "get-credentials",
            "--name",
            &cluster,
            "--resource-group",
            &resource_group,
            "--overwrite-existing",
        ])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(format!("az aks get-credentials: {e}")))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(format!("Credentials configured for cluster {}", cluster))
}

#[tauri::command]
pub async fn cloud_azure_aks_exec(
    cluster: String,
    resource_group: String,
    namespace: String,
    pod: String,
    command: String,
) -> Result<String, CloudError> {
    // First ensure credentials are available
    cloud_azure_aks_get_credentials(cluster, resource_group).await?;

    let output = tokio::process::Command::new("kubectl")
        .args([
            "exec",
            "-it",
            &pod,
            "-n",
            &namespace,
            "--",
            "sh",
            "-c",
            &command,
        ])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(format!("kubectl exec: {e}")))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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

    #[test]
    fn test_azure_parse_vms_missing_fields_default() {
        // A VM entry missing location/powerState/hardwareProfile/id entirely
        // should fall back to defaults instead of erroring.
        let json = r#"[{ "name": "bare-vm" }]"#;
        let vms = parse_vms(json).unwrap();
        assert_eq!(vms.len(), 1);
        assert_eq!(vms[0].name, "bare-vm");
        assert_eq!(vms[0].id, "");
        assert_eq!(vms[0].resource_group, "");
        assert_eq!(vms[0].location, "");
        assert_eq!(vms[0].status, "Unknown");
        assert_eq!(vms[0].public_ip, None);
        assert_eq!(vms[0].private_ip, None);
        assert_eq!(vms[0].size, "");
    }

    #[test]
    fn test_azure_login_args_interactive_has_no_extra_flag() {
        assert_eq!(azure_login_args(&AzureLoginMethod::Interactive), vec!["login"]);
    }

    #[test]
    fn test_azure_login_args_device_code() {
        assert_eq!(
            azure_login_args(&AzureLoginMethod::DeviceCode),
            vec!["login", "--use-device-code"]
        );
    }

    #[test]
    fn test_azure_login_args_managed_identity() {
        assert_eq!(
            azure_login_args(&AzureLoginMethod::ManagedIdentity),
            vec!["login", "--identity"]
        );
    }

    #[test]
    fn test_azure_list_vms_args_without_resource_group() {
        let args = azure_list_vms_args("sub-1", &None);
        assert_eq!(
            args,
            vec!["vm", "list", "--subscription", "sub-1", "--show-details", "--output", "json"]
        );
    }

    #[test]
    fn test_azure_list_vms_args_with_resource_group() {
        let args = azure_list_vms_args("sub-1", &Some("myRG".to_string()));
        assert_eq!(
            args,
            vec![
                "vm",
                "list",
                "--subscription",
                "sub-1",
                "--show-details",
                "--output",
                "json",
                "--resource-group",
                "myRG"
            ]
        );
    }

    #[test]
    fn test_azure_parse_subscriptions_empty() {
        let subs = parse_subscriptions("[]").unwrap();
        assert!(subs.is_empty());
    }

    #[test]
    fn test_azure_parse_subscriptions_malformed_errors() {
        assert!(parse_subscriptions("not json").is_err());
    }

    #[test]
    fn test_azure_parse_subscriptions_missing_fields_default() {
        let json = r#"[{"name": "OnlyName"}]"#;
        let subs = parse_subscriptions(json).unwrap();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].name, "OnlyName");
        assert_eq!(subs[0].id, "");
        assert_eq!(subs[0].state, "Unknown");
        assert_eq!(subs[0].tenant_id, "");
    }

    #[test]
    fn test_extract_resource_group() {
        assert_eq!(
            extract_resource_group(
                "/subscriptions/abc/resourceGroups/myRG/providers/Microsoft.Compute/virtualMachines/vm1"
            ),
            "myRG"
        );
        // case-insensitive match on the segment name
        assert_eq!(
            extract_resource_group("/subscriptions/abc/RESOURCEGROUPS/otherRG/providers/x"),
            "otherRG"
        );
        // no resourceGroups segment present
        assert_eq!(extract_resource_group("/subscriptions/abc"), "");
        assert_eq!(extract_resource_group(""), "");
    }

    #[test]
    fn test_azure_parse_storage_accounts() {
        let json = br#"[
            {
                "id": "/subscriptions/abc/resourceGroups/storageRG/providers/Microsoft.Storage/storageAccounts/mystorage",
                "name": "mystorage",
                "kind": "StorageV2",
                "sku": { "name": "Standard_LRS" },
                "location": "eastus"
            }
        ]"#;

        let accounts = parse_storage_accounts(json).unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].name, "mystorage");
        assert_eq!(accounts[0].resource_group, "storageRG");
        assert_eq!(accounts[0].kind, "StorageV2");
        assert_eq!(accounts[0].sku, "Standard_LRS");
        assert_eq!(accounts[0].location, "eastus");
    }

    #[test]
    fn test_azure_parse_storage_accounts_missing_fields_default() {
        let json = br#"[{"name": "bare"}]"#;
        let accounts = parse_storage_accounts(json).unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].resource_group, "");
        assert_eq!(accounts[0].kind, "");
        assert_eq!(accounts[0].sku, "");
    }

    #[test]
    fn test_azure_parse_storage_accounts_malformed_errors() {
        assert!(parse_storage_accounts(b"not json").is_err());
    }

    #[test]
    fn test_azure_parse_blob_entries() {
        let json = br#"[
            {
                "name": "folder/file.txt",
                "properties": {
                    "contentLength": 12345,
                    "contentType": "text/plain",
                    "lastModified": "2024-05-01T10:00:00Z",
                    "blobType": "BlockBlob"
                }
            },
            {
                "name": "pageblob.vhd",
                "properties": {
                    "contentLength": 999,
                    "blobType": "PageBlob"
                }
            }
        ]"#;

        let entries = parse_blob_entries(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "folder/file.txt");
        assert_eq!(entries[0].content_length, 12345);
        assert_eq!(entries[0].content_type, "text/plain");
        assert_eq!(entries[0].blob_type, "BlockBlob");

        // Second entry has no contentType, so it should default.
        assert_eq!(entries[1].content_type, "application/octet-stream");
        assert_eq!(entries[1].blob_type, "PageBlob");
        assert_eq!(entries[1].last_modified, "");
    }

    #[test]
    fn test_azure_parse_blob_entries_empty() {
        let entries = parse_blob_entries(b"[]").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_azure_parse_vms_empty_private_ip_filtered_to_none() {
        // Mirrors the already-covered empty-publicIps case: an empty
        // `privateIps` string should also be filtered to `None`, not kept
        // as `Some("")`.
        let json = r#"[
            {
                "id": "/subscriptions/abc/resourceGroups/rg/providers/Microsoft.Compute/virtualMachines/vm1",
                "name": "vm-no-private-ip",
                "location": "eastus",
                "powerState": "VM running",
                "publicIps": "1.2.3.4",
                "privateIps": "",
                "hardwareProfile": { "vmSize": "Standard_B1s" }
            }
        ]"#;
        let vms = parse_vms(json).unwrap();
        assert_eq!(vms.len(), 1);
        assert_eq!(vms[0].public_ip, Some("1.2.3.4".to_string()));
        assert_eq!(vms[0].private_ip, None);
    }
}
