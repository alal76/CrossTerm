---
title: "Troubleshooting"
slug: "troubleshooting"
category: "support"
order: 5
schema_version: 1
keywords: ["error", "problem", "fix", "debug", "fail", "connect", "timeout", "refuse", "key", "auth", "firewall", "dns"]
---

# Troubleshooting

This guide covers common issues and their solutions when using CrossTerm.

## Connection Issues

### Authentication Failed

**Symptom:** "Authentication failed" or "Permission denied" error.

**Possible causes and solutions:**

1. **Wrong password**: Double-check your password. Passwords are case-sensitive.
2. **Wrong username**: Verify the username for the remote host.
3. **SSH key not accepted**: Ensure the public key is in the server's `~/.ssh/authorized_keys`.
4. **Key format mismatch**: CrossTerm supports OpenSSH, PEM, and PKCS#8 formats. Convert if needed.
5. **Passphrase required**: If your key has a passphrase, provide it in the credential or at the prompt.
6. **Server restrictions**: The server may not allow the authentication method you're using. Check `/etc/ssh/sshd_config`.

### Connection Timed Out

**Symptom:** Connection hangs and eventually shows "Connection timed out."

**Possible causes and solutions:**

1. **Host unreachable**: Verify the hostname or IP address. Try `ping hostname` in a local terminal.
2. **Wrong port**: SSH typically uses port 22. Verify the correct port with your administrator.
3. **Firewall blocking**: A firewall may be blocking the connection. Check both local and remote firewall rules.
4. **Network issues**: Check your internet connection. Try accessing other services.
5. **DNS resolution**: If using a hostname, ensure DNS resolves correctly. Try connecting by IP address.

### Connection Refused

**Symptom:** "Connection refused" error immediately.

**Possible causes and solutions:**

1. **SSH server not running**: The SSH daemon may not be running on the remote host.
2. **Wrong port**: The SSH server may be listening on a non-standard port.
3. **Firewall rules**: The port may be blocked by a firewall.
4. **Host-based restrictions**: The server may restrict connections by source IP.

### Host Key Verification Failed

**Symptom:** Warning about changed host key or verification failure.

**What this means:** The server's host key doesn't match what was previously recorded. This could mean:

- The server was reinstalled or upgraded.
- The server's SSH keys were regenerated.
- A potential man-in-the-middle attack.

**What to do:**

1. Verify with your system administrator that the key change is expected.
2. If confirmed safe, remove the old key and accept the new one.
3. If unexpected, do not connect and investigate further.

## Terminal Issues

### Terminal Not Rendering

**Symptom:** Terminal window is blank or characters display incorrectly.

**Solutions:**

1. Try resizing the terminal window.
2. Check Settings → Terminal → Font Family for a valid monospace font.
3. Toggle GPU Acceleration in Settings → Terminal.
4. Restart the terminal session.

### Copy/Paste Not Working

**Solutions:**

1. **Copy**: Select text and use Ctrl+Shift+C / ⌘C, or enable "Copy on Select" in Settings.
2. **Paste**: Use Ctrl+Shift+V / ⌘V.
3. Multi-line paste shows a confirmation dialog by default. Check Settings → Security → Paste Confirmation.

### Colors Look Wrong

**Solutions:**

1. Check your current theme in Settings → Appearance → Theme.
2. Try a different theme to see if the issue persists.
3. Verify the remote server's terminal type setting (`$TERM`).

## Performance Issues

### Slow Terminal Response

1. Check your network latency to the remote host.
2. Reduce scrollback buffer in Settings → Terminal → Scrollback Lines.
3. Disable GPU Acceleration if it's causing issues.
4. Close unused tabs to free resources.

### High Memory Usage

1. Reduce scrollback buffer size.
2. Close idle sessions you're not using.
3. Large file transfers via SFTP can temporarily increase memory usage.

## Vault Issues

### Forgot Master Password

Unfortunately, the master password cannot be recovered. You will need to:

1. Delete the vault data file.
2. Create a new vault.
3. Re-enter all credentials.

### Vault Won't Unlock

1. Ensure Caps Lock is not enabled.
2. Check if you're using the correct profile (each profile can have its own vault).
3. Try restarting CrossTerm.

## Getting Help

If your issue is not covered here:

1. Check the other help articles for feature-specific guidance.
2. Visit the CrossTerm issue tracker to report bugs.
3. Include the following when reporting issues: CrossTerm version, OS version, and steps to reproduce.
