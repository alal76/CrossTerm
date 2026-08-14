import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import WebSocketTerminalTab from "@/components/Terminal/WebSocketTerminalTab";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { WsTermConfig } from "@/types";

// No existing terminal component in this codebase (SshTerminalTab included)
// unit-tests real xterm.js mounting — canvas/WebGL isn't meaningfully
// testable under jsdom. This mocks xterm entirely and focuses on what's
// actually new here: the wsterm_connect/wsterm_send/wsterm_disconnect
// invoke wiring and connecting/connected/disconnected status transitions.
const mockTermWrite = vi.fn();
const mockTermOnData = vi.fn((_cb: (data: string) => void) => ({ dispose: vi.fn() }));
const mockTermOpen = vi.fn();
const mockTermDispose = vi.fn();
// vi.fn().mockImplementation(() => ({...})) can't be `new`'d (arrow
// functions aren't constructors) — use a regular function expression
// assigning to `this` instead, which the real xterm.js class constructors
// are stubbing in for.
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

const config: WsTermConfig = { url: "ws://192.168.0.11:7681", verify_tls: false };

describe("WebSocketTerminalTab", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects on mount and mounts the terminal once connected", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "wsterm_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<WebSocketTerminalTab sessionId="sess-1" config={config} />);

    expect(await screen.findByTestId("wsterm-container-sess-1")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("wsterm_connect", { config });
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "wsterm_connect") return Promise.reject(new Error("ECONNREFUSED"));
      return Promise.resolve(undefined);
    });

    render(<WebSocketTerminalTab sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Failed to connect/)).toBeInTheDocument();
    expect(screen.getByText(/ECONNREFUSED/)).toBeInTheDocument();
  });

  it("forwards keystrokes to wsterm_send and incoming wsterm:data to the terminal", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "wsterm_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<WebSocketTerminalTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOnData).toHaveBeenCalled());

    // Simulate the user typing — invoke the onData callback the component registered.
    const onDataCallback = mockTermOnData.mock.calls[0][0] as (data: string) => void;
    onDataCallback("ls\n");
    expect(mockInvoke).toHaveBeenCalledWith("wsterm_send", { id: "conn-1", data: "ls\n" });

    handlers["wsterm:data"]({ payload: { session_id: "conn-1", data: "total 0\n" } });
    expect(mockTermWrite).toHaveBeenCalledWith("total 0\n");
  });

  it("ignores wsterm:data for a different connection", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "wsterm_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<WebSocketTerminalTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());

    handlers["wsterm:data"]({ payload: { session_id: "other-conn", data: "should not appear" } });
    expect(mockTermWrite).not.toHaveBeenCalledWith("should not appear");
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "wsterm_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<WebSocketTerminalTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("wsterm_disconnect", { id: "conn-1" });
  });
});
