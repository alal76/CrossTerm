import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import KubernetesPortForwardPanel from "@/components/KubernetesPortForward/KubernetesPortForwardPanel";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { K8sPortForwardConfig } from "@/types";

Element.prototype.scrollIntoView = vi.fn();

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

const config: K8sPortForwardConfig = {
  namespace: "default",
  pod_name: "web-abc123",
  remote_port: 8080,
  local_port: 0,
};

describe("KubernetesPortForwardPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects and shows the bound local port and target pod", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "k8s_port_forward_connect") return Promise.resolve({ id: "conn-1", local_port: 54321 });
      return Promise.resolve(undefined);
    });

    render(<KubernetesPortForwardPanel sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Port Forward — default\/web-abc123/)).toBeInTheDocument();
    expect(screen.getByText(/127\.0\.0\.1:54321/)).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("k8s_port_forward_connect", { config });
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "k8s_port_forward_connect") return Promise.reject(new Error("pod not found"));
      return Promise.resolve(undefined);
    });

    render(<KubernetesPortForwardPanel sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Couldn't forward to web-abc123:8080/)).toBeInTheDocument();
    expect(screen.getByText(/pod not found/)).toBeInTheDocument();
  });

  it("appends connection and error events to the log", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "k8s_port_forward_connect") return Promise.resolve({ id: "conn-1", local_port: 54321 });
      return Promise.resolve(undefined);
    });

    render(<KubernetesPortForwardPanel sessionId="sess-1" config={config} />);
    await screen.findByText(/Port Forward/);

    handlers["k8s_port_forward:connection"]({ payload: { session_id: "conn-1", peer: "127.0.0.1:51000", state: "open" } });
    handlers["k8s_port_forward:error"]({ payload: { session_id: "conn-1", message: "stream closed unexpectedly" } });

    expect(await screen.findByText(/127\.0\.0\.1:51000 — open/)).toBeInTheDocument();
    expect(await screen.findByText(/\[error\] stream closed unexpectedly/)).toBeInTheDocument();
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "k8s_port_forward_connect") return Promise.resolve({ id: "conn-1", local_port: 54321 });
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<KubernetesPortForwardPanel sessionId="sess-1" config={config} />);
    await screen.findByText(/Port Forward/);
    unmount();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("k8s_port_forward_disconnect", { id: "conn-1" });
    });
  });
});
