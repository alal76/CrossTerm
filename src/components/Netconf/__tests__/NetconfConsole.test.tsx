import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import NetconfConsole from "@/components/Netconf/NetconfConsole";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import type { NetconfConfig, NetconfSessionInfo, NetconfRpcResult } from "@/types";

Element.prototype.scrollIntoView = vi.fn();

const mockInvoke = vi.mocked(invoke);

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: NetconfConfig = {
  host: "192.168.0.40",
  port: 830,
  username: "admin",
  password: "hunter2",
  capabilities: [],
};

const sessionInfo: NetconfSessionInfo = {
  id: "conn-1",
  host: "192.168.0.40",
  server_capabilities: ["urn:ietf:params:netconf:base:1.1", "urn:ietf:params:netconf:capability:candidate:1.0"],
  session_id: 7,
};

describe("NetconfConsole", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("connects and shows the session id and server capabilities", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "netconf_connect") return Promise.resolve("conn-1");
      if (cmd === "netconf_list") return Promise.resolve([sessionInfo]);
      return Promise.resolve(undefined);
    });

    renderWithToast(<NetconfConsole sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/session 7/)).toBeInTheDocument();
    expect(screen.getByText("base:1.1")).toBeInTheDocument();
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "netconf_connect") return Promise.reject(new Error("Authentication rejected"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<NetconfConsole sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Couldn't connect/)).toBeInTheDocument();
    expect(screen.getByText(/Authentication rejected/)).toBeInTheDocument();
  });

  it("runs get-config against the selected datastore and logs the result", async () => {
    const result: NetconfRpcResult = { message_id: "1", xml: "<data/>", ok: true };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "netconf_connect") return Promise.resolve("conn-1");
      if (cmd === "netconf_list") return Promise.resolve([sessionInfo]);
      if (cmd === "netconf_get_config") return Promise.resolve(result);
      return Promise.resolve(undefined);
    });

    renderWithToast(<NetconfConsole sessionId="sess-1" config={config} />);
    await screen.findByText(/session 7/);

    fireEvent.click(screen.getByText("Run"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("netconf_get_config", { id: "conn-1", datastore: "running", filter: null });
    });
    expect(await screen.findByText("get-config(running)")).toBeInTheDocument();
    expect(screen.getByText("<data/>")).toBeInTheDocument();
  });

  it("sends a free-form RPC body", async () => {
    const result: NetconfRpcResult = { message_id: "2", xml: "<ok/>", ok: true };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "netconf_connect") return Promise.resolve("conn-1");
      if (cmd === "netconf_list") return Promise.resolve([sessionInfo]);
      if (cmd === "netconf_rpc") return Promise.resolve(result);
      return Promise.resolve(undefined);
    });

    renderWithToast(<NetconfConsole sessionId="sess-1" config={config} />);
    await screen.findByText(/session 7/);

    fireEvent.click(screen.getByText("Send"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("netconf_rpc", { id: "conn-1", xmlBody: "<get/>" });
    });
    expect(await screen.findByText("<ok/>")).toBeInTheDocument();
  });

  it("surfaces an RPC-level error via toast without throwing", async () => {
    const result: NetconfRpcResult = { message_id: "3", xml: "<rpc-error/>", ok: false, error: "invalid-value" };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "netconf_connect") return Promise.resolve("conn-1");
      if (cmd === "netconf_list") return Promise.resolve([sessionInfo]);
      if (cmd === "netconf_get_config") return Promise.resolve(result);
      return Promise.resolve(undefined);
    });

    renderWithToast(<NetconfConsole sessionId="sess-1" config={config} />);
    await screen.findByText(/session 7/);

    fireEvent.click(screen.getByText("Run"));

    expect(await screen.findByText("<rpc-error/>")).toBeInTheDocument();
    expect(screen.getAllByText(/invalid-value/).length).toBeGreaterThan(0);
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "netconf_connect") return Promise.resolve("conn-1");
      if (cmd === "netconf_list") return Promise.resolve([sessionInfo]);
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<NetconfConsole sessionId="sess-1" config={config} />);
    await screen.findByText(/session 7/);
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("netconf_disconnect", { id: "conn-1" });
  });
});
