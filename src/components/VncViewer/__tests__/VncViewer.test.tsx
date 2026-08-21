import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
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
    Object.assign(navigator, {
      clipboard: {
        writeText: vi.fn().mockResolvedValue(undefined),
        readText: vi.fn().mockResolvedValue(""),
      },
    });
  });

  async function connectAndGetCanvas(width = 800, height = 600) {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "vnc_connect") return Promise.resolve({ id: "conn-1", width, height });
      return Promise.resolve(undefined);
    });
    const utils = render(<VncViewer sessionId="sess-1" config={config} />);
    await waitFor(() => {
      expect(utils.container.querySelector("canvas")?.width).toBe(width);
    });
    return utils;
  }

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

  it("disconnects immediately once connect resolves if unmounted first (cancelled-before-connect)", async () => {
    let resolveConnect: (v: { id: string; width: number; height: number }) => void = () => {};
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "vnc_connect") {
        return new Promise((resolve) => {
          resolveConnect = resolve;
        });
      }
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<VncViewer sessionId="sess-1" config={config} />);
    unmount();
    resolveConnect({ id: "conn-1", width: 100, height: 100 });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_disconnect", { connectionId: "conn-1" });
    });
  });

  it("renders the toolbar once connected and forwards disconnect/view-only/clipboard/screenshot actions", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "vnc_connect") return Promise.resolve({ id: "conn-1", width: 800, height: 600 });
      return Promise.resolve(undefined);
    });

    render(<VncViewer sessionId="sess-1" config={config} />);
    await screen.findByTitle("vnc.disconnect");

    // View-only toggle
    fireEvent.click(screen.getByTitle("vnc.viewOnly"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_set_view_only", { connectionId: "conn-1", viewOnly: true });
    });

    // Clipboard: reads local clipboard then forwards to the server
    fireEvent.click(screen.getByTitle("vnc.clipboard"));
    await waitFor(() => {
      expect(navigator.clipboard.readText).toHaveBeenCalled();
      expect(mockInvoke).toHaveBeenCalledWith("vnc_clipboard_send", { connectionId: "conn-1", text: "" });
    });

    // Screenshot: triggers a canvas.toDataURL download
    const toDataURLSpy = vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue("data:image/png;base64,x");
    const clickSpy = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    fireEvent.click(screen.getByTitle("vnc.screenshot"));
    expect(toDataURLSpy).toHaveBeenCalledWith("image/png");
    expect(clickSpy).toHaveBeenCalled();

    // Disconnect
    fireEvent.click(screen.getByTitle("vnc.disconnect"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_disconnect", { connectionId: "conn-1" });
    });
  });

  it("changes scaling mode via the toolbar and forwards it to vnc_set_scaling", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "vnc_connect") return Promise.resolve({ id: "conn-1", width: 800, height: 600 });
      return Promise.resolve(undefined);
    });

    render(<VncViewer sessionId="sess-1" config={config} />);
    await screen.findByTitle("vnc.scaleMode");

    fireEvent.click(screen.getByTitle("vnc.scaleMode"));
    fireEvent.click(screen.getByText("vnc.scroll"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_set_scaling", { connectionId: "conn-1", mode: "scroll" });
    });
    // "scroll" mode switches the container to a scrollable overflow.
    await waitFor(() => {
      expect(screen.getByRole("application").className).toContain("overflow-auto");
    });

    fireEvent.click(screen.getByTitle("vnc.scaleMode"));
    fireEvent.click(screen.getByText("vnc.actualSize"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_set_scaling", { connectionId: "conn-1", mode: "one_to_one" });
    });
  });

  it("forwards mouse events to vnc_send_mouse with translated coordinates and a button mask", async () => {
    const { container } = await connectAndGetCanvas(800, 600);
    const canvas = container.querySelector("canvas")!;

    // The global test setup gives every element a 1024x800 bounding rect,
    // so a click at the rect's center maps predictably onto the 800x600
    // canvas backing store: x = 512 * 800/1024 = 400, y = 400 * 600/800 = 300.
    fireEvent.mouseDown(canvas, { clientX: 512, clientY: 400, buttons: 1 });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_send_mouse", {
        connectionId: "conn-1",
        x: 400,
        y: 300,
        buttonMask: 1,
      });
    });
  });

  it("suppresses mouse forwarding once view-only is enabled", async () => {
    const { container } = await connectAndGetCanvas();
    fireEvent.click(screen.getByTitle("vnc.viewOnly"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_set_view_only", { connectionId: "conn-1", viewOnly: true });
    });
    mockInvoke.mockClear();

    fireEvent.mouseDown(container.querySelector("canvas")!, { clientX: 10, clientY: 10, buttons: 1 });
    expect(mockInvoke).not.toHaveBeenCalledWith("vnc_send_mouse", expect.anything());
  });

  it("forwards keydown/keyup to vnc_send_key using X11 keysyms", async () => {
    await connectAndGetCanvas();
    const app = screen.getByRole("application");

    fireEvent.keyDown(app, { key: "a" });
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_send_key", { connectionId: "conn-1", keyCode: 97, pressed: true });
    });

    fireEvent.keyUp(app, { key: "a" });
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_send_key", { connectionId: "conn-1", keyCode: 97, pressed: false });
    });

    // Special keys resolve through the X11 keysym table.
    fireEvent.keyDown(app, { key: "Enter" });
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_send_key", { connectionId: "conn-1", keyCode: 0xff0d, pressed: true });
    });
  });

  it("suppresses keyboard forwarding once view-only is enabled", async () => {
    await connectAndGetCanvas();
    fireEvent.click(screen.getByTitle("vnc.viewOnly"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vnc_set_view_only", { connectionId: "conn-1", viewOnly: true });
    });
    mockInvoke.mockClear();

    fireEvent.keyDown(screen.getByRole("application"), { key: "a" });
    expect(mockInvoke).not.toHaveBeenCalledWith("vnc_send_key", expect.anything());
  });

  it("ignores vnc:resize and vnc:disconnected events for a different connection id", async () => {
    const { container } = await connectAndGetCanvas(800, 600);

    handlers["vnc:resize"]({ payload: { connection_id: "some-other-conn", width: 999, height: 999 } });
    expect(container.querySelector("canvas")?.width).toBe(800);

    handlers["vnc:disconnected"]({ payload: { connection_id: "some-other-conn", reason: "x" } });
    // Still connected — toolbar (and not the disconnected overlay) is showing.
    expect(screen.getByTitle("vnc.disconnect")).toBeInTheDocument();
    expect(screen.queryByText(/disconnected/i)).not.toBeInTheDocument();
  });

  it("does not throw handling a vnc:frame update for the active connection", async () => {
    await connectAndGetCanvas(4, 4);
    expect(() =>
      handlers["vnc:frame"]({
        payload: { connection_id: "conn-1", x: 0, y: 0, width: 4, height: 4, data_base64: btoa("") },
      })
    ).not.toThrow();
  });

  it("copies vnc:clipboard payloads for the active connection to the local clipboard", async () => {
    await connectAndGetCanvas();
    handlers["vnc:clipboard"]({ payload: { connection_id: "conn-1", text: "hello from server" } });
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("hello from server");
    });
  });

  it("ignores a vnc:clipboard payload from a different connection", async () => {
    await connectAndGetCanvas();
    handlers["vnc:clipboard"]({ payload: { connection_id: "some-other-conn", text: "nope" } });
    expect(navigator.clipboard.writeText).not.toHaveBeenCalled();
  });

  it("prevents the browser context menu on the canvas", async () => {
    const { container } = await connectAndGetCanvas();
    const result = fireEvent.contextMenu(container.querySelector("canvas")!);
    expect(result).toBe(false);
  });
});
