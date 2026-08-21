import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
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

  async function connectAndGetCanvas(width = 800, height = 600) {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spice_connect") return Promise.resolve({ id: "conn-1", width, height });
      return Promise.resolve(undefined);
    });
    const utils = render(<SpiceViewer sessionId="sess-1" config={config} />);
    await waitFor(() => {
      expect(utils.container.querySelector("canvas")?.width).toBe(width);
    });
    return utils;
  }

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

  it("disconnects immediately once connect resolves if unmounted first (cancelled-before-connect)", async () => {
    let resolveConnect: (v: { id: string; width: number; height: number }) => void = () => {};
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "spice_connect") {
        return new Promise((resolve) => {
          resolveConnect = resolve;
        });
      }
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<SpiceViewer sessionId="sess-1" config={config} />);
    unmount();
    resolveConnect({ id: "conn-1", width: 100, height: 100 });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("spice_disconnect", { connectionId: "conn-1" });
    });
  });

  it("clicking Disconnect forwards to spice_disconnect", async () => {
    await connectAndGetCanvas();
    fireEvent.click(screen.getByTitle("Disconnect"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("spice_disconnect", { connectionId: "conn-1" });
    });
  });

  it("forwards mouse move to spice_send_mouse_move with translated coordinates", async () => {
    const { container } = await connectAndGetCanvas(800, 600);
    const canvas = container.querySelector("canvas")!;

    // The global test setup gives every element a 1024x800 bounding rect,
    // so a move at the rect's center maps predictably onto the 800x600
    // canvas backing store: x = 512 * 800/1024 = 400, y = 400 * 600/800 = 300.
    fireEvent.mouseMove(canvas, { clientX: 512, clientY: 400 });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("spice_send_mouse_move", { connectionId: "conn-1", x: 400, y: 300 });
    });
  });

  it("forwards mouse button presses/releases, mapping left/middle/right and ignoring other buttons", async () => {
    const { container } = await connectAndGetCanvas();
    const canvas = container.querySelector("canvas")!;

    fireEvent.mouseDown(canvas, { button: 0 });
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("spice_send_mouse_button", { connectionId: "conn-1", button: 0, pressed: true });
    });

    fireEvent.mouseUp(canvas, { button: 2 });
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("spice_send_mouse_button", { connectionId: "conn-1", button: 2, pressed: false });
    });

    mockInvoke.mockClear();
    fireEvent.mouseDown(canvas, { button: 3 });
    expect(mockInvoke).not.toHaveBeenCalledWith("spice_send_mouse_button", expect.anything());
  });

  it("forwards keydown/keyup to spice_send_key using PC/AT scancodes, skipping unmapped codes", async () => {
    await connectAndGetCanvas();
    const app = screen.getByRole("application");

    fireEvent.keyDown(app, { code: "KeyA" });
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("spice_send_key", { connectionId: "conn-1", scancode: 0x1e, pressed: true });
    });

    fireEvent.keyUp(app, { code: "KeyA" });
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("spice_send_key", { connectionId: "conn-1", scancode: 0x1e, pressed: false });
    });

    mockInvoke.mockClear();
    const result = fireEvent.keyDown(app, { code: "MediaPlayPause" });
    expect(result).toBe(true); // not prevented — unmapped code is a no-op
    expect(mockInvoke).not.toHaveBeenCalledWith("spice_send_key", expect.anything());
  });

  it("ignores spice:frame and spice:disconnected events for a different connection id", async () => {
    const { container } = await connectAndGetCanvas(800, 600);

    handlers["spice:frame"]({
      payload: { connection_id: "some-other-conn", width: 999, height: 999, data_base64: btoa("") },
    });
    expect(container.querySelector("canvas")?.width).toBe(800);

    handlers["spice:disconnected"]({ payload: { connection_id: "some-other-conn", reason: "x" } });
    expect(screen.getByTitle("Disconnect")).toBeInTheDocument();
    expect(screen.queryByText("Disconnected")).not.toBeInTheDocument();
  });

  it("prevents the browser context menu on the canvas", async () => {
    const { container } = await connectAndGetCanvas();
    const result = fireEvent.contextMenu(container.querySelector("canvas")!);
    expect(result).toBe(false);
  });
});
