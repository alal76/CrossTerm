---
title: "SSH Protocol Reference"
slug: ssh-protocol-reference
category: Protocols
keywords: [ssh, cipher, key exchange, port forwarding, proxy, agent, known_hosts]
schema_version: 1
---

# SSH Protocol Reference

This reference covers the SSH protocol capabilities supported by CrossTerm, including cipher suites, key exchange algorithms, authentication, port forwarding, and performance tuning.

## Cipher Suites

CrossTerm negotiates ciphers in preference order. The following ciphers are supported:

| Cipher | Key Size | Mode | Notes |
|--------|----------|------|-------|
| `chacha20-poly1305@openssh.com` | 256-bit | AEAD | Preferred. Constant-time, no padding oracle. |
| `aes256-gcm@openssh.com` | 256-bit | AEAD | Hardware-accelerated on AES-NI CPUs. |
| `aes128-gcm@openssh.com` | 128-bit | AEAD | Faster than AES-256 with slightly smaller key. |
| `aes256-ctr` | 256-bit | CTR | Widely compatible fallback. |
| `aes192-ctr` | 192-bit | CTR | Intermediate key size. |
| `aes128-ctr` | 128-bit | CTR | Maximum compatibility. |

AEAD ciphers (ChaCha20-Poly1305 and AES-GCM) provide both encryption and integrity in a single operation, eliminating the need for a separate MAC algorithm.

## Key Exchange Algorithms

Key exchange (KEX) establishes the shared session key. Supported algorithms in preference order:

| Algorithm | Type | Notes |
|-----------|------|-------|
| `curve25519-sha256` | ECDH | Default. Fast, constant-time, 128-bit security. |
| `curve25519-sha256@libssh.org` | ECDH | Alias for the above (legacy naming). |
| `ecdh-sha2-nistp256` | ECDH | NIST P-256 curve. FIPS 140-2 compliant. |
| `ecdh-sha2-nistp384` | ECDH | NIST P-384 curve. Higher security margin. |
| `diffie-hellman-group16-sha512` | DH | 4096-bit modulus. Fallback for legacy servers. |
| `diffie-hellman-group14-sha256` | DH | 2048-bit modulus. Minimum recommended. |

CrossTerm rejects `diffie-hellman-group1-sha1` and `diffie-hellman-group14-sha1` due to SHA-1 deprecation.

## Host Key Verification

CrossTerm uses Trust-On-First-Use (TOFU) for host key management. On first connection, the server's public key fingerprint is stored in `~/.crossterm/known_hosts`. Subsequent connections verify the key matches.

If the host key changes, CrossTerm displays a **host key mismatch warning** and refuses the connection to protect against MITM attacks. The user must manually remove the old entry before reconnecting.

Supported host key algorithms: `ssh-ed25519`, `ecdsa-sha2-nistp256`, `rsa-sha2-512`, `rsa-sha2-256`.

## Authentication Methods

### Password

Standard password authentication. Passwords can be stored in the encrypted Credential Vault.

### Public Key

Supported key formats: OpenSSH, PEM (PKCS#1/PKCS#8). Supported key types: Ed25519 (recommended), ECDSA (P-256, P-384), RSA (2048-bit minimum).

Encrypted private keys use the passphrase from the Vault or prompt at connection time.

### SSH Agent Forwarding

When enabled, CrossTerm forwards the local SSH agent socket to the remote host, allowing the remote session to authenticate to further hosts using the local agent's keys without storing private keys on the remote machine.

Agent forwarding uses the `SSH_AUTH_SOCK` environment variable on Unix systems and the OpenSSH agent pipe on Windows.

## ProxyJump (Jump Hosts)

CrossTerm supports multi-hop connections through one or more jump hosts. Each hop establishes an independent SSH session, then the next hop tunnels through the previous session's `direct-tcpip` channel.

Configuration requires host, port, username, and authentication for each jump host. CrossTerm connects sequentially through the chain before reaching the final destination.

## Port Forwarding

### Local Forwarding (`-L`)

Binds a local port and forwards connections through the SSH tunnel to a remote destination. Use cases: accessing remote databases, internal web services, or APIs behind a firewall.

### Remote Forwarding (`-R`)

Binds a port on the remote server and forwards incoming connections back through the tunnel to a local destination. Use cases: exposing a local development server to a remote network.

### Dynamic Forwarding (SOCKS5) (`-D`)

Creates a local SOCKS5 proxy. Applications configured to use this proxy have their traffic forwarded through the SSH tunnel. CrossTerm supports SOCKS5 address types:

- **0x01 (IPv4)**: 4-byte address + 2-byte port
- **0x03 (Domain)**: 1-byte length + domain string + 2-byte port
- **0x04 (IPv6)**: 16-byte address + 2-byte port

No SOCKS5 authentication is required (method `0x00`). The proxy opens a `direct-tcpip` channel for each connection.

## Performance Tuning

- **Keepalive interval**: Configurable interval (default 15s) for SSH keepalive packets. Prevents idle timeout disconnections from firewalls and NAT devices.
- **Keepalive max**: Maximum missed keepalives before disconnecting (default 3).
- **TCP_NODELAY**: Enabled by default to reduce latency for interactive sessions.
- **Compression**: zlib compression can be enabled for slow links. Not recommended for fast networks due to CPU overhead.
- **Channel window size**: Adjustable for bulk data transfer (SFTP). Larger windows improve throughput on high-latency links.

## Security Considerations

- All sessions use strict key exchange (`strict-kex`) when supported by the server.
- Passwords and key passphrases are held in `Zeroizing` memory and cleared after authentication.
- Host key verification prevents MITM attacks. Never disable host key checking in production.
- Agent forwarding should only be enabled to trusted servers, as a compromised server could use the forwarded agent.
