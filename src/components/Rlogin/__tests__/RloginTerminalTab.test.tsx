import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import RloginTerminalTab from "@/components/Rlogin/RloginTerminalTab";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { RloginConfig } from "@/types";

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

const config: RloginConfig = { host: "10.0.0.70", port: 513, local_username: "alal", remote_username: "root", terminal_type: "xterm", terminal_speed: 38400 };

describe("RloginTerminalTab", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects on mount and mounts the terminal", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rlogin_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<RloginTerminalTab sessionId="sess-1" config={config} />);

    expect(await screen.findByTestId("rlogin-container-sess-1")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("rlogin_connect", { config });
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rlogin_connect") return Promise.reject(new Error("Connection failed"));
      return Promise.resolve(undefined);
    });

    render(<RloginTerminalTab sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Failed to connect/)).toBeInTheDocument();
  });

  it("forwards keystrokes to rlogin_send and incoming rlogin:data to the terminal", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rlogin_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<RloginTerminalTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOnData).toHaveBeenCalled());

    const onDataCallback = mockTermOnData.mock.calls[0][0] as (data: string) => void;
    onDataCallback("ls\n");
    expect(mockInvoke).toHaveBeenCalledWith("rlogin_send", { id: "conn-1", data: "ls\n" });

    handlers["rlogin:data"]({ payload: { session_id: "conn-1", data: "total 0\n" } });
    expect(mockTermWrite).toHaveBeenCalledWith("total 0\n");
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rlogin_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<RloginTerminalTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("rlogin_disconnect", { id: "conn-1" });
  });
});
