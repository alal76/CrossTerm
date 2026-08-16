import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTerminalStore } from "@/stores/terminalStore";

class MockResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

let xtermOnDataCb: ((data: string) => void) | undefined;

const mockTerminalInstance = {
  open: vi.fn(),
  write: vi.fn(),
  onData: vi.fn((cb: (data: string) => void) => {
    xtermOnDataCb = cb;
    return { dispose: vi.fn() };
  }),
  dispose: vi.fn(),
  loadAddon: vi.fn(),
  focus: vi.fn(),
  cols: 80,
  rows: 24,
};

vi.mock("@xterm/xterm", () => ({
  Terminal: function MockTerminal() {
    return mockTerminalInstance;
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: function MockFitAddon() {
    return { fit: vi.fn(), dispose: vi.fn() };
  },
}));
vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: function MockWebglAddon() {
    return { onContextLoss: vi.fn(), dispose: vi.fn() };
  },
}));
vi.mock("@xterm/addon-search", () => ({
  SearchAddon: function MockSearchAddon() {
    return { dispose: vi.fn() };
  },
}));
vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: function MockWebLinksAddon() {
    return { dispose: vi.fn() };
  },
}));

const listenCallbacks = new Map<string, (event: unknown) => void>();
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(listen as any).mockImplementation((eventName: string, cb: (event: unknown) => void) => {
  listenCallbacks.set(eventName, cb);
  return Promise.resolve(() => listenCallbacks.delete(eventName));
});

import SshTerminalView from "@/components/Terminal/SshTerminalView";

const mockInvoke = vi.mocked(invoke);

describe("SshTerminalView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listenCallbacks.clear();
    xtermOnDataCb = undefined;
    mockInvoke.mockResolvedValue(undefined);
    useTerminalStore.setState({
      updateTerminalDimensions: vi.fn(),
      updateTerminalStatus: vi.fn(),
    });
  });

  it("mounts the xterm terminal and connects", () => {
    render(<SshTerminalView connectionId="conn-1" isActive />);
    expect(mockTerminalInstance.open).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith("ssh_resize", expect.objectContaining({ connectionId: "conn-1" }));
  });

  it("forwards user input to ssh_write", () => {
    render(<SshTerminalView connectionId="conn-1" isActive />);
    act(() => xtermOnDataCb?.("ls -la\n"));
    expect(mockInvoke).toHaveBeenCalledWith("ssh_write", { connectionId: "conn-1", data: "ls -la\n" });
  });

  it("writes incoming ssh:output for the matching connection only", async () => {
    render(<SshTerminalView connectionId="conn-1" isActive />);
    await act(async () => {
      await Promise.resolve();
    });

    listenCallbacks.get("ssh:output")?.({ payload: { connection_id: "conn-1", data: "hello\n" } });
    expect(mockTerminalInstance.write).toHaveBeenCalledWith("hello\n");

    mockTerminalInstance.write.mockClear();
    listenCallbacks.get("ssh:output")?.({ payload: { connection_id: "other-conn", data: "nope\n" } });
    expect(mockTerminalInstance.write).not.toHaveBeenCalled();
  });

  it("shows a disconnect message and updates status on ssh:disconnected", async () => {
    render(<SshTerminalView connectionId="conn-1" isActive />);
    await act(async () => {
      await Promise.resolve();
    });

    listenCallbacks.get("ssh:disconnected")?.({ payload: { connection_id: "conn-1", reason: "timeout" } });
    expect(mockTerminalInstance.write).toHaveBeenCalledWith(expect.stringContaining("SSH disconnected: timeout"));
  });

  it("drains buffered output once listeners are registered", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_drain_buffer") return Promise.resolve("buffered data");
      return Promise.resolve(undefined);
    });
    render(<SshTerminalView connectionId="conn-1" isActive />);

    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(mockTerminalInstance.write).toHaveBeenCalledWith("buffered data");
  });

  it("starts the health monitor and reacts to session_health events", async () => {
    render(<SshTerminalView connectionId="conn-1" isActive />);
    await act(async () => {
      await Promise.resolve();
    });
    expect(mockInvoke).toHaveBeenCalledWith("ssh_start_health_monitor", { connectionId: "conn-1" });

    act(() => {
      listenCallbacks.get("session_health")?.({
        payload: { sessionId: "conn-1", status: "dropped", latencyMs: 500, lastSeenSecs: 3 },
      });
    });
    expect(document.body.textContent).toContain("Connection lost");
  });

  it("ignores session_health events for a different session", async () => {
    const { container } = render(<SshTerminalView connectionId="conn-1" isActive />);
    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      listenCallbacks.get("session_health")?.({
        payload: { sessionId: "other", status: "dropped", latencyMs: 500, lastSeenSecs: 3 },
      });
    });
    expect(container.textContent).not.toContain("Connection lost");
  });

  it("reconnect handler calls ssh_connect and increments the attempt count", async () => {
    render(<SshTerminalView connectionId="conn-1" isActive />);
    await act(async () => {
      await Promise.resolve();
    });
    act(() => {
      listenCallbacks.get("session_health")?.({
        payload: { sessionId: "conn-1", status: "dropped", latencyMs: null, lastSeenSecs: 3 },
      });
    });

    act(() => {
      document.querySelectorAll("button").forEach((b) => {
        if (b.textContent?.includes("Reconnect now")) b.click();
      });
    });
    expect(mockInvoke).toHaveBeenCalledWith("ssh_connect", { connectionId: "conn-1" });
  });
});
