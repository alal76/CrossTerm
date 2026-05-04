import { vi } from "vitest";
import "@testing-library/jest-dom/vitest";

// ── Mock Tauri APIs ──

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// ── Mock localStorage for Zustand persist middleware ──

const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: (key: string) => store[key] ?? null,
    setItem: (key: string, value: string) => { store[key] = value; },
    removeItem: (key: string) => { delete store[key]; },
    clear: () => { store = {}; },
    get length() { return Object.keys(store).length; },
    key: (index: number) => Object.keys(store)[index] ?? null,
  };
})();
Object.defineProperty(globalThis, "localStorage", { value: localStorageMock });

// ── Give DOM elements a real bounding rect so virtualizers render items ──
// Without this, jsdom returns all zeros and @tanstack/react-virtual renders
// nothing (container has no height → 0 visible items → empty list in tests).

Object.defineProperty(HTMLElement.prototype, "getBoundingClientRect", {
  configurable: true,
  value(): DOMRect {
    return {
      width: 1024, height: 800, top: 0, left: 0, bottom: 800, right: 1024, x: 0, y: 0,
      toJSON() { return this; },
    } as DOMRect;
  },
});

// ── Mock ResizeObserver for @tanstack/react-virtual ──
// jsdom doesn't implement ResizeObserver; without this the virtualizer never
// learns the container size and renders 0 items.
class MockResizeObserver {
  private readonly cb: ResizeObserverCallback;
  constructor(cb: ResizeObserverCallback) { this.cb = cb; }
  observe(target: Element) {
    this.cb([{
      target,
      contentRect: { width: 1024, height: 800, top: 0, left: 0, bottom: 800, right: 1024, x: 0, y: 0, toJSON() { return {}; } } as DOMRectReadOnly,
      borderBoxSize: [{ blockSize: 800, inlineSize: 1024 }] as ReadonlyArray<ResizeObserverSize>,
      contentBoxSize: [{ blockSize: 800, inlineSize: 1024 }] as ReadonlyArray<ResizeObserverSize>,
      devicePixelContentBoxSize: [] as ReadonlyArray<ResizeObserverSize>,
    }], this);
  }
  unobserve(_target: Element) { /* no-op — test mock */ }
  disconnect() { /* no-op — test mock */ }
}
globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

// ── Mock window.matchMedia ──

Object.defineProperty(globalThis, "matchMedia", {
  writable: true,
  value: vi.fn().mockImplementation((query: string) => ({
    matches: query === "(prefers-color-scheme: dark)",
    media: query,
    onchange: null,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
});
