---
title: "Security Guide"
slug: "security-guide"
category: "security"
order: 5
schema_version: 1
keywords: ["security", "vault", "encrypt", "aes", "argon2", "password", "master", "known hosts", "cipher", "credential", "safe"]
---

# Security Guide

CrossTerm is designed with security as a core principle. This guide covers the encryption model, credential storage, SSH security, and best practices for keeping your connections safe.

## Credential Vault Encryption

### Encryption Algorithm

All credentials stored in the vault are encrypted using **AES-256-GCM** (Advanced Encryption Standard with 256-bit keys in Galois/Counter Mode). This provides both confidentiality and authenticity — any tampering with encrypted data is detected.

- **Algorithm:** AES-256-GCM
- **Key Size:** 256 bits
- **Nonce:** 96-bit random nonce per encryption operation
- **Tag Size:** 128-bit authentication tag

### Key Derivation

The encryption key is derived from your master password using **Argon2id**, the winner of the Password Hashing Competition. Argon2id combines:

- **Memory-hardness** — Requires significant RAM, making GPU/ASIC attacks expensive
- **Time-hardness** — Multiple iterations increase computation cost
- **Side-channel resistance** — The "id" variant resists timing attacks

Default parameters:
- Memory: 64 MiB
- Iterations: 3
- Parallelism: 4
- Salt: 16 bytes (randomly generated per vault)
- Output: 32 bytes (256-bit key)

### Memory Protection

Key material is held in Rust's `Zeroizing<Vec<u8>>` wrapper, which guarantees the key is overwritten with zeros when it goes out of scope. This prevents key material from lingering in memory after the vault is locked.

### No Backdoor

There is no password recovery mechanism. If you forget your master password, the vault contents **cannot** be recovered. This is by design — it ensures that even the application developers cannot access your credentials.

## Master Password Best Practices

Choose a strong master password:

1. **Length** — Use at least 12 characters. Longer is better.
2. **Complexity** — Mix uppercase, lowercase, numbers, and symbols.
3. **Uniqueness** — Never reuse your master password elsewhere.
4. **Passphrase** — Consider a random multi-word passphrase (e.g., "correct-horse-battery-staple").
5. **Password Manager** — Store your master password in a separate password manager as a backup.

### Auto-Lock

Configure automatic vault locking from **Settings** → **Security** → **Vault Auto-Lock**. Set the idle timeout in minutes (0 = never auto-lock). The default is 15 minutes of inactivity.

### Clipboard Auto-Clear

When you copy a password from the vault, CrossTerm can automatically clear your clipboard after a configurable delay. Set this from **Settings** → **Security** → **Clipboard Auto-Clear** (seconds, 0 = never).

## SSH Security

### Known Hosts Verification

CrossTerm verifies SSH server fingerprints against a known hosts database:

1. **First Connection** — You are prompted to accept or reject the server's host key fingerprint.
2. **Subsequent Connections** — The fingerprint is compared against the stored value. If it changes, CrossTerm warns you of a potential man-in-the-middle attack.
3. **Storage** — Known hosts are stored locally per profile, isolated from other profiles.

### Cipher Policy

CrossTerm supports modern SSH ciphers and key exchange algorithms:

**Ciphers (in order of preference):**
- `chacha20-poly1305@openssh.com`
- `aes256-gcm@openssh.com`
- `aes128-gcm@openssh.com`
- `aes256-ctr`
- `aes192-ctr`
- `aes128-ctr`

**Key Exchange:**
- `curve25519-sha256`
- `curve25519-sha256@libssh.org`
- `ecdh-sha2-nistp256`
- `ecdh-sha2-nistp384`
- `diffie-hellman-group16-sha512`
- `diffie-hellman-group14-sha256`

**Host Key Algorithms:**
- `ssh-ed25519`
- `ecdsa-sha2-nistp256`
- `ecdsa-sha2-nistp384`
- `rsa-sha2-512`
- `rsa-sha2-256`

Legacy algorithms (e.g., `arcfour`, `blowfish-cbc`, `diffie-hellman-group1-sha1`) are **not** supported.

### SSH Key Types

CrossTerm supports the following SSH key types for authentication:

- **Ed25519** (recommended) — Fast, secure, compact keys
- **ECDSA** (P-256, P-384, P-521) — Elliptic curve keys
- **RSA** (2048-bit minimum) — Legacy compatibility; 4096-bit recommended

### Agent Forwarding

SSH agent forwarding allows you to use your local SSH keys on remote servers without copying the keys. Enable it per-session in the connection settings. **Caution:** Only enable agent forwarding on trusted servers, as a compromised server could use your forwarded agent.

## Credential Storage

### Supported Credential Types

The vault stores six types of credentials:

| Type | Use Case |
|------|----------|
| Password | Username/password authentication |
| SSH Key | Key-based SSH authentication |
| Certificate | X.509 certificate authentication |
| API Token | Service tokens (GitHub, AWS, etc.) |
| Cloud Credential | AWS/Azure/GCP access keys |
| TOTP Seed | Time-based one-time passwords |

### Sensitive Field Handling

- Sensitive fields (passwords, private keys, tokens) are marked with `#[serde(skip_serializing)]` and never written to logs or exported.
- All identifiers use UUID v4 for unpredictability.
- Credentials are encrypted individually — compromising one does not expose others.

## Audit Trail

CrossTerm logs security-relevant events to the Audit Log:

- Vault unlock / lock events
- Credential access (read, create, update, delete)
- SSH connection attempts and authentication results
- Port forwarding setup and teardown
- Profile switches

View the audit log from the **Bottom Panel** → **Audit Log** tab. Logs are stored locally and scoped per profile.

## Security Checklist

- [ ] Use a strong, unique master password (12+ characters)
- [ ] Enable vault auto-lock (15 minutes or less)
- [ ] Enable clipboard auto-clear for copied passwords
- [ ] Use Ed25519 SSH keys where possible
- [ ] Verify SSH host key fingerprints on first connection
- [ ] Only enable SSH agent forwarding on trusted hosts
- [ ] Run `cargo audit` and `npm audit` regularly
- [ ] Keep CrossTerm updated for security patches
