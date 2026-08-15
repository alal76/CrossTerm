import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import SpiceViewer from "@/components/Spice/SpiceViewer";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SpiceConsoleConfig } from "@/types";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

const config: SpiceConsoleConfig = {
  host: "10.0.0.30",
  port: 5900,
  password: "ticket123",
};

describe("SpiceViewer", () => {
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
      if (cmd === "spice_connect") return Promise.resolve({ id: "conn-1", width: 1024, height: 768 });
      return Promise.resolve(undefined);
    });

    const { container } = render(<SpiceViewer sessionId="sess-1" config={config} />);

    await waitFor(() => {
      const canvas = container.querySelector("canvas");
      expect(canvas?.width).toBe(1024);
      expect(canvas?.height).toBe(768);
    });
    expect(mockInvoke).toHaveBeenCalledWith("spice_connect", { config });
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spice_connect") return Promise.reject(new Error("Authentication failed"));
      return Promise.resolve(undefined);
    });

    render(<SpiceViewer sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Authentication failed/)).toBeInTheDocument();
  });

  it("resizes the canvas on a spice:frame event carrying a different size", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spice_connect") return Promise.resolve({ id: "conn-1", width: 800, height: 600 });
      return Promise.resolve(undefined);
    });

    const { container } = render(<SpiceViewer sessionId="sess-1" config={config} />);
    await waitFor(() => {
      expect(container.querySelector("canvas")?.width).toBe(800);
    });

    handlers["spice:frame"]({
      payload: { connection_id: "conn-1", width: 1024, height: 768, data_base64: btoa("") },
    });

    await waitFor(() => {
      const canvas = container.querySelector("canvas");
      expect(canvas?.width).toBe(1024);
      expect(canvas?.height).toBe(768);
    });
  });

  it("shows a disconnected state on a spice:disconnected event", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spice_connect") return Promise.resolve({ id: "conn-1", width: 1024, height: 768 });
      return Promise.resolve(undefined);
    });

    render(<SpiceViewer sessionId="sess-1" config={config} />);
    await screen.findByTitle("Disconnect");

    handlers["spice:disconnected"]({ payload: { connection_id: "conn-1", reason: "closed" } });

    expect(await screen.findByText("Disconnected")).toBeInTheDocument();
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spice_connect") return Promise.resolve({ id: "conn-1", width: 1024, height: 768 });
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<SpiceViewer sessionId="sess-1" config={config} />);
    await screen.findByTitle("Disconnect");
    unmount();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("spice_disconnect", { connectionId: "conn-1" });
    });
  });
});
