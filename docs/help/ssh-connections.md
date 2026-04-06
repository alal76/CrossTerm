---
title: "SSH Connections"
slug: "ssh-connections"
category: "connections"
order: 2
schema_version: 1
keywords: ["ssh", "connect", "remote", "key", "password", "jump", "proxy", "port", "forward", "agent", "host"]
---

# SSH Connections

CrossTerm provides a full-featured SSH client with support for modern authentication methods, jump hosts, port forwarding, and agent forwarding.

## Authentication Methods

### Password Authentication

The simplest method. Enter your username and password when connecting. Passwords can be stored in the Credential Vault for quick access.

1. Open Quick Connect (**Ctrl+Shift+N** / **⌘⇧N**).
2. Enter hostname, port, and username.
3. Select **Password** authentication.
4. Enter your password or choose a saved credential.

### SSH Key Authentication

More secure than passwords. CrossTerm supports OpenSSH, PEM, and PKCS#8 key formats.

1. Open Quick Connect or create a new session.
2. Select **SSH Key** authentication.
3. Provide the path to your private key, or paste the key content.
4. If your key has a passphrase, enter it or use a vault credential.

**Supported key types:**
- RSA (2048-bit and above)
- Ed25519 (recommended)
- ECDSA (P-256, P-384, P-521)

### Certificate Authentication

For organizations using SSH certificates:

1. Select **Certificate** authentication.
2. Provide your certificate and matching private key.
3. Supports PEM and PKCS#12 formats.

## Jump Hosts (ProxyJump)

Connect through an intermediate server when direct access is not possible:

1. Edit your session settings.
2. Under **Advanced**, enable **Jump Host**.
3. Enter the jump host address, port, and credentials.
4. CrossTerm establishes the hop automatically.

You can chain multiple jump hosts for complex network topologies.

## Port Forwarding

### Local Forwarding

Forward a port from your local machine to a remote destination through the SSH tunnel:

- **Use case**: Access a remote database on `db-server:5432` through your SSH connection.
- **Configuration**: Local port 15432 → Remote `db-server:5432`.
- Connections to `localhost:15432` are tunneled to `db-server:5432` via the SSH server.

### Remote Forwarding

Expose a local service to the remote host:

- **Use case**: Let the remote server access your local dev server on port 3000.
- **Configuration**: Remote port 8080 → Local `localhost:3000`.

### Dynamic Forwarding (SOCKS Proxy)

Create a SOCKS5 proxy through the SSH tunnel:

- **Use case**: Route all browser traffic through the SSH server.
- Allocates a local SOCKS port that proxies traffic through the remote host.

## Agent Forwarding

Allow the remote server to use your local SSH keys for onward connections:

1. In session settings, enable **Agent Forwarding**.
2. Your local SSH agent (or CrossTerm's built-in agent) will be forwarded.
3. On the remote host, you can SSH to other servers using your local keys.

**Security note:** Only enable agent forwarding to trusted servers.

## Keep-Alive Settings

To prevent idle disconnections:

- **Keep Alive Interval**: Sends a packet every N seconds (default: 60).
- Configure per-session or globally in Settings → SSH.

## Host Key Verification

On first connection, CrossTerm displays the server's host key fingerprint. You can:

- **Accept**: Trust this key and save it.
- **Reject**: Cancel the connection.
- **Accept Once**: Connect without saving the key.

If a previously saved host key changes, CrossTerm shows a warning. This may indicate a server reinstallation or a potential security issue.

## Connection Troubleshooting

If you have trouble connecting, see the [Troubleshooting](troubleshooting) guide.
