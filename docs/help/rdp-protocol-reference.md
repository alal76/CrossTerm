---
title: "RDP Protocol Reference"
slug: rdp-protocol-reference
category: Protocols
keywords: [rdp, remote desktop, NLA, clipboard, multi-monitor, RemoteApp]
schema_version: 1
---

# RDP Protocol Reference

This reference covers the Remote Desktop Protocol (RDP) capabilities supported by CrossTerm, including security modes, codec options, clipboard integration, and multi-monitor support.

## Connection Security

### Network Level Authentication (NLA)

NLA is the default and recommended authentication mode. It authenticates the user before establishing the full RDP session, reducing the attack surface of the remote host. NLA uses CredSSP (Credential Security Support Provider) which wraps NTLM or Kerberos authentication inside a TLS channel.

CrossTerm requires NLA for all connections by default. Legacy connections without NLA can be enabled per-session in advanced settings, but this is discouraged.

### TLS Transport

All RDP connections are encrypted using TLS 1.2 or 1.3. The TLS handshake occurs before any RDP-specific negotiation. Server certificate validation is enforced — self-signed certificates trigger a warning dialog showing the certificate fingerprint and expiration.

### RDP Security Layer (Legacy)

The original RDP encryption using RC4. This mode is considered insecure and is only available as a fallback for legacy Windows XP/Server 2003 hosts. CrossTerm displays a security warning when this mode is negotiated.

## Codec Options

CrossTerm supports multiple bitmap codecs for rendering the remote desktop:

| Codec | Compression | Notes |
|-------|-------------|-------|
| RemoteFX (RFX) | Progressive | Best quality. Hardware-accelerated decode. |
| NSCodec | Lossy | Good balance of quality and bandwidth. |
| Bitmap (RLE) | Lossless | Fallback. Higher bandwidth usage. |

### Color Depth

Supported: 32-bit (true color, default), 24-bit, 16-bit, and 8-bit. Lower color depths reduce bandwidth on constrained networks.

### Frame Rate

Configurable from 1–60 FPS. Default is 30 FPS. Lower frame rates reduce bandwidth and CPU usage. For productivity workloads (text, documents), 15 FPS is usually sufficient.

## Display Configuration

### Resolution

CrossTerm supports arbitrary resolutions up to 8192×8192 pixels per monitor. Resolution can be:

- **Fit to window**: Automatically scales to the CrossTerm pane size.
- **Fixed**: Set a specific resolution (e.g., 1920×1080).
- **Match local**: Uses the local monitor's native resolution.

### Multi-Monitor

CrossTerm supports spanning the remote desktop across multiple monitors. Each monitor is reported to the remote host with its geometry (position, size, DPI). The remote desktop extends across all selected monitors.

Configuration: Select which local monitors to use in the session properties dialog. Monitor layout is sent during connection negotiation.

### DPI Scaling

DPI-aware rendering ensures text and UI elements appear at the correct size on high-DPI displays. CrossTerm reports the local DPI to the server, which adjusts rendering accordingly.

## Clipboard Integration

Bidirectional clipboard sharing supports:

- **Text**: Plain text and rich text (RTF).
- **Files**: Drag-and-drop file transfer via clipboard redirection. Files are transferred over a virtual channel.
- **Images**: Bitmap clipboard content (e.g., screenshots).

Clipboard redirection can be disabled per-session for security-sensitive environments. File transfer size is limited to 2 GB per operation.

## Device Redirection

### Drive Mapping

Local drives or directories can be mapped into the remote session as network drives. This enables file transfer between local and remote without clipboard.

### Audio

Remote audio can be played locally (default), played on the remote host, or disabled. Audio recording redirection (microphone) is supported for VoIP applications.

### Printer

Local printers can be redirected to the remote session, allowing printing from remote applications to local printers.

## RemoteApp

RemoteApp mode launches individual applications from the remote host as if they were local windows, without showing the full remote desktop. Each RemoteApp window integrates with the local taskbar and window management.

Configuration requires the RemoteApp program path and optional command-line arguments. The remote server must have RemoteApp publishing configured.

## Performance Tuning

- **Bandwidth auto-detect**: CrossTerm negotiates codec and quality settings based on measured connection speed.
- **Persistent bitmap caching**: Caches frequently used bitmaps locally to reduce repeated transfers.
- **Font smoothing**: ClearType font smoothing can be disabled to reduce bandwidth.
- **Desktop composition**: Aero/DWM desktop composition can be disabled for lower bandwidth usage.
- **Reconnection**: Automatic reconnection attempts on network interruption with session state preservation.

## Security Considerations

- Always use NLA to prevent unauthenticated resource consumption on the remote host.
- Verify server certificates to protect against MITM attacks.
- Disable clipboard and drive redirection when connecting to untrusted servers.
- Use TLS 1.2+ exclusively. CrossTerm rejects SSL 3.0 and TLS 1.0/1.1.
