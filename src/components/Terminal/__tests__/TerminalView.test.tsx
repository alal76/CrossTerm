import { describe, it, expect, beforeEach, vi } from "vitest";
import { render } from "@testing-library/react";
import "@/i18n";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTerminalStore } from "@/stores/terminalStore";
import { useAppStore } from "@/stores/appStore";
import { ThemeVariant, ConnectionStatus } from "@/types";

// ── Mock ResizeObserver ──
class MockResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

// ── Mock AudioContext (for bell) ──
globalThis.AudioContext = vi.fn().mockImplementation(() => ({
  createOscillator: vi.fn(() => ({
    connect: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
    frequency: { value: 0 },
  })),
  createGain: vi.fn(() => ({
    connect: vi.fn(),
    gain: { value: 0 },
  })),
  destination: {},
  currentTime: 0,
})) as unknown as typeof AudioContext;

// ── Capture xterm callbacks ──
let xtermOnDataCb: ((data: string) => void) | undefined;

const mockTerminalInstance = {
  open: vi.fn(),
  write: vi.fn(),
  onData: vi.fn((cb: (data: string) => void) => {
    xtermOnDataCb = cb;
    return { dispose: vi.fn() };
  }),
  onResize: vi.fn(() => {
    return { dispose: vi.fn() };
  }),
  dispose: vi.fn(),
  loadAddon: vi.fn(),
  options: {},
  cols: 80,
  rows: 24,
  focus: vi.fn(),
  getSelection: vi.fn(() => ""),
  selectAll: vi.fn(),
  clear: vi.fn(),
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
    return { findNext: vi.fn(), findPrevious: vi.fn(), dispose: vi.fn() };
  },
}));

vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: function MockWebLinksAddon() {
    return { dispose: vi.fn() };
  },
}));

vi.mock("@tauri-apps/plugin-shell", () => ({
  open: vi.fn(),
}));

// ── Mock TerminalSearch (not under test) ──
vi.mock("@/components/Terminal/TerminalSearch", () => ({
  default: () => null,
}));

// ── Capture listen callbacks ──
const listenCallbacks = new Map<string, (event: unknown) => void>();

// eslint-disable-next-line @typescript-eslint/no-explicit-any
(listen as any).mockImplementation((eventName: string, cb: (event: unknown) => void) => {
  listenCallbacks.set(eventName, cb);
  return Promise.resolve(() => {
    listenCallbacks.delete(eventName);
  });
});

import TerminalView from "@/components/Terminal/TerminalView";

function resetStores() {
  // Override store mutators with no-ops to prevent infinite re-render loop.
  // The real functions mutate the terminals Map, which changes the emitData
  // callback reference (a useEffect dep), causing cleanup → setState → re-render → loop.
  const noop = () => {};
  useTerminalStore.setState({
    terminals: new Map([
      ["test-term-1", {
        id: "test-term-1",
        sessionId: "session-1",
        status: ConnectionStatus.Idle,
        cols: 80,
        rows: 24,
        title: "",
      }],
    ]),
    broadcastMode: false,
    removeTerminal: noop,
    updateTerminalStatus: noop,
    updateTerminalDimensions: noop,
  });

  useAppStore.setState({
    bellStyle: "visual",
    cursorStyle: "block",
    cursorBlink: true,
    theme: ThemeVariant.Dark,
    resolvedTheme: ThemeVariant.Dark,
    customShortcuts: {},
  });
}

describe("TerminalView", () => {
  beforeEach(() => {
    resetStores();
    vi.clearAllMocks();
    listenCallbacks.clear();
    xtermOnDataCb = undefined;
    // invoke must return a Promise (component calls .catch() on it)
    vi.mocked(invoke).mockResolvedValue(undefined);
    mockTerminalInstance.open.mockClear();
    mockTerminalInstance.write.mockClear();
    mockTerminalInstance.onData.mockClear();
    mockTerminalInstance.loadAddon.mockClear();
    mockTerminalInstance.dispose.mockClear();
  });

  // FT-C-29: Component renders and mounts xterm Terminal instance
  it("FT-C-29: mounts xterm Terminal instance on render", () => {
    render(<TerminalView terminalId="test-term-1" isActive={true} />);

    // xterm Terminal.open() should be called with the container div
    expect(mockTerminalInstance.open).toHaveBeenCalledTimes(1);
    expect(mockTerminalInstance.loadAddon).toHaveBeenCalled();
  });

  // FT-C-30: User input triggers invoke("terminal_write")
  it("FT-C-30: user input triggers terminal_write invoke", () => {
    render(<TerminalView terminalId="test-term-1" isActive={true} />);

    // Simulate user typing by calling the onData callback
    expect(xtermOnDataCb).toBeDefined();
    if (xtermOnDataCb) xtermOnDataCb("hello");

    expect(invoke).toHaveBeenCalledWith("terminal_write", {
      id: "test-term-1",
      data: "hello",
    });
  });

  // FT-C-31: Backend output from listen("terminal:output") calls terminal.write()
  it("FT-C-31: backend output event calls terminal.write()", () => {
    render(<TerminalView terminalId="test-term-1" isActive={true} />);

    // The component should have registered a listener for terminal:output
    const outputCb = listenCallbacks.get("terminal:output");
    expect(outputCb).toBeDefined();

    // Simulate backend sending output
    if (outputCb) outputCb({ payload: { terminal_id: "test-term-1", data: "Welcome to shell" } });

    expect(mockTerminalInstance.write).toHaveBeenCalledWith("Welcome to shell");
  });

  // FT-C-32: Resize event sends dimensions to backend
  it("FT-C-32: resize sends dimensions to backend via invoke", () => {
    render(<TerminalView terminalId="test-term-1" isActive={true} />);

    // invoke is called for terminal_resize during initial fit
    expect(invoke).toHaveBeenCalledWith("terminal_resize", {
      id: "test-term-1",
      rows: 24,
      cols: 80,
    });
  });
});
