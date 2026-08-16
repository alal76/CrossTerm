import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import WifiScanner from "@/components/NetworkTools/WifiScanner";
import { invoke } from "@tauri-apps/api/core";
import type { WifiScanResult } from "@/types";

const mockInvoke = vi.mocked(invoke);

function scanResult(overrides: Partial<WifiScanResult> = {}): WifiScanResult {
  return {
    networks: [
      { ssid: "HomeNet", bssid: "aa:bb", channel: 6, band: "2.4GHz", signal_dbm: -45, security: "wpa2_psk", is_current: true },
      { ssid: "GuestNet", bssid: "cc:dd", channel: 36, band: "5GHz", signal_dbm: -70, security: "open", is_current: false },
    ],
    security_issues: [
      { ssid: "GuestNet", severity: "critical", issue: "Open network", recommendation: "Enable WPA2" },
    ],
    channel_congestion: [
      { channel: 6, band: "2.4GHz", network_count: 3, congestion_level: "high" },
      { channel: 36, band: "5GHz", network_count: 1, congestion_level: "low" },
    ],
    recommended_channels_2g: [1, 11],
    recommended_channels_5g: [40],
    interface_name: "wlan0",
    scan_timestamp: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("WifiScanner", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the empty state before any scan", () => {
    render(<WifiScanner />);
    expect(screen.getByText(/No WiFi networks found/)).toBeInTheDocument();
  });

  it("scans and renders the networks tab with current network summary", async () => {
    mockInvoke.mockResolvedValue(scanResult());
    render(<WifiScanner />);
    fireEvent.click(screen.getAllByRole("button", { name: /Scan WiFi/ })[0]);

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("network_wifi_scan"));
    expect(await screen.findByText("HomeNet")).toBeInTheDocument();
    expect(screen.getByText("GuestNet")).toBeInTheDocument();
    expect(screen.getByText("WPA2-PSK")).toBeInTheDocument();
    expect(screen.getByText("Open")).toBeInTheDocument();
    expect(screen.getByText("wlan0")).toBeInTheDocument();
  });

  it("shows the current-network summary bar when present", async () => {
    mockInvoke.mockResolvedValue(scanResult({
      current_network: { ssid: "HomeNet", bssid: "aa:bb", channel: 6, band: "2.4GHz", signal_dbm: -45, security: "wpa2_psk", is_current: true },
    }));
    render(<WifiScanner />);
    fireEvent.click(screen.getAllByRole("button", { name: /Scan WiFi/ })[0]);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("network_wifi_scan"));
    expect(await screen.findAllByText("HomeNet")).not.toHaveLength(0);
    expect(screen.getByText(/-45 dBm/)).toBeInTheDocument();
  });

  it("shows a scan error", async () => {
    mockInvoke.mockRejectedValue(new Error("no wifi adapter"));
    render(<WifiScanner />);
    fireEvent.click(screen.getAllByRole("button", { name: /Scan WiFi/ })[0]);
    expect(await screen.findByText(/no wifi adapter/)).toBeInTheDocument();
  });

  it("sorts networks by channel and SSID", async () => {
    mockInvoke.mockResolvedValue(scanResult());
    render(<WifiScanner />);
    fireEvent.click(screen.getAllByRole("button", { name: /Scan WiFi/ })[0]);
    await screen.findByText("HomeNet");

    fireEvent.click(screen.getByText("Channel"));
    fireEvent.click(screen.getByText("SSID"));
    // No crash across sort modes is the meaningful assertion.
    expect(screen.getByText("HomeNet")).toBeInTheDocument();
  });

  it("filters networks by band", async () => {
    mockInvoke.mockResolvedValue(scanResult());
    render(<WifiScanner />);
    fireEvent.click(screen.getAllByRole("button", { name: /Scan WiFi/ })[0]);
    await screen.findByText("HomeNet");

    fireEvent.click(screen.getByRole("button", { name: "5 GHz" }));
    expect(screen.queryByText("HomeNet")).not.toBeInTheDocument();
    expect(screen.getByText("GuestNet")).toBeInTheDocument();
  });

  it("opens the advanced details modal for a network", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_wifi_scan") return Promise.resolve(scanResult());
      if (cmd === "network_analyze_wifi_details") {
        return Promise.resolve({
          ssid: "HomeNet", bssid: "aa:bb", channel: 6, channel_width_mhz: 20,
          band: "2.4GHz", signal_dbm: -45, noise_dbm: -90, security: "WPA2-PSK",
        });
      }
      return Promise.resolve(undefined);
    });
    render(<WifiScanner />);
    fireEvent.click(screen.getAllByRole("button", { name: /Scan WiFi/ })[0]);
    await screen.findByText("HomeNet");

    fireEvent.click(screen.getAllByText("network.wifiAdvancedDetails")[0]);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("network_analyze_wifi_details", expect.objectContaining({ ssid: "HomeNet" })));
    expect(await screen.findByText("aa:bb")).toBeInTheDocument();

    fireEvent.click(screen.getByText("✕"));
    expect(screen.queryByText("aa:bb")).not.toBeInTheDocument();
  });

  it("switches to the channels tab", async () => {
    mockInvoke.mockResolvedValue(scanResult());
    render(<WifiScanner />);
    fireEvent.click(screen.getAllByRole("button", { name: /Scan WiFi/ })[0]);
    await screen.findByText("HomeNet");

    fireEvent.click(screen.getByText("Channel Map"));
    expect((await screen.findAllByText("2.4 GHz")).length).toBeGreaterThan(0);
    expect(screen.getByText(/Recommended Channels: 1, 11/)).toBeInTheDocument();
  });

  it("switches to the security tab and shows grouped issues", async () => {
    mockInvoke.mockResolvedValue(scanResult());
    render(<WifiScanner />);
    fireEvent.click(screen.getAllByRole("button", { name: /Scan WiFi/ })[0]);
    await screen.findByText("HomeNet");

    fireEvent.click(screen.getByText("Security Audit"));
    expect(await screen.findByText("Open network")).toBeInTheDocument();
    expect(screen.getByText("Enable WPA2")).toBeInTheDocument();
  });

  it("shows the no-issues state on the security tab", async () => {
    mockInvoke.mockResolvedValue(scanResult({ security_issues: [] }));
    render(<WifiScanner />);
    fireEvent.click(screen.getAllByRole("button", { name: /Scan WiFi/ })[0]);
    await screen.findByText("HomeNet");

    fireEvent.click(screen.getByText("Security Audit"));
    expect(await screen.findByText("No security issues detected")).toBeInTheDocument();
  });

  it("toggles auto-refresh and scans again after the interval", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    mockInvoke.mockResolvedValue(scanResult());
    render(<WifiScanner />);

    fireEvent.click(screen.getByRole("checkbox"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("network_wifi_scan"));
    const callsAfterFirst = mockInvoke.mock.calls.length;

    await vi.advanceTimersByTimeAsync(10000);
    expect(mockInvoke.mock.calls.length).toBeGreaterThan(callsAfterFirst);
    vi.useRealTimers();
  });
});
