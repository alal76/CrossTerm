---
title: "Network Explorer"
slug: "network-explorer"
category: "tools"
order: 5
schema_version: 1
keywords: ["network", "scan", "port", "host", "subnet", "ping", "nmap", "wifi", "wireless", "explore", "discovery"]
---

# Network Explorer

The Network Explorer scans your local network to discover hosts, open ports, running services, and wireless access points. It is intended for network diagnostics and auditing on networks you own or have permission to scan.

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

### Quick Scan

Scans the most common 100 ports across all hosts in the subnet. Fast, suitable for an overview of a typical home or office network.

- **Estimated time:** 5–30 seconds for a /24 subnet.

### Full Scan

Scans all 65 535 TCP ports on discovered hosts. Much slower but reveals unusual services.

- **Estimated time:** Several minutes for a /24 subnet.

### Starting a scan

1. Enter or confirm the subnet in the input field.
2. Click **Quick Scan** or **Full Scan**.
3. Results stream in as hosts respond — you do not need to wait for the full scan to finish.

Use the **Stop** button to abort a scan in progress.

---

## Results

Each discovered host is shown as a card with:

| Field | Description |
|-------|-------------|
| IP Address | IPv4 address of the host |
| Hostname | Reverse-DNS lookup result (if available) |
| MAC Address | Hardware address (ARP, LAN only) |
| Vendor | NIC manufacturer derived from the MAC OUI |
| Open Ports | List of responding TCP ports |
| Services | Inferred service names (SSH, HTTP, HTTPS, RDP, etc.) |
| Latency | Round-trip ping time in milliseconds |
| OS Guess | Operating system fingerprint (best-effort) |

### Sorting and filtering

- Click any column header to sort by that field.
- Use the search box to filter by IP, hostname, or service name.

### Connecting directly from results

Clicking **SSH** or **RDP** next to an open port immediately opens a Quick Connect dialog pre-filled with that host's IP and the detected port.

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

## Exporting Results

Click **Export** to save scan results:

- **JSON** — machine-readable, includes all fields per host.
- **CSV** — spreadsheet-compatible, one row per host/port combination.

---

## Connection History

The Explorer tracks every host you have previously connected to or scanned. Previously seen hosts are flagged in results so you can quickly identify new or unexpected devices.

---

## Security & Ethics

The Network Explorer uses raw TCP connection attempts and ICMP probes. Only scan networks you own or have explicit written permission to scan. Unauthorized port scanning may be illegal in your jurisdiction and violates the terms of service of most cloud providers.

CrossTerm's scan engine never sends exploit payloads — it only opens and closes TCP connections and sends standard ICMP echo requests.
