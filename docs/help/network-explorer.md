---
title: "Network Explorer"
slug: "network-explorer"
category: "tools"
order: 5
schema_version: 1
keywords: ["network", "scan", "port", "host", "subnet", "ping", "nmap", "mdns", "bonjour", "wifi", "wireless", "explore", "discovery"]
---

# Network Explorer

The Network Explorer scans your local network to discover hosts, open ports, service versions, TLS certificate details, and mDNS/Bonjour-advertised devices. It is intended for network diagnostics and auditing on networks you own or have permission to scan.

Open the Network Explorer from the **+** button → **Network Explorer**, or via **Connect → Network Explorer** in the macOS menu bar.

---

## Subnet Detection

When the Network Explorer opens it automatically detects all active local network interfaces and pre-populates the subnet field with the detected ranges (e.g. `192.168.1.0/24`). You can edit the subnet manually if needed.

Multiple subnets can be scanned by separating them with commas:
```
192.168.1.0/24, 10.0.0.0/24
```

---

## Scanning

Choose which well-known services to probe for under **Service Filters** (SSH, RDP, VNC, HTTP/HTTPS, databases, WinRM, MQTT, Kubernetes/Docker APIs, RTSP cameras, and more), plus any custom ports. A host is reported if **any** selected TCP port responds **or** if it answers an ICMP echo (ping) — so firewalled machines that block all probed ports but still respond to ping will still appear, even with an empty **Open Ports** column.

Alongside the port sweep, a concurrent mDNS/Bonjour browse listens for common service announcements (Home Assistant, Chromecast, AirPlay, Matter, printers, file shares, Spotify Connect, and more) and merges anything it finds into matching rows by IP address as it arrives.

1. Enter or confirm the subnet(s) in the input field.
2. Optionally expand **Service Filters** to change which ports are probed.
3. Click **Explore**. Results stream in as hosts respond — you do not need to wait for the scan to finish.
4. Click **Stop** at any point to cancel a scan in progress; hosts not yet started are skipped.

---

## Results

Each discovered host is shown as a row in a sortable table:

| Field | Description |
|-------|-------------|
| IP Address | IPv4 address of the host |
| Hostname | Best available name — DNS PTR, `/etc/hosts`, NetBIOS, mDNS, or a TLS certificate's CN/SAN |
| MAC Address | Hardware address, read from the OS's own ARP cache (LAN only) |
| Vendor | NIC manufacturer, resolved from a bundled offline IEEE OUI database with an online lookup as a fallback |
| Open Ports | Responding TCP ports, with a short version string where one could be captured (e.g. `22/ssh · OpenSSH 10.0p2`) |
| OS Guess | Best-effort device/OS guess from banner text, TLS certificate vendor, open-port heuristics, and ICMP TTL, in that priority order — not a raw-socket TCP/IP stack fingerprint |
| Latency | Round-trip response time in milliseconds |

Click the chevron next to a host's IP address to expand a detail panel with everything that wouldn't fit in the row: full banners, HTTP `Server` headers and page titles, TLS certificate subject/issuer/SAN/expiry, mDNS service records, and a plain-language evidence trail explaining how the hostname/OS/vendor were determined.

### Sorting and filtering

- Click any column header to sort by that field.
- Use the search box to filter by IP, hostname, MAC, vendor, or service name.
- Click a service-count badge above the table to filter to just that service.

### Connecting directly from results

Hosts with a connectable service (SSH, RDP, VNC, Telnet, SFTP, and more) show a **Connect** button that opens a new session tab pre-filled with that host's address and port.

### Exporting results

Click **Export** to save the currently filtered results to a JSON file, including every field captured above.

### Saving as sessions

Click **Save All as Sessions** to add every connectable discovered host to a "Discovered Hosts" group in your session list.

---

## WiFi Analysis (macOS)

On macOS, the **WiFi** tab displays all visible wireless networks using the CoreWLAN framework.

| Field | Description |
|-------|-------------|
| SSID | Network name |
| BSSID | Access point MAC address |
| RSSI | Signal strength in dBm (higher is better, e.g. −50 dBm is excellent) |
| Channel | 2.4 GHz or 5 GHz channel number |
| Band | 2.4 GHz or 5 GHz |
| Security | WPA2, WPA3, Open, etc. |
| Country | Regulatory country code |

Click **Refresh** to re-scan. The currently connected network is highlighted.

!!! note "macOS permission"
    The first time you use WiFi scanning, macOS may ask for location permission. This is required by Apple for apps that read WiFi details.

---

## Connection History

The Explorer keeps a running log of connection attempts made from scan results during the current session — target host, service, status, and timestamp. This is a session log of your own connect actions, not a persistent record of previously-seen devices across scans.

---

## Security & Ethics

The Network Explorer uses TCP connection attempts, ICMP probes, and standard-protocol handshakes (HTTP GET, TLS handshake, RTSP OPTIONS) to identify services — the same class of traffic an ordinary client generates. It never sends exploit payloads. Only scan networks you own or have explicit written permission to scan; unauthorized port scanning may be illegal in your jurisdiction and violates the terms of service of most cloud providers.
