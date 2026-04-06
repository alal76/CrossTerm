use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::Emitter;

use super::CloudError;

// ── Types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsProfile {
    pub name: String,
    pub region: Option<String>,
    pub sso_start_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ec2Instance {
    pub id: String,
    pub name: String,
    pub state: String,
    pub instance_type: String,
    pub public_ip: Option<String>,
    pub private_ip: Option<String>,
    pub key_name: Option<String>,
    pub vpc_id: Option<String>,
    pub launch_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Bucket {
    pub name: String,
    pub region: String,
    pub creation_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Object {
    pub key: String,
    pub size: u64,
    pub last_modified: String,
    pub storage_class: String,
    pub etag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub total_cost: f64,
    pub currency: String,
    pub start_date: String,
    pub end_date: String,
    pub by_service: Vec<ServiceCost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceCost {
    pub service_name: String,
    pub cost: f64,
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// List AWS profile names from the AWS CLI config.
pub(crate) async fn list_profile_names() -> Result<Vec<String>, CloudError> {
    let output = tokio::process::Command::new("aws")
        .args(["configure", "list-profiles"])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(format!("aws configure list-profiles: {e}")))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let profiles = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    Ok(profiles)
}

/// Parse AWS profiles from `aws configure list-profiles` output plus config files.
fn parse_profiles_from_config(config_content: &str, credentials_content: &str) -> Vec<AwsProfile> {
    let mut profiles: HashMap<String, AwsProfile> = HashMap::new();

    // Parse profiles from config file
    let mut current_profile: Option<String> = None;
    for line in config_content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            let section = line.trim_start_matches('[').trim_end_matches(']').trim();
            let name = section.strip_prefix("profile ").unwrap_or(section);
            current_profile = Some(name.to_string());
            profiles.entry(name.to_string()).or_insert_with(|| AwsProfile {
                name: name.to_string(),
                region: None,
                sso_start_url: None,
            });
        } else if let Some(ref profile_name) = current_profile {
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().to_string();
                if let Some(profile) = profiles.get_mut(profile_name) {
                    match key {
                        "region" => profile.region = Some(value),
                        "sso_start_url" => profile.sso_start_url = Some(value),
                        _ => {}
                    }
                }
            }
        }
    }

    // Parse profiles from credentials file (add any not already found)
    current_profile = None;
    for line in credentials_content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            let name = line
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .to_string();
            current_profile = Some(name.clone());
            profiles.entry(name.clone()).or_insert_with(|| AwsProfile {
                name,
                region: None,
                sso_start_url: None,
            });
        }
    }

    let _ = current_profile; // suppress unused warning

    profiles.into_values().collect()
}

