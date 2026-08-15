import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import RdpViewer from "@/components/RdpViewer/RdpViewer";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { RdpConfig } from "@/types";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

const config: RdpConfig = {
  host: "192.168.0.10",
  port: 3389,
  username: "alal",
  password: "hunter2",
  domain: undefined,
  nla_enabled: true,
  tls_required: false,
  codec: "auto",
  clipboard_sync: true,
  drive_paths: [],
  printer_redirect: false,
  audio_mode: "none",
  smart_card: false,
};

describe("RdpViewer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects and sizes the canvas from the connect result", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rdp_connect") return Promise.resolve({ id: "conn-1", width: 1280, height: 720 });
      return Promise.resolve(undefined);
    });

    const { container } = render(<RdpViewer sessionId="sess-1" config={config} />);

    await waitFor(() => {
      const canvas = container.querySelector("canvas");
      expect(canvas?.width).toBe(1280);
      expect(canvas?.height).toBe(720);
    });
    expect(mockInvoke).toHaveBeenCalledWith("rdp_connect", { config });
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rdp_connect") return Promise.reject(new Error("NLA authentication failed"));
      return Promise.resolve(undefined);
    });

    render(<RdpViewer sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/NLA authentication failed/)).toBeInTheDocument();
  });

  it("blits a rdp:frame update onto the canvas at the given offset", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rdp_connect") return Promise.resolve({ id: "conn-1", width: 800, height: 600 });
      return Promise.resolve(undefined);
    });

    render(<RdpViewer sessionId="sess-1" config={config} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("rdp_connect", { config });
    });

    // Just confirm the handler is wired and doesn't throw for the active
    // connection id; jsdom's canvas getContext isn't implemented, so the
    // actual pixel blit itself can't be asserted here.
    expect(() =>
      handlers["rdp:frame"]({
        payload: { connection_id: "conn-1", x: 0, y: 0, width: 4, height: 4, data_base64: btoa("") },
      })
    ).not.toThrow();
  });

  it("shows a disconnected state on a rdp:disconnected event", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rdp_connect") return Promise.resolve({ id: "conn-1", width: 1024, height: 768 });
      return Promise.resolve(undefined);
    });

    render(<RdpViewer sessionId="sess-1" config={config} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("rdp_connect", { config });
    });

    handlers["rdp:disconnected"]({ payload: { connection_id: "conn-1", reason: "closed" } });

    expect(await screen.findByText(/disconnected/i)).toBeInTheDocument();
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rdp_connect") return Promise.resolve({ id: "conn-1", width: 1024, height: 768 });
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<RdpViewer sessionId="sess-1" config={config} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("rdp_connect", { config });
    });
    unmount();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("rdp_disconnect", { connectionId: "conn-1" });
    });
  });
});
