# Vault Sharing — Threat Model

**Version:** 1.0  
**Date:** 2026-05-05  
**Scope:** `vault/shared.rs` — DEK sharing, envelope encryption, DEK rotation,
recording encryption for reviewers.

---

## 1. Assets

| Asset | Sensitivity | Description |
|---|---|---|
| Vault DEK | Critical | 32-byte AES-256 key that directly encrypts all credentials in the vault |
| Credentials in vault | Critical | Passwords, SSH keys, API tokens stored AES-256-GCM-encrypted under the DEK |
| Sharing envelopes | High | X25519/AES-GCM-wrapped copies of the DEK, one per authorised recipient |
| User KEK | Critical | 32-byte Argon2id-derived key that encrypts the user's long-term private key at rest |
| User long-term private key | High | X25519 static secret; exposure lets an attacker open all past and future envelopes addressed to that user |
| Reviewer key pair | High | Ephemeral X25519 key pair used to encrypt session recordings; private key encrypted under reviewer KEK |
| Vault sharing manifest | Medium | JSON listing all authorised recipients; integrity loss enables silent re-addition of revoked users |
| Encrypted session recordings | Medium | AES-256-GCM blobs; can reveal terminal output if decrypted |

---

## 2. Trust Boundaries

```
┌─────────────────────────────────────────────────────────────────┐
│  User's Device (trusted)                                        │
│  ┌──────────────┐   ┌──────────────────────────────────────┐   │
│  │  Tauri/Rust  │   │  WebView (frontend JS)               │   │
│  │  backend     │◄──┤  — never sees raw DEK or private key │   │
│  └──────────────┘   └──────────────────────────────────────┘   │
│        │                                                        │
│  ┌─────▼──────────────────┐                                     │
│  │  Disk (semi-trusted)   │  manifests, encrypted vault DB     │
│  │  OS keychain optional  │                                     │
│  └────────────────────────┘                                     │
└────────────────────────┬────────────────────────────────────────┘
                         │ network (untrusted)
┌────────────────────────▼────────────────────────────────────────┐
│  Peer Devices (untrusted until envelope opened)                 │
│  — receive only public keys and encrypted envelopes             │
└─────────────────────────────────────────────────────────────────┘
```

**Boundary crossings that carry sensitive material:**

- Frontend → backend: KEK (base64) passed as Tauri IPC argument — stays in process memory only.
- Backend → disk: only ciphertext and manifests are persisted; plaintext DEK and private keys never touch disk.
- Backend → peer: only public keys and `SharedEnvelope` JSON leave the device.

---

## 3. Threat Actors

| Actor | Capability |
|---|---|
| Network attacker (passive) | Packet capture on LAN/WAN; cannot modify traffic |
| Network attacker (active / MITM) | Can intercept and modify network messages before TLS is established |
| Compromised peer device | Has full file-system access on the peer; may hold a legitimately issued envelope |
| Malicious plugin / third-party extension | Runs inside the same process as the frontend; can call Tauri commands |
| Physical access attacker | Can read disk, take memory dumps if device is unlocked |
| Malicious insider (revoked user) | Was once a legitimate recipient; retains an old copy of the DEK |

---

## 4. Threat Table

