import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import "@/i18n";
import PortForwardManager from "@/components/NetworkTools/PortForwardManager";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TunnelRule, TunnelStatus, TunnelMetrics } from "@/types";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

function rule(overrides: Partial<TunnelRule> = {}): TunnelRule {
  return {
    id: "tun-1",
    name: "Web",
    local_port: 8080,
    remote_host: "10.0.0.5",
    remote_port: 80,
    tunnel_type: "local",
    ssh_session_ref: undefined,
    auto_start: false,
    enabled: false,
    ...overrides,
  };
}

function metrics(overrides: Partial<TunnelMetrics> = {}): TunnelMetrics {
  return {
    tunnel_id: "tun-1",
    bytes_in: 0,
    bytes_out: 0,
    active_connections: 0,
    uptime_seconds: 0,
    last_activity: null,
    ...overrides,
  };
}

describe("PortForwardManager", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockListen.mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows the empty state when there are no tunnels", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_tunnel_list") return Promise.resolve([]);
      if (cmd === "network_tunnel_metrics_all") return Promise.resolve([]);
      if (cmd === "ssh_list_connections") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    render(<PortForwardManager />);

    expect(await screen.findByText(/No sessions yet/)).toBeInTheDocument();
  });

  it("loads and renders tunnels from network_tunnel_list", async () => {
    const tunnels: [TunnelRule, TunnelStatus][] = [[rule({ name: "My Tunnel" }), "active"]];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_tunnel_list") return Promise.resolve(tunnels);
      if (cmd === "network_tunnel_metrics_all") return Promise.resolve([]);
      if (cmd === "ssh_list_connections") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    render(<PortForwardManager />);

    expect(await screen.findByText("My Tunnel")).toBeInTheDocument();
    expect(screen.getByText(/local · :8080 → 10\.0\.0\.5:80/)).toBeInTheDocument();
  });

  it("creates a tunnel with the entered form data", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_tunnel_list") return Promise.resolve([]);
      if (cmd === "network_tunnel_metrics_all") return Promise.resolve([]);
      if (cmd === "ssh_list_connections") return Promise.resolve([]);
      if (cmd === "network_tunnel_create") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    render(<PortForwardManager />);
    await screen.findByText("Add Tunnel");

    fireEvent.click(screen.getByText("Add Tunnel"));
    fireEvent.change(screen.getByPlaceholderText("Name"), { target: { value: "New Tunnel" } });
    fireEvent.change(screen.getByPlaceholderText("Local Port"), { target: { value: "9000" } });
    fireEvent.change(screen.getByPlaceholderText("Remote Host"), { target: { value: "example.com" } });
    fireEvent.change(screen.getByPlaceholderText("Remote Port"), { target: { value: "443" } });
    fireEvent.click(screen.getByText("Create"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "network_tunnel_create",
        expect.objectContaining({
          rule: expect.objectContaining({
            name: "New Tunnel",
            local_port: 9000,
            remote_host: "example.com",
            remote_port: 443,
            tunnel_type: "local",
          }),
        })
      );
    });
  });

  it("toggling on a tunnel with an ssh_session_ref adds a real SSH port forward", async () => {
    const tunnels: [TunnelRule, TunnelStatus][] = [
      [rule({ ssh_session_ref: "conn-1", enabled: false }), "inactive"],
    ];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_tunnel_list") return Promise.resolve(tunnels);
      if (cmd === "network_tunnel_metrics_all") return Promise.resolve([]);
      if (cmd === "ssh_list_connections") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    render(<PortForwardManager />);
    await screen.findByText("Web");

    fireEvent.click(screen.getByTitle("Inactive"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("ssh_port_forward_add", {
        connectionId: "conn-1",
        forward: { Local: { id: "tun-1", bind_host: "127.0.0.1", bind_port: 8080, remote_host: "10.0.0.5", remote_port: 80 } },
      });
      expect(mockInvoke).toHaveBeenCalledWith("network_tunnel_toggle", { ruleId: "tun-1", enabled: true });
    });
  });

  it("toggling off a tunnel with an ssh_session_ref removes the real SSH port forward", async () => {
    const tunnels: [TunnelRule, TunnelStatus][] = [
      [rule({ ssh_session_ref: "conn-1", enabled: true }), "active"],
    ];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_tunnel_list") return Promise.resolve(tunnels);
      if (cmd === "network_tunnel_metrics_all") return Promise.resolve([]);
      if (cmd === "ssh_list_connections") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    render(<PortForwardManager />);
    await screen.findByText("Web");

    fireEvent.click(screen.getByTitle("Active"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("ssh_port_forward_remove", { connectionId: "conn-1", forwardId: "tun-1" });
      expect(mockInvoke).toHaveBeenCalledWith("network_tunnel_toggle", { ruleId: "tun-1", enabled: false });
    });
  });

  it("removes a tunnel", async () => {
    const tunnels: [TunnelRule, TunnelStatus][] = [[rule(), "inactive"]];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_tunnel_list") return Promise.resolve(tunnels);
      if (cmd === "network_tunnel_metrics_all") return Promise.resolve([]);
      if (cmd === "ssh_list_connections") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    render(<PortForwardManager />);
    await screen.findByText("Web");

    const removeButtons = document.querySelectorAll("button");
    const removeButton = Array.from(removeButtons).find((b) => b.querySelector("svg.lucide-trash2"));
    expect(removeButton).toBeTruthy();
    fireEvent.click(removeButton as HTMLButtonElement);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("network_tunnel_remove", { ruleId: "tun-1" });
    });
  });

  it("polls network_tunnel_metrics_all on mount and shows bytes for an enabled tunnel with traffic", async () => {
    const tunnels: [TunnelRule, TunnelStatus][] = [[rule({ enabled: true }), "active"]];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_tunnel_list") return Promise.resolve(tunnels);
      if (cmd === "network_tunnel_metrics_all")
        return Promise.resolve([metrics({ bytes_in: 2048, bytes_out: 512, active_connections: 2 })]);
      if (cmd === "ssh_list_connections") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    render(<PortForwardManager />);

    expect(await screen.findByTestId("tunnel-metrics-tun-1")).toHaveTextContent("↓2.0 KB ↑512 B · 2 connections");
  });

  it("does not show a metrics line for a tunnel with no traffic yet", async () => {
    const tunnels: [TunnelRule, TunnelStatus][] = [[rule({ enabled: true }), "active"]];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_tunnel_list") return Promise.resolve(tunnels);
      if (cmd === "network_tunnel_metrics_all") return Promise.resolve([metrics()]);
      if (cmd === "ssh_list_connections") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    render(<PortForwardManager />);
    await screen.findByText("Web");

    expect(screen.queryByTestId("tunnel-metrics-tun-1")).not.toBeInTheDocument();
  });

  it("polls metrics again after the interval elapses", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const tunnels: [TunnelRule, TunnelStatus][] = [[rule({ enabled: true }), "active"]];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "network_tunnel_list") return Promise.resolve(tunnels);
      if (cmd === "network_tunnel_metrics_all") return Promise.resolve([metrics({ bytes_in: 10, bytes_out: 10 })]);
      if (cmd === "ssh_list_connections") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    render(<PortForwardManager />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("network_tunnel_metrics_all");
    });
    const callsAfterMount = mockInvoke.mock.calls.filter((c) => c[0] === "network_tunnel_metrics_all").length;

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });

    const callsAfterInterval = mockInvoke.mock.calls.filter((c) => c[0] === "network_tunnel_metrics_all").length;
    expect(callsAfterInterval).toBeGreaterThan(callsAfterMount);
  });
});
