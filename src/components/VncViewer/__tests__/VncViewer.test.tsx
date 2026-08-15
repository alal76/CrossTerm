import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import VncViewer from "@/components/VncViewer/VncViewer";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { VncConfig } from "@/types";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

const config: VncConfig = {
  host: "192.168.0.11",
  port: 5900,
  password: "hunter2",
  vnc_auth: true,
  vencrypt: false,
};

describe("VncViewer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects with vnc_connect by default and sizes the canvas from the result", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "vnc_connect") return Promise.resolve({ id: "conn-1", width: 1024, height: 768 });
      return Promise.resolve(undefined);
    });

    const { container } = render(<VncViewer sessionId="sess-1" config={config} />);

    await waitFor(() => {
      const canvas = container.querySelector("canvas");
      expect(canvas?.width).toBe(1024);
      expect(canvas?.height).toBe(768);
    });
    expect(mockInvoke).toHaveBeenCalledWith("vnc_connect", { config });
  });

  it("uses the connectCommand override (e.g. proxmox_console_connect) when provided", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "proxmox_console_connect") return Promise.resolve({ id: "conn-1", width: 800, height: 600 });
      return Promise.resolve(undefined);
    });

    render(<VncViewer sessionId="sess-1" config={config} connectCommand="proxmox_console_connect" />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("proxmox_console_connect", { config });
    });
    expect(mockInvoke).not.toHaveBeenCalledWith("vnc_connect", expect.anything());
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "vnc_connect") return Promise.reject(new Error("Authentication failed"));
      return Promise.resolve(undefined);
    });

    render(<VncViewer sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Authentication failed/)).toBeInTheDocument();
  });

  it("resizes the canvas on a vnc:resize event for the active connection", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "vnc_connect") return Promise.resolve({ id: "conn-1", width: 800, height: 600 });
      return Promise.resolve(undefined);
    });

    const { container } = render(<VncViewer sessionId="sess-1" config={config} />);
    await waitFor(() => {
      expect(container.querySelector("canvas")?.width).toBe(800);
    });

    handlers["vnc:resize"]({ payload: { connection_id: "conn-1", width: 1024, height: 768 } });

    await waitFor(() => {
      const canvas = container.querySelector("canvas");
      expect(canvas?.width).toBe(1024);
      expect(canvas?.height).toBe(768);
    });
  });

  it("shows a disconnected state on a vnc:disconnected event", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "vnc_connect") return Promise.resolve({ id: "conn-1", width: 1024, height: 768 });
      return Promise.resolve(undefined);
    });

    render(<VncViewer sessionId="sess-1" config={config} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_connect", { config });
    });

    handlers["vnc:disconnected"]({ payload: { connection_id: "conn-1", reason: "closed" } });

    expect(await screen.findByText(/disconnected/i)).toBeInTheDocument();
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "vnc_connect") return Promise.resolve({ id: "conn-1", width: 1024, height: 768 });
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<VncViewer sessionId="sess-1" config={config} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_connect", { config });
    });
    unmount();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_disconnect", { connectionId: "conn-1" });
    });
  });
});
