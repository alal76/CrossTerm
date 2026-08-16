import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import { invoke } from "@tauri-apps/api/core";
import AircrackPanel from "@/components/NetworkTools/AircrackPanel";
import type { AircrackToolStatus, WirelessInterface } from "@/types";

const mockInvoke = vi.mocked(invoke);

const TOOL_STATUS: AircrackToolStatus = {
  aircrack_ng: true,
  airmon_ng: true,
  airodump_ng: true,
  aireplay_ng: true,
  version: "1.7",
  needs_root: false,
};

const MON_IFACE: WirelessInterface = {
  name: "wlan0mon",
  driver: "ath9k_htc",
  chipset: "Atheros",
  monitor_mode: true,
};

const NON_MON_IFACE: WirelessInterface = {
  name: "wlan0",
  driver: "ath9k_htc",
  monitor_mode: false,
};

function defaultInvoke(overrides: Record<string, (...args: unknown[]) => unknown> = {}) {
  mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
    if (overrides[cmd]) return Promise.resolve(overrides[cmd](args));
    if (cmd === "network_aircrack_check") return Promise.resolve(TOOL_STATUS);
    if (cmd === "network_aircrack_accept_disclaimer") return Promise.resolve(undefined);
    if (cmd === "network_aircrack_interfaces") return Promise.resolve([NON_MON_IFACE]);
    if (cmd === "network_aircrack_audit_log") return Promise.resolve([]);
    return Promise.resolve(undefined);
  });
}

async function acceptDisclaimer() {
  render(<AircrackPanel />);
  await screen.findByText("⚠️ Educational & Ethical Use Only");
  const checkboxes = screen.getAllByRole("checkbox");
  for (const cb of checkboxes) fireEvent.click(cb);
  fireEvent.click(screen.getByText("I Accept — Proceed with Caution"));
  await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("network_aircrack_accept_disclaimer"));
}

