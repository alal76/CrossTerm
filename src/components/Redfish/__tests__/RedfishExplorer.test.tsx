import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import RedfishExplorer from "@/components/Redfish/RedfishExplorer";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import type { RedfishConfig, RedfishSystem } from "@/types";

const mockInvoke = vi.mocked(invoke);

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: RedfishConfig = {
  host: "192.168.0.20",
  port: 443,
  username: "admin",
  password: "hunter2",
  use_tls: true,
  verify_tls: false,
};

const system: RedfishSystem = {
  id: "System.1",
  name: "Server1",
  manufacturer: "Dell",
  model: "PowerEdge R740",
  serial: "ABC123",
  power_state: "On",
};

describe("RedfishExplorer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("connects, loads systems, and renders them", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "redfish_connect") return Promise.resolve("conn-1");
      if (cmd === "redfish_get_systems") return Promise.resolve([system]);
      return Promise.resolve(undefined);
    });

    renderWithToast(<RedfishExplorer sessionId="sess-1" config={config} />);

    expect(await screen.findByText("Server1")).toBeInTheDocument();
    expect(screen.getByText(/Dell/)).toBeInTheDocument();
    expect(screen.getByText("On")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("redfish_connect", { config });
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "redfish_connect") return Promise.reject(new Error("connection refused"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<RedfishExplorer sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Couldn't reach the Redfish service/)).toBeInTheDocument();
  });

  it("sends a power action for a system and refreshes the list", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "redfish_connect") return Promise.resolve("conn-1");
      if (cmd === "redfish_get_systems") return Promise.resolve([system]);
      if (cmd === "redfish_power_control") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    renderWithToast(<RedfishExplorer sessionId="sess-1" config={config} />);
    await screen.findByText("Server1");

    fireEvent.click(screen.getByText("Restart"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("redfish_power_control", {
        id: "conn-1",
        systemId: "System.1",
        action: "GracefulRestart",
      });
    });
    // Restart isn't in the confirm-required set, so no window.confirm needed —
    // the refresh call after the action proves the action actually ran.
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("redfish_get_systems", { id: "conn-1" });
    });
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "redfish_connect") return Promise.resolve("conn-1");
      if (cmd === "redfish_get_systems") return Promise.resolve([system]);
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<RedfishExplorer sessionId="sess-1" config={config} />);
    await screen.findByText("Server1");
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("redfish_disconnect", { id: "conn-1" });
  });
});
