---
title: "Credential Vault"
slug: "credential-vault"
category: "security"
order: 4
schema_version: 1
keywords: ["vault", "credential", "password", "key", "security", "encrypt", "lock", "master", "store", "secret"]
---

# Credential Vault

The Credential Vault is CrossTerm's encrypted storage for passwords, SSH keys, API tokens, and other sensitive credentials. All data is encrypted with AES-256-GCM using a master password.

## Setting Up the Vault

On first launch, you are prompted to create a master password:

1. Choose a strong password (minimum 8 characters).
2. Confirm the password.
3. The vault is created and unlocked.

**Important:** There is no password recovery mechanism. If you forget your master password, vault contents cannot be recovered.

## Unlocking the Vault

When you start CrossTerm, the vault is locked by default:

1. Click the lock icon in the sidebar or status bar.
2. Enter your master password.
3. The vault unlocks, and credentials become available for connections.

## Storing Credentials

### Password Credentials

Store username/password pairs:

1. Open the vault (sidebar → Vault icon).
2. Click **Add Credential**.
3. Select type **Password**.
4. Enter a name, username, and password.
5. Click **Save**.

### SSH Keys

Store private keys for SSH authentication:

1. Click **Add Credential** → **SSH Key**.
2. Enter a descriptive name.
3. Paste your private key content.
4. Optionally add the passphrase.
5. Click **Save**.

### API Tokens

Store tokens for cloud services and APIs:

1. Click **Add Credential** → **API Token**.
2. Enter the provider name and token value.
3. Optionally set an expiry date.
4. Click **Save**.

### Cloud Credentials

Store AWS, Azure, or GCP access keys:

1. Click **Add Credential** → **Cloud Credential**.
2. Select the cloud provider.
3. Enter access key, secret key, and optional region.
4. Click **Save**.

## Using Credentials

When creating or editing a session:

1. In the authentication section, click the key icon.
2. A dropdown shows matching credentials from the vault.
3. Select a credential to auto-fill the authentication fields.

Credentials are referenced by ID — if you update a credential, all sessions using it get the new values automatically.

## Auto-Lock

For security, the vault automatically locks after a period of inactivity:

- **Default**: 15 minutes.
- **Configurable**: Settings → Security → Vault Auto-Lock.
- **Manual lock**: Click the lock icon or use Command Palette → Lock Vault.
- Setting auto-lock to 0 disables automatic locking.

## Changing the Master Password

1. Open Settings → Security.
2. Click **Change Master Password**.
3. Enter your current password.
4. Enter and confirm your new password.
5. All credentials are re-encrypted with the new key.

## Clipboard Security

When copying passwords from the vault:

- Passwords are automatically cleared from the clipboard after a configurable time.
- **Default**: 30 seconds.
- **Configurable**: Settings → Security → Clipboard Auto-Clear.
- Setting to 0 disables auto-clear.

## Security Details

- **Encryption**: AES-256-GCM with PBKDF2 key derivation.
- **Key material**: Held in memory with zeroize-on-drop protection.
- **No plaintext storage**: Credentials are never written to disk unencrypted.
- **Audit logging**: All vault access events are recorded in the audit log.
- **Sensitive fields**: Passwords and key material are excluded from serialization and export.
