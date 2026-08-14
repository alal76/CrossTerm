import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import MoshTerminalTab from "@/components/Mosh/MoshTerminalTab";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { MoshConfig } from "@/types";

const mockTermWrite = vi.fn();
const mockTermOnData = vi.fn((_cb: (data: string) => void) => ({ dispose: vi.fn() }));
const mockTermOpen = vi.fn();
const mockTermDispose = vi.fn();
vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn().mockImplementation(function (this: Record<string, unknown>) {
    this.loadAddon = vi.fn();
    this.open = mockTermOpen;
    this.onData = mockTermOnData;
    this.write = mockTermWrite;
    this.dispose = mockTermDispose;
    this.focus = vi.fn();
    this.cols = 80;
    this.rows = 24;
  }),
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn().mockImplementation(function (this: Record<string, unknown>) {
    this.fit = vi.fn();
  }),
}));
vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: vi.fn().mockImplementation(function () {}),
}));

class MockResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

const config: MoshConfig = { host: "192.168.0.20", port: 22, username: "alal" };

describe("MoshTerminalTab", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects on mount, mounts the terminal, and sends an initial resize", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mosh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<MoshTerminalTab sessionId="sess-1" config={config} />);

    expect(await screen.findByTestId("mosh-container-sess-1")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("mosh_connect", { config });
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("mosh_resize", { id: "conn-1", cols: 80, rows: 24 }));
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mosh_connect") return Promise.reject(new Error("mosh: Could not resolve hostname"));
      return Promise.resolve(undefined);
    });

    render(<MoshTerminalTab sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Failed to connect/)).toBeInTheDocument();
    expect(screen.getByText(/Could not resolve hostname/)).toBeInTheDocument();
  });

  it("forwards keystrokes to mosh_write and incoming mosh:output to the terminal", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mosh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<MoshTerminalTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOnData).toHaveBeenCalled());

    const onDataCallback = mockTermOnData.mock.calls[0][0] as (data: string) => void;
    onDataCallback("ls\n");
    expect(mockInvoke).toHaveBeenCalledWith("mosh_write", { id: "conn-1", data: "ls\n" });

    handlers["mosh:output"]({ payload: { id: "conn-1", data: "total 0\n" } });
    expect(mockTermWrite).toHaveBeenCalledWith("total 0\n");
  });

  it("ignores mosh:output for a different connection", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mosh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<MoshTerminalTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());

    handlers["mosh:output"]({ payload: { id: "other-conn", data: "should not appear" } });
    expect(mockTermWrite).not.toHaveBeenCalledWith("should not appear");
  });

  it("writes a message and flips to disconnected on mosh:exit", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mosh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<MoshTerminalTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());

    handlers["mosh:exit"]({ payload: { id: "conn-1" } });
    expect(mockTermWrite).toHaveBeenCalledWith(expect.stringContaining("mosh session ended"));
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "mosh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<MoshTerminalTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("mosh_disconnect", { id: "conn-1" });
  });
});
