use super::*;

// ── BE-MOD-02: SSH key generation and listing ───────────────────────────

#[tauri::command]
pub async fn ssh_generate_key(
    key_type: String,
    comment: Option<String>,
    passphrase: Option<String>,
    output_path: String,
) -> Result<String, SshError> {
    use ssh_key::{Algorithm, LineEnding, private::PrivateKey};

    let algorithm = match key_type.to_lowercase().as_str() {
        "ed25519" => Algorithm::Ed25519,
        "rsa" => Algorithm::Rsa { hash: None },
        "ecdsa" | "ecdsa-p256" => Algorithm::Ecdsa {
            curve: ssh_key::EcdsaCurve::NistP256,
        },
        other => return Err(SshError::Key(format!("Unsupported key type: {}", other))),
    };

    let private_key = PrivateKey::random(&mut rand::rngs::OsRng, algorithm)
        .map_err(|e| SshError::Key(e.to_string()))?;

    let output = std::path::Path::new(&output_path);
    if let Some(parent) = output.parent() {
        tokio::fs::create_dir_all(parent).await
            .map_err(|e| SshError::Io(e.to_string()))?;
    }

    // Write private key
    let private_pem = if let Some(ref pass) = passphrase {
        private_key
            .encrypt(&mut rand::rngs::OsRng, pass)
            .map_err(|e| SshError::Key(e.to_string()))?
            .to_openssh(LineEnding::LF)
            .map_err(|e| SshError::Key(e.to_string()))?
    } else {
        private_key
            .to_openssh(LineEnding::LF)
            .map_err(|e| SshError::Key(e.to_string()))?
    };

    tokio::fs::write(&output_path, private_pem.as_bytes()).await
        .map_err(|e| SshError::Io(e.to_string()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&output_path, std::fs::Permissions::from_mode(0o600)).await
            .map_err(|e| SshError::Io(e.to_string()))?;
    }

    // Write public key
    let public_key = private_key.public_key();
    let mut pub_openssh = public_key
        .to_openssh()
        .map_err(|e| SshError::Key(e.to_string()))?;
    if let Some(ref c) = comment {
        if !c.is_empty() {
            pub_openssh = format!("{} {}", pub_openssh.trim(), c);
        }
    }
    let pub_path = format!("{}.pub", output_path);
    tokio::fs::write(&pub_path, format!("{}\n", pub_openssh)).await
        .map_err(|e| SshError::Io(e.to_string()))?;

    let fingerprint = public_key.fingerprint(ssh_key::HashAlg::Sha256).to_string();
    Ok(fingerprint)
}

#[tauri::command]
pub async fn ssh_list_keys(
    directory: String,
) -> Result<Vec<String>, SshError> {
    let dir = std::path::Path::new(&directory);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut keys = Vec::new();
    let mut entries = tokio::fs::read_dir(&directory).await
        .map_err(|e| SshError::Io(e.to_string()))?;
    while let Some(entry) = entries.next_entry().await
        .map_err(|e| SshError::Io(e.to_string()))? {
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "pub" {
                keys.push(path.to_string_lossy().to_string());
            }
        }
    }
    keys.sort();
    Ok(keys)
}
