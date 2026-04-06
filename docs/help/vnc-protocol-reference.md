---
title: "VNC Protocol Reference"
slug: vnc-protocol-reference
category: Protocols
keywords: [vnc, TLS, encoding, clipboard, tight, ultra, zrle]
schema_version: 1
---

# VNC Protocol Reference

This reference covers the VNC (Virtual Network Computing) protocol capabilities supported by CrossTerm, including security modes, encoding types, clipboard handling, and performance tuning.

## Connection Modes

### Direct Connection

Connect directly to a VNC server by specifying hostname and port. The default VNC port is 5900 (display :0). Display number N maps to port 5900+N.

### Reverse Connection (Listening Mode)

CrossTerm can listen for incoming VNC connections. The remote server initiates the connection to CrossTerm on a specified port (default 5500). Useful when the VNC server is behind a firewall.

### Gateway/Repeater

Connect through a VNC repeater or gateway by specifying the repeater address and the target server's ID. The repeater routes the connection to the correct backend server.

## Security Modes

### TLS Encryption

| Mode | Authentication | Encryption | Notes |
|------|---------------|------------|-------|
| VeNCrypt TLS + X.509 | Certificate | TLS 1.2+ | Strongest. Mutual authentication. |
| VeNCrypt TLS + Password | VNC password | TLS 1.2+ | Encrypted channel with password auth. |
| VeNCrypt Plain TLS | None | TLS 1.2+ | Encrypted but unauthenticated. |
| TLS Anonymous | None | TLS (anon DH) | Vulnerable to MITM. Not recommended. |

### VNC Authentication

Standard VNC password authentication uses DES-based challenge-response. The password is truncated to 8 characters. This mode is insecure without TLS wrapping — the traffic is unencrypted and the DES key space is small.

CrossTerm warns when connecting with VNC authentication over an unencrypted channel.

### No Authentication

Some VNC servers allow unauthenticated access. CrossTerm displays a prominent security warning when connecting without authentication.

## Encoding Types

Encodings determine how screen updates are compressed and transmitted. CrossTerm negotiates encodings in preference order:

| Encoding | Type | Best For |
|----------|------|----------|
| Tight | Lossy/Lossless | General use. JPEG for photos, zlib for text. |
| ZRLE (Zlib Run-Length) | Lossless | Good compression, moderate CPU. |
| Ultra | Lossy | Low bandwidth. Aggressive compression. |
| Hextile | Lossless | Legacy. Low CPU, moderate bandwidth. |
| RRE (Rise-and-Run-length) | Lossless | Simple scenes with large solid areas. |
| Raw | None | LAN only. Highest bandwidth, lowest CPU. |
| CopyRect | N/A | Window moves and scrolling. Always enabled. |

### Tight Encoding Details

Tight encoding uses a combination of techniques:

- **JPEG compression** for photographic regions (quality 1–9, configurable).
- **Zlib compression** for regions with few colors (text, UI elements).
- **Palette encoding** for regions with very few distinct colors.
- **Gradient filter** for smooth gradients.

JPEG quality can be set per-session: lower quality (1–3) for constrained links, higher quality (7–9) for LAN connections.

### Ultra Encoding

Ultra encoding applies LZO compression with optional lossy preprocessing. It provides the highest compression ratio at the cost of image quality. Suitable for very low bandwidth connections (< 1 Mbps).

## Pixel Format

CrossTerm negotiates pixel format with the server:

- **True color (32-bit)**: Default. Full color fidelity.
- **16-bit**: Reduces bandwidth by ~50% with minor color loss.
- **8-bit**: Palette mode. Maximum bandwidth savings, significant color reduction.

The bits-per-pixel, depth, and color channel masks are sent during protocol initialization.

## Clipboard Integration

VNC clipboard uses the `ServerCutText` and `ClientCutText` messages for bidirectional text transfer. Limitations:

- **Text only**: No file or image clipboard support in standard VNC.
- **Latin-1 encoding**: Standard VNC clipboard is limited to ISO 8859-1. Extended clipboard (if supported by server) enables UTF-8.
- **No automatic sync**: Clipboard is transferred on explicit copy/paste operations.

## Input Handling

### Keyboard

CrossTerm translates local key events to X11 keysyms for transmission. Special handling for modifier keys (Ctrl, Alt, Super) ensures correct behavior across operating systems. Dead keys and compose sequences are supported for international input.

### Mouse

All mouse buttons (left, middle, right, scroll up/down) are transmitted. Scroll events are mapped to buttons 4/5. Cursor position is sent as absolute coordinates.

## Performance Tuning

- **Encoding selection**: Use Tight or ZRLE for WAN. Use Raw for gigabit LAN.
- **JPEG quality**: Lower values (1–3) for slow links. Higher (7–9) for LAN.
- **Color depth**: Reduce to 16-bit or 8-bit for constrained bandwidth.
- **Update request rate**: Configurable continuous update interval. Lower rates reduce CPU and bandwidth.
- **Compression level**: Zlib compression level (1–9) trades CPU for bandwidth. Level 6 is default.
- **Cursor handling**: Local cursor rendering eliminates cursor lag on high-latency connections.

## Security Considerations

- Always use VeNCrypt with TLS for connections over untrusted networks.
- Standard VNC authentication without TLS exposes the password hash and all session data.
- Disable clipboard sharing when connecting to untrusted servers.
- VNC passwords are limited to 8 characters. Use TLS client certificates for stronger authentication.
