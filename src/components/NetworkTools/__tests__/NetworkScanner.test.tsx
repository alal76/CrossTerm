import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import NetworkScanner from "@/components/NetworkTools/NetworkScanner";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ScanResult } from "@/types";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

function scanResult(overrides: Partial<ScanResult> = {}): ScanResult {
  return {
    ip: "192.168.1.10",
    hostname: "host-a",
    open_ports: [{ port: 22, service_name: "ssh", protocol: "tcp" }],
    response_time_ms: 5,
    ...overrides,
  };
}

describe("NetworkScanner", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("does not start a scan for an empty CIDR", () => {
    render(<NetworkScanner />);
    expect(screen.getByRole("button", { name: /Scan/i })).toBeDisabled();
  });

  it("starts a scan and streams in found hosts", async () => {
    mockInvoke.mockResolvedValue("scan-1");
    render(<NetworkScanner />);

    fireEvent.change(screen.getByPlaceholderText(/CIDR|e\.g\./i), { target: { value: "192.168.1.0/24" } });
    fireEvent.click(screen.getByRole("button", { name: /Scan/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("network_scan_start", { target: { cidr: "192.168.1.0/24" } });
    });

    handlers["network:scan_host_found"]({ payload: { scan_id: "scan-1", result: scanResult() } });
    expect(await screen.findByText("192.168.1.10")).toBeInTheDocument();
    expect(screen.getByText("host-a")).toBeInTheDocument();
  });

  it("ignores host results from a stale scan id", async () => {
    mockInvoke.mockResolvedValue("scan-1");
    render(<NetworkScanner />);
    fireEvent.change(screen.getByPlaceholderText(/CIDR|e\.g\./i), { target: { value: "192.168.1.0/24" } });
    fireEvent.click(screen.getByRole("button", { name: /Scan/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());

    handlers["network:scan_host_found"]({ payload: { scan_id: "stale-scan", result: scanResult() } });
    expect(screen.queryByText("192.168.1.10")).not.toBeInTheDocument();
  });

  it("updates progress and stops scanning when complete", async () => {
    mockInvoke.mockResolvedValue("scan-1");
    render(<NetworkScanner />);
    fireEvent.change(screen.getByPlaceholderText(/CIDR|e\.g\./i), { target: { value: "10.0.0.0/24" } });
    fireEvent.click(screen.getByRole("button", { name: /Scan/i }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());

    handlers["network:scan_progress"]({ payload: { scan_id: "scan-1", hosts_scanned: 128, total_hosts: 254 } });
    expect(await screen.findByText("128/254")).toBeInTheDocument();

    handlers["network:scan_progress"]({ payload: { scan_id: "scan-1", hosts_scanned: 254, total_hosts: 254 } });
    await waitFor(() => {
      expect(screen.getByRole("button", { name: /Scan/i })).not.toBeDisabled();
    });
  });

  it("submits on Enter and saves results as sessions", async () => {
    mockInvoke.mockResolvedValueOnce("scan-1");
    render(<NetworkScanner />);
    const input = screen.getByPlaceholderText(/CIDR|e\.g\./i);
    fireEvent.change(input, { target: { value: "10.0.0.0/24" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("network_scan_start", { target: { cidr: "10.0.0.0/24" } }));

    handlers["network:scan_host_found"]({ payload: { scan_id: "scan-1", result: scanResult() } });
    await screen.findByText("192.168.1.10");

    mockInvoke.mockResolvedValueOnce(undefined);
    fireEvent.click(screen.getByText(/Save All as Sessions/i));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("network_scan_save_as_sessions", { scanId: "scan-1", folder: "Scanned Hosts" });
    });
  });
});