| # | STRIDE | Threat | Affected Asset | Mitigation | Residual Risk |
|---|---|---|---|---|---|
| T1 | Spoofing | Attacker substitutes their own public key during vault-share flow, receiving a valid envelope for the real DEK | Sharing envelope / DEK | Recipient public keys must be verified out-of-band (e.g., fingerprint comparison or signed profile); `create_sharing_envelope` trusts the key it is given | Medium — no built-in PKI; relies on user verification |
| T2 | Tampering | Manifest JSON is modified on disk to re-add a revoked recipient's public key | Vault sharing manifest | Manifest should be stored with an HMAC or signed by the owner's key; current design does not sign the manifest | High — deferred (see §6) |
| T3 | Tampering | Encrypted recording payload byte-flip causes wrong data to be decrypted | Encrypted session recordings | AES-256-GCM authentication tag detects any single-byte modification; `decrypt_recording_for_reviewer` returns `Err` on tag mismatch | Low |
| T4 | Repudiation | A user denies having shared vault access with a peer | Sharing envelopes | Envelopes embed `recipient_public_key`; audit log of `vault_share_with` calls should be stored by the application layer | Medium — audit log not enforced in this module |
| T5 | Information Disclosure | Private key extracted from memory dump while vault is unlocked | User long-term private key | Private key is only materialised in stack memory inside `open_sharing_envelope`; zeroisation on drop (x25519-dalek `StaticSecret` zeroises on drop) | Low |
| T6 | Information Disclosure | Old DEK copy retained by revoked user used to decrypt vault contents | Vault DEK | `vault_rotate_dek` generates a new DEK and re-encrypts all remaining envelopes; vault data must be re-encrypted by caller using the new DEK | Medium — vault data re-encryption is caller's responsibility |
| T7 | Denial of Service | Manifest envelopes are deleted, locking all users out of the vault | Vault sharing manifest | Manifest should be backed up; vault owner should retain a local envelope encrypted under their own password-derived KEK | Medium — backup policy is application-layer |
| T8 | Elevation of Privilege | Malicious plugin calls `vault_generate_reviewer_keypair` or `vault_encrypt_recording` with an attacker-controlled KEK, creating a backdoor envelope | Reviewer key pair | Tauri command allowlisting limits which frontend origins may invoke commands; KEK is derived from the user's master password and never hardcoded | Low if allowlist is configured; Medium otherwise |
| T9 | Information Disclosure | Ephemeral DEK for a recording is reused across sessions, enabling correlation attacks | Encrypted session recordings | `encrypt_recording_for_reviewer` generates a fresh 32-byte DEK via `OsRng` for each call | Low |
| T10 | Tampering | Envelope nonce reuse under the same AES key breaks GCM confidentiality | Sharing envelope / DEK | Nonces are generated with `OsRng.fill_bytes` (cryptographically random); nonce collision probability is negligible for 12-byte nonces | Low |

---

## 5. Cryptographic Assumptions

### Argon2id (KEK derivation — caller's responsibility)
- Recommended parameters: `m = 65536` KiB, `t = 3` iterations, `p = 4` lanes.
- Security relies on adversary being unable to run many parallel instances; these parameters should be tuned to ≥ 0.5 s on the deployment device.

### AES-256-GCM
- Provides authenticated encryption; any modification to ciphertext or AAD causes decryption to fail.
- Nonce size is 96 bits (12 bytes); nonces are generated fresh from `OsRng` for every encryption call, making collision probability ≈ 2⁻⁹⁶ per pair of messages.
- Key size is 256 bits; no known practical attacks against AES-256.

### X25519 Diffie-Hellman
- Based on Curve25519; provides ~128-bit security against classical attacks.
- Ephemeral secrets (`EphemeralSecret`) are used for envelope creation, providing forward secrecy: compromise of the long-term private key does not expose past envelope sessions.
- `StaticSecret` is used for user identity keys and reviewer keys; these are stored encrypted under the user's KEK.

### SHA-256 (KDF)
- Used to derive a 32-byte AES key from the raw 32-byte X25519 shared secret: `aes_key = SHA-256(shared_secret)`.
- This is a one-way compression; the shared secret is never directly exposed.

---

## 6. Known Limitations and Deferred Mitigations

| Item | Description | Planned Mitigation |
|---|---|---|
| No manifest signing | The `VaultSharingManifest` JSON is not signed; a local attacker with write access to disk can add or remove envelopes silently | Sign manifest with owner's long-term key; verify signature before applying any change |
| DEK rotation frequency | There is no enforced rotation schedule; a stale DEK remains valid indefinitely | Application layer should trigger rotation after revocation events and on a configurable time-based policy |
| YubiKey / CTAP2 support | KEK derivation relies solely on Argon2id + password; hardware token binding is not implemented | Integrate CTAP2 / WebAuthn resident-credential flow to bind KEK to hardware token presence |
| Sentry / telemetry | Error messages passed to `map_err(|e| e.to_string())` may be forwarded to Sentry; these must not contain key material | Audit all error paths before enabling telemetry; scrub key bytes from error strings |
| No AAD binding | AES-GCM calls do not pass additional authenticated data (AAD) such as a vault ID or recipient identity | Add vault-scoped AAD to prevent cross-vault envelope transplant attacks |
| Memory zeroisation | Intermediate plaintext DEK and private key bytes may persist in heap allocations until the allocator reclaims them | Use `zeroize` crate for all sensitive byte vectors |
