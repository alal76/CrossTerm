#!/usr/bin/env swift
// CrossTerm WiFi scanner — uses CoreWLAN for real (non-redacted) SSIDs on macOS.
// Outputs JSON to stdout.

import CoreWLAN
import Foundation

struct ScanNetwork: Codable {
    let ssid: String
    let bssid: String
    let channel: Int
    let channel_width_mhz: Int
    let band: String
    let signal_dbm: Int
    let noise_dbm: Int
    let security: String
    let phy_mode: String?
    let is_current: Bool
}

struct ScanOutput: Codable {
    let networks: [ScanNetwork]
    let current_ssid: String?
    let interface_name: String?
}

func channelBand(_ ch: CWChannel?) -> String {
    guard let ch = ch else { return "unknown" }
    switch ch.channelBand {
    case .band2GHz: return "2.4GHz"
    case .band5GHz: return "5GHz"
    case .band6GHz: return "6GHz"
    @unknown default: return "unknown"
    }
}

func channelWidth(_ ch: CWChannel?) -> Int {
    guard let ch = ch else { return 20 }
    switch ch.channelWidth {
    case .width20MHz: return 20
    case .width40MHz: return 40
    case .width80MHz: return 80
    case .width160MHz: return 160
    @unknown default: return 20
    }
}

func securityString(_ net: CWNetwork) -> String {
    // Build security string from what CWNetwork exposes
    if net.supportsSecurity(.wpa3Personal) || net.supportsSecurity(.wpa3Enterprise) {
        return "WPA3"
    } else if net.supportsSecurity(.wpa2Personal) || net.supportsSecurity(.wpa2Enterprise) {
        if net.supportsSecurity(.wpaPersonal) || net.supportsSecurity(.wpaEnterprise) {
            return "WPA/WPA2"
        }
        return "WPA2"
    } else if net.supportsSecurity(.wpaPersonal) || net.supportsSecurity(.wpaEnterprise) {
        return "WPA"
    } else if net.supportsSecurity(.dynamicWEP) {
        return "WEP"
    } else if net.supportsSecurity(.none) {
        return "Open"
    }
    return "Unknown"
}

let client = CWWiFiClient.shared()
guard let iface = client.interface() else {
    let empty = ScanOutput(networks: [], current_ssid: nil, interface_name: nil)
    let data = try! JSONEncoder().encode(empty)
    print(String(data: data, encoding: .utf8)!)
    exit(0)
}

let currentSSID = iface.ssid()
let ifaceName = iface.interfaceName

var results: [ScanNetwork] = []

do {
    let scanned = try iface.scanForNetworks(withSSID: nil)
    for net in scanned {
        let ssid = net.ssid ?? ""
        let bssid = net.bssid ?? ""
        let ch = net.wlanChannel
        results.append(ScanNetwork(
            ssid: ssid,
            bssid: bssid,
            channel: ch?.channelNumber ?? 0,
            channel_width_mhz: channelWidth(ch),
            band: channelBand(ch),
            signal_dbm: net.rssiValue,
            noise_dbm: net.noiseMeasurement,
            security: securityString(net),
            phy_mode: nil,
            is_current: ssid == currentSSID && !ssid.isEmpty
        ))
    }
} catch {
    // Fall through with empty results
}

let output = ScanOutput(
    networks: results,
    current_ssid: currentSSID,
    interface_name: ifaceName
)

let encoder = JSONEncoder()
let data = try! encoder.encode(output)
print(String(data: data, encoding: .utf8)!)