/// Parse EC2 instances from `aws ec2 describe-instances --output json`.
fn parse_ec2_instances(json: &str) -> Result<Vec<Ec2Instance>, CloudError> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|e| CloudError::Parse(e.to_string()))?;

    let reservations = root
        .get("Reservations")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CloudError::Parse("Missing Reservations key".to_string()))?;

    let mut instances = Vec::new();
    for reservation in reservations {
        let insts = reservation
            .get("Instances")
            .and_then(|v| v.as_array())
            .unwrap_or(&Vec::new())
            .clone();

        for inst in &insts {
            let id = inst
                .get("InstanceId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let name = inst
                .get("Tags")
                .and_then(|v| v.as_array())
                .and_then(|tags| {
                    tags.iter().find_map(|t| {
                        if t.get("Key").and_then(|k| k.as_str()) == Some("Name") {
                            t.get("Value").and_then(|v| v.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_default();

            let state = inst
                .get("State")
                .and_then(|v| v.get("Name"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let instance_type = inst
                .get("InstanceType")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let public_ip = inst
                .get("PublicIpAddress")
                .and_then(|v| v.as_str())
                .map(String::from);

            let private_ip = inst
                .get("PrivateIpAddress")
                .and_then(|v| v.as_str())
                .map(String::from);

            let key_name = inst
                .get("KeyName")
                .and_then(|v| v.as_str())
                .map(String::from);

            let vpc_id = inst
                .get("VpcId")
                .and_then(|v| v.as_str())
                .map(String::from);

            let launch_time = inst
                .get("LaunchTime")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            instances.push(Ec2Instance {
                id,
                name,
                state,
                instance_type,
                public_ip,
                private_ip,
                key_name,
                vpc_id,
                launch_time,
            });
        }
    }

    Ok(instances)
}

/// Parse S3 buckets from `aws s3api list-buckets --output json`.
fn parse_s3_buckets(json: &str) -> Result<Vec<S3Bucket>, CloudError> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|e| CloudError::Parse(e.to_string()))?;

    let buckets = root
        .get("Buckets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| CloudError::Parse("Missing Buckets key".to_string()))?;

    let result = buckets
        .iter()
        .map(|b| S3Bucket {
            name: b
                .get("Name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            region: "us-east-1".to_string(), // bucket location requires separate call
            creation_date: b
                .get("CreationDate")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
        .collect();

    Ok(result)
}

/// Parse S3 objects from `aws s3api list-objects-v2 --output json`.
fn parse_s3_objects(json: &str) -> Result<Vec<S3Object>, CloudError> {
    let root: serde_json::Value =
        serde_json::from_str(json).map_err(|e| CloudError::Parse(e.to_string()))?;

    let contents = root
        .get("Contents")
        .and_then(|v| v.as_array())
        .unwrap_or(&Vec::new())
        .clone();

    let result = contents
        .iter()
        .map(|o| S3Object {
            key: o
                .get("Key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            size: o
                .get("Size")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            last_modified: o
                .get("LastModified")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            storage_class: o
                .get("StorageClass")
                .and_then(|v| v.as_str())
                .unwrap_or("STANDARD")
                .to_string(),
            etag: o.get("ETag").and_then(|v| v.as_str()).map(String::from),
        })
        .collect();

    Ok(result)
}

// ── Tauri Commands ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn cloud_aws_list_profiles() -> Result<Vec<AwsProfile>, CloudError> {
    let home = dirs::home_dir().ok_or_else(|| {
        CloudError::CliExecution("Cannot determine home directory".to_string())
    })?;

    let config_path = home.join(".aws").join("config");
    let creds_path = home.join(".aws").join("credentials");

    let config_content = tokio::fs::read_to_string(&config_path)
        .await
        .unwrap_or_default();
    let creds_content = tokio::fs::read_to_string(&creds_path)
        .await
        .unwrap_or_default();

    let profiles = parse_profiles_from_config(&config_content, &creds_content);
    Ok(profiles)
}

#[tauri::command]
pub async fn cloud_aws_switch_profile(profile: String) -> Result<(), CloudError> {
    // Set AWS_PROFILE environment variable for subsequent commands
    std::env::set_var("AWS_PROFILE", &profile);

    // Verify profile exists
    let output = tokio::process::Command::new("aws")
        .args(["configure", "list", "--profile", &profile])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::NotConfigured(format!(
            "Profile '{}' not found",
            profile
        )));
    }

    Ok(())
}

#[tauri::command]
pub async fn cloud_aws_sso_login(profile: String) -> Result<(), CloudError> {
    let output = tokio::process::Command::new("aws")
        .args(["sso", "login", "--profile", &profile])
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
pub async fn cloud_aws_list_ec2(region: String) -> Result<Vec<Ec2Instance>, CloudError> {
    let output = tokio::process::Command::new("aws")
        .args([
            "ec2",
            "describe-instances",
            "--region",
            &region,
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

    let json = String::from_utf8_lossy(&output.stdout);
    parse_ec2_instances(&json)
}

#[tauri::command]
pub async fn cloud_aws_ssm_start(
    instance_id: String,
    region: String,
) -> Result<String, CloudError> {
    let session_id = uuid::Uuid::new_v4().to_string();

    // Start SSM session in background
    let _child = tokio::process::Command::new("aws")
        .args([
            "ssm",
            "start-session",
            "--target",
            &instance_id,
            "--region",
            &region,
        ])
        .spawn()
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    Ok(session_id)
}

#[tauri::command]
pub async fn cloud_aws_list_s3_buckets() -> Result<Vec<S3Bucket>, CloudError> {
    let output = tokio::process::Command::new("aws")
        .args(["s3api", "list-buckets", "--output", "json"])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let json = String::from_utf8_lossy(&output.stdout);
    parse_s3_buckets(&json)
}

#[tauri::command]
pub async fn cloud_aws_list_s3_objects(
    bucket: String,
    prefix: String,
) -> Result<Vec<S3Object>, CloudError> {
    let output = tokio::process::Command::new("aws")
        .args([
            "s3api",
            "list-objects-v2",
            "--bucket",
            &bucket,
            "--prefix",
            &prefix,
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

    let json = String::from_utf8_lossy(&output.stdout);
    parse_s3_objects(&json)
}

#[tauri::command]
pub async fn cloud_aws_cloudwatch_tail(
    app: tauri::AppHandle,
    log_group: String,
    log_stream: Option<String>,
) -> Result<(), CloudError> {
    let mut args = vec![
        "logs".to_string(),
        "tail".to_string(),
        log_group,
        "--follow".to_string(),
        "--format".to_string(),
        "short".to_string(),
    ];

    if let Some(stream) = log_stream {
        args.push("--log-stream-names".to_string());
        args.push(stream);
    }

    let mut child = tokio::process::Command::new("aws")
        .args(&args)
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

#[tauri::command]
pub async fn cloud_aws_ecs_exec(
    cluster: String,
    task: String,
    container: String,
) -> Result<String, CloudError> {
    let session_id = uuid::Uuid::new_v4().to_string();

    let _child = tokio::process::Command::new("aws")
        .args([
            "ecs",
            "execute-command",
            "--cluster",
            &cluster,
            "--task",
            &task,
            "--container",
            &container,
            "--interactive",
            "--command",
            "/bin/sh",
        ])
        .spawn()
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    Ok(session_id)
}

#[tauri::command]
pub async fn cloud_aws_lambda_invoke(
    function: String,
    payload: String,
) -> Result<String, CloudError> {
    let output = tokio::process::Command::new("aws")
        .args([
            "lambda",
            "invoke",
            "--function-name",
            &function,
            "--payload",
            &payload,
            "--cli-binary-format",
            "raw-in-base64-out",
            "/dev/stdout",
        ])
        .output()
        .await
        .map_err(|e| CloudError::CliExecution(e.to_string()))?;

    if !output.status.success() {
        return Err(CloudError::CliExecution(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[tauri::command]
pub async fn cloud_aws_cost_summary() -> Result<CostSummary, CloudError> {
    let output = tokio::process::Command::new("aws")
        .args([
            "ce",
            "get-cost-and-usage",
            "--time-period",
            &format!(
                "Start={},End={}",
                chrono::Utc::now().format("%Y-%m-01"),
                chrono::Utc::now().format("%Y-%m-%d")
            ),
            "--granularity",
            "MONTHLY",
            "--metrics",
            "BlendedCost",
            "--group-by",
            "Type=DIMENSION,Key=SERVICE",
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

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

    let mut by_service = Vec::new();
    let mut total = 0.0;
    let mut currency = "USD".to_string();

    if let Some(results) = json.get("ResultsByTime").and_then(|v| v.as_array()) {
        for result in results {
            if let Some(groups) = result.get("Groups").and_then(|v| v.as_array()) {
                for group in groups {
                    let svc = group
                        .get("Keys")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.first())
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    let metrics = group.get("Metrics").and_then(|v| v.get("BlendedCost"));
                    let amount = metrics
                        .and_then(|v| v.get("Amount"))
                        .and_then(|v| v.as_str())
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);

                    if let Some(unit) = metrics.and_then(|v| v.get("Unit")).and_then(|v| v.as_str())
                    {
                        currency = unit.to_string();
                    }

                    total += amount;
                    by_service.push(ServiceCost {
                        service_name: svc,
                        cost: amount,
                    });
                }
            }
        }
    }

    Ok(CostSummary {
        total_cost: total,
        currency,
        start_date: chrono::Utc::now().format("%Y-%m-01").to_string(),
        end_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        by_service,
    })
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_parse_profiles() {
        let config = r#"
[default]
region = us-east-1

[profile dev]
region = us-west-2
sso_start_url = https://my-sso.awsapps.com/start

[profile staging]
region = eu-west-1
"#;

        let credentials = r#"
[default]
aws_access_key_id = AKIAIOSFODNN7EXAMPLE
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY

[production]
aws_access_key_id = AKIAI44QH8DHBEXAMPLE
aws_secret_access_key = je7MtGbClwBF/2Zp9Utk/h3yCo8nvbEXAMPLEKEY
"#;

        let profiles = parse_profiles_from_config(config, credentials);

        assert!(profiles.len() >= 4, "Expected at least 4 profiles, got {}", profiles.len());

        let dev = profiles.iter().find(|p| p.name == "dev").unwrap();
        assert_eq!(dev.region, Some("us-west-2".to_string()));
        assert_eq!(
            dev.sso_start_url,
            Some("https://my-sso.awsapps.com/start".to_string())
        );

        let default = profiles.iter().find(|p| p.name == "default").unwrap();
        assert_eq!(default.region, Some("us-east-1".to_string()));

        assert!(profiles.iter().any(|p| p.name == "production"));
    }

    #[test]
    fn test_aws_parse_ec2_json() {
        let json = r#"{
            "Reservations": [
                {
                    "Instances": [
                        {
                            "InstanceId": "i-0abcd1234efgh5678",
                            "InstanceType": "t3.micro",
                            "State": { "Name": "running" },
                            "PublicIpAddress": "54.123.45.67",
                            "PrivateIpAddress": "10.0.1.100",
                            "KeyName": "my-key",
                            "VpcId": "vpc-abc123",
                            "LaunchTime": "2024-01-15T10:30:00Z",
                            "Tags": [
                                { "Key": "Name", "Value": "web-server-1" }
                            ]
                        },
                        {
                            "InstanceId": "i-0efgh5678abcd1234",
                            "InstanceType": "m5.large",
                            "State": { "Name": "stopped" },
                            "PrivateIpAddress": "10.0.2.50",
                            "LaunchTime": "2024-02-20T08:00:00Z",
                            "Tags": []
                        }
                    ]
                }
            ]
        }"#;

        let instances = parse_ec2_instances(json).unwrap();

        assert_eq!(instances.len(), 2);

        assert_eq!(instances[0].id, "i-0abcd1234efgh5678");
        assert_eq!(instances[0].name, "web-server-1");
        assert_eq!(instances[0].state, "running");
        assert_eq!(instances[0].instance_type, "t3.micro");
        assert_eq!(instances[0].public_ip, Some("54.123.45.67".to_string()));
        assert_eq!(instances[0].private_ip, Some("10.0.1.100".to_string()));
        assert_eq!(instances[0].key_name, Some("my-key".to_string()));
        assert_eq!(instances[0].vpc_id, Some("vpc-abc123".to_string()));
        assert_eq!(instances[0].launch_time, "2024-01-15T10:30:00Z");

        assert_eq!(instances[1].id, "i-0efgh5678abcd1234");
        assert_eq!(instances[1].state, "stopped");
        assert_eq!(instances[1].public_ip, None);
    }

    #[test]
    fn test_aws_parse_s3_list() {
        let json = r#"{
            "Buckets": [
                {
                    "Name": "my-app-assets",
                    "CreationDate": "2023-06-15T12:00:00Z"
                },
                {
                    "Name": "my-app-logs",
                    "CreationDate": "2023-08-20T09:30:00Z"
                }
            ],
            "Owner": {
                "ID": "abc123"
            }
        }"#;

        let buckets = parse_s3_buckets(json).unwrap();

        assert_eq!(buckets.len(), 2);
        assert_eq!(buckets[0].name, "my-app-assets");
        assert_eq!(buckets[0].creation_date, "2023-06-15T12:00:00Z");
        assert_eq!(buckets[1].name, "my-app-logs");
        assert_eq!(buckets[1].creation_date, "2023-08-20T09:30:00Z");

        // Test s3 objects parsing too
        let obj_json = r#"{
            "Contents": [
                {
                    "Key": "images/logo.png",
                    "Size": 45678,
                    "LastModified": "2024-01-10T08:00:00Z",
                    "StorageClass": "STANDARD",
                    "ETag": "\"d41d8cd98f00b204e9800998ecf8427e\""
                },
                {
                    "Key": "docs/readme.md",
                    "Size": 1234,
                    "LastModified": "2024-02-15T14:30:00Z",
                    "StorageClass": "STANDARD_IA"
                }
            ]
        }"#;

        let objects = parse_s3_objects(obj_json).unwrap();
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].key, "images/logo.png");
        assert_eq!(objects[0].size, 45678);
        assert_eq!(objects[0].storage_class, "STANDARD");
        assert!(objects[0].etag.is_some());
        assert_eq!(objects[1].key, "docs/readme.md");
        assert_eq!(objects[1].storage_class, "STANDARD_IA");
    }
}