describe("AircrackPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    defaultInvoke();
  });

  it("shows the not-installed state with an install command", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_aircrack_check") {
        return Promise.resolve({ ...TOOL_STATUS, aircrack_ng: false });
      }
      return Promise.resolve(undefined);
    });
    render(<AircrackPanel />);
    expect(await screen.findByText(/aircrack-ng is not installed/)).toBeInTheDocument();
  });

  it("shows the disclaimer gate and requires all checkboxes before accepting", async () => {
    render(<AircrackPanel />);
    await screen.findByText("⚠️ Educational & Ethical Use Only");

    const acceptButton = screen.getByText("I Accept — Proceed with Caution");
    expect(acceptButton).toBeDisabled();

    const checkboxes = screen.getAllByRole("checkbox");
    expect(checkboxes).toHaveLength(4);
    fireEvent.click(checkboxes[0]);
    expect(acceptButton).toBeDisabled();

    fireEvent.click(screen.getByText("Accept All"));
    expect(acceptButton).not.toBeDisabled();
  });

  it("declining the disclaimer stays on the gate", async () => {
    render(<AircrackPanel />);
    await screen.findByText("⚠️ Educational & Ethical Use Only");
    fireEvent.click(screen.getByText("Cancel"));
    expect(screen.getByText("⚠️ Educational & Ethical Use Only")).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("network_aircrack_accept_disclaimer");
  });

  it("accepting the disclaimer reveals the panel with tabs and tool status badges", async () => {
    await acceptDisclaimer();
    expect(screen.getByText("1.7")).toBeInTheDocument();
    expect(screen.getByText("aircrack-ng")).toBeInTheDocument();
    expect(screen.getByText("Interfaces")).toBeInTheDocument();
    expect(screen.getByText("Scan")).toBeInTheDocument();
    expect(screen.getByText("Test")).toBeInTheDocument();
    expect(screen.getByText("Audit Log")).toBeInTheDocument();
  });

  it("shows the needs-root badge when required", async () => {
    defaultInvoke({ network_aircrack_check: () => ({ ...TOOL_STATUS, needs_root: true }) });
    await acceptDisclaimer();
    expect(screen.getByText("aircrack-ng requires root/sudo privileges for monitor mode and packet injection. Run CrossTerm with elevated permissions.")).toBeInTheDocument();
  });

  describe("Interfaces tab", () => {
    it("lists interfaces and shows the no-interfaces empty state", async () => {
      defaultInvoke({ network_aircrack_interfaces: () => [] });
      await acceptDisclaimer();
      expect(await screen.findByText(/No wireless interfaces found/)).toBeInTheDocument();
    });

    it("renders an interface and starts monitor mode", async () => {
      defaultInvoke({ network_aircrack_interfaces: () => [NON_MON_IFACE] });
      await acceptDisclaimer();

      expect(await screen.findByText("wlan0")).toBeInTheDocument();
      fireEvent.click(screen.getByText("Enable Monitor"));

      await waitFor(() =>
        expect(mockInvoke).toHaveBeenCalledWith("network_aircrack_monitor_start", { interface: "wlan0" }),
      );
    });

    it("stops monitor mode for an interface already in monitor mode", async () => {
      defaultInvoke({ network_aircrack_interfaces: () => [MON_IFACE] });
      await acceptDisclaimer();

      expect(await screen.findByText("MONITOR")).toBeInTheDocument();
      fireEvent.click(screen.getByText("Disable Monitor"));

      await waitFor(() =>
        expect(mockInvoke).toHaveBeenCalledWith("network_aircrack_monitor_stop", { interface: "wlan0mon" }),
      );
    });

    it("shows an error if fetching interfaces fails", async () => {
      defaultInvoke({
        network_aircrack_interfaces: () => {
          throw new Error("no wireless hardware");
        },
      });
      await acceptDisclaimer();
      expect(await screen.findByText(/no wireless hardware/)).toBeInTheDocument();
    });
  });

  describe("Scan tab", () => {
    async function goToScanTab() {
      await acceptDisclaimer();
      fireEvent.click(screen.getByText("Scan"));
    }

    it("warns when no monitor-mode interface is available", async () => {
      defaultInvoke({ network_aircrack_interfaces: () => [NON_MON_IFACE] });
      await goToScanTab();
      expect(await screen.findByText(/Enable monitor mode on an interface first/)).toBeInTheDocument();
      expect(screen.getByText("Start Scan")).toBeDisabled();
    });

    it("runs a scan and renders discovered networks and clients", async () => {
      defaultInvoke({
        network_aircrack_interfaces: () => [MON_IFACE],
        network_aircrack_scan_start: () => ({
          networks: [
            { bssid: "AA:BB:CC:DD:EE:FF", channel: 6, privacy: "WPA2", cipher: "CCMP", power: -45, essid: "HomeNet", data_frames: 120, clients: 1 },
          ],
          clients: [
            { station_mac: "11:22:33:44:55:66", bssid: "AA:BB:CC:DD:EE:FF", power: -50, packets: 30, probes: ["HomeNet"] },
          ],
          scan_id: "scan-1",
          interface: "wlan0mon",
          scan_time_secs: 15,
        }),
      });
      await goToScanTab();

      await waitFor(() => expect(screen.getByRole("combobox")).toHaveValue("wlan0mon"));
      fireEvent.click(screen.getByText("Start Scan"));

      await waitFor(() =>
        expect(mockInvoke).toHaveBeenCalledWith(
          "network_aircrack_scan_start",
          expect.objectContaining({ interface: "wlan0mon", durationSecs: 15 }),
        ),
      );
      expect((await screen.findAllByText("HomeNet")).length).toBeGreaterThan(0);
      expect(screen.getAllByText("AA:BB:CC:DD:EE:FF").length).toBeGreaterThan(0);
      expect(screen.getByText("Connected Clients")).toBeInTheDocument();
      expect(screen.getByText("11:22:33:44:55:66")).toBeInTheDocument();
    });

    it("shows a scan error", async () => {
      defaultInvoke({
        network_aircrack_interfaces: () => [MON_IFACE],
        network_aircrack_scan_start: () => {
          throw new Error("device busy");
        },
      });
      await goToScanTab();
      await waitFor(() => expect(screen.getByRole("combobox")).toHaveValue("wlan0mon"));
      fireEvent.click(screen.getByText("Start Scan"));
      expect(await screen.findByText(/device busy/)).toBeInTheDocument();
    });
  });

  describe("Test tab", () => {
    async function goToTestTab() {
      await acceptDisclaimer();
      fireEvent.click(screen.getByText("Test"));
      await waitFor(() => expect(screen.getByRole("combobox")).toHaveValue("wlan0mon"));
    }

    it("sends a deauth and shows the result", async () => {
      defaultInvoke({
        network_aircrack_interfaces: () => [MON_IFACE],
        network_aircrack_deauth: () => "Sent 5 deauth frames",
      });
      await goToTestTab();

      fireEvent.change(screen.getByPlaceholderText("AA:BB:CC:DD:EE:FF"), { target: { value: "AA:BB:CC:DD:EE:FF" } });
      fireEvent.click(screen.getByText("Send Deauth"));

      await waitFor(() =>
        expect(mockInvoke).toHaveBeenCalledWith(
          "network_aircrack_deauth",
          expect.objectContaining({ interface: "wlan0mon", targetBssid: "AA:BB:CC:DD:EE:FF" }),
        ),
      );
      expect(await screen.findByText("Sent 5 deauth frames")).toBeInTheDocument();
    });

    it("captures a handshake successfully", async () => {
      defaultInvoke({
        network_aircrack_interfaces: () => [MON_IFACE],
        network_aircrack_capture_handshake: () => ({
          operation_id: "op-1",
          target_bssid: "AA:BB:CC:DD:EE:FF",
          target_essid: "HomeNet",
          handshake_captured: true,
          capture_file: "/tmp/capture.cap",
          elapsed_secs: 12,
        }),
      });
      await goToTestTab();

      fireEvent.change(screen.getByPlaceholderText("AA:BB:CC:DD:EE:FF"), { target: { value: "AA:BB:CC:DD:EE:FF" } });
      const channelInputs = screen.getAllByDisplayValue("");
      fireEvent.change(channelInputs[0], { target: { value: "6" } });
      fireEvent.click(screen.getByText("Capture Handshake"));

      expect(await screen.findByText("✅ Handshake captured successfully!")).toBeInTheDocument();
    });

    it("shows handshake capture failure", async () => {
      defaultInvoke({
        network_aircrack_interfaces: () => [MON_IFACE],
        network_aircrack_capture_handshake: () => ({
          operation_id: "op-1",
          target_bssid: "AA:BB:CC:DD:EE:FF",
          target_essid: "HomeNet",
          handshake_captured: false,
          elapsed_secs: 60,
        }),
      });
      await goToTestTab();

      fireEvent.change(screen.getByPlaceholderText("AA:BB:CC:DD:EE:FF"), { target: { value: "AA:BB:CC:DD:EE:FF" } });
      const channelInputs = screen.getAllByDisplayValue("");
      fireEvent.change(channelInputs[0], { target: { value: "6" } });
      fireEvent.click(screen.getByText("Capture Handshake"));

      expect(await screen.findByText("No handshake captured. Try again with deauth enabled.")).toBeInTheDocument();
    });

    it("disables the crack button until a handshake is captured, then reports a found key", async () => {
      defaultInvoke({
        network_aircrack_interfaces: () => [MON_IFACE],
        network_aircrack_capture_handshake: () => ({
          operation_id: "op-1",
          target_bssid: "AA:BB:CC:DD:EE:FF",
          target_essid: "HomeNet",
          handshake_captured: true,
          capture_file: "/tmp/capture.cap",
          elapsed_secs: 12,
        }),
        network_aircrack_crack_start: () => ({
          operation_id: "op-1",
          target_bssid: "AA:BB:CC:DD:EE:FF",
          keys_tested: 1000,
          keys_per_second: 500,
          key_found: "password123",
          running: false,
          elapsed_secs: 2,
        }),
      });
      await goToTestTab();
      expect(screen.getByText("Test Password Strength")).toBeDisabled();

      fireEvent.change(screen.getByPlaceholderText("AA:BB:CC:DD:EE:FF"), { target: { value: "AA:BB:CC:DD:EE:FF" } });
      const channelInputs = screen.getAllByDisplayValue("");
      fireEvent.change(channelInputs[0], { target: { value: "6" } });
      fireEvent.click(screen.getByText("Capture Handshake"));
      await screen.findByText("✅ Handshake captured successfully!");

      fireEvent.change(screen.getByPlaceholderText("/usr/share/wordlists/rockyou.txt"), {
        target: { value: "/tmp/wordlist.txt" },
      });
      fireEvent.click(screen.getByText("Test Password Strength"));

      expect(await screen.findByText(/PASSWORD FOUND/)).toBeInTheDocument();
      expect(screen.getByText(/Key: password123/)).toBeInTheDocument();
    });
  });

  describe("Audit Log tab", () => {
    async function goToLogTab() {
      await acceptDisclaimer();
      fireEvent.click(screen.getByText("Audit Log"));
    }

    it("shows the empty state when there are no entries", async () => {
      await goToLogTab();
      expect(await screen.findByText("No operations logged yet.")).toBeInTheDocument();
    });

    it("renders audit entries and stops all processes", async () => {
      defaultInvoke({
        network_aircrack_audit_log: () => [
          {
            timestamp: "2026-01-01T00:00:00Z",
            operation: "scan",
            interface: "wlan0mon",
            target: "AA:BB:CC:DD:EE:FF",
            command: "airodump-ng wlan0mon",
            result: "ok",
          },
        ],
        network_aircrack_stop_all: () => undefined,
      });
      await goToLogTab();

      expect(await screen.findByText("airodump-ng wlan0mon")).toBeInTheDocument();

      fireEvent.click(screen.getByText("Stop All Processes"));
      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("network_aircrack_stop_all"));
    });
  });
});
