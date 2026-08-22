import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@/i18n";
import WhatsNewPanel from "@/components/Help/WhatsNewPanel";

// Provide a mock localStorage for jsdom if needed
const localStorageStore: Record<string, string> = {};
const mockLocalStorage = {
  getItem: vi.fn((key: string) => localStorageStore[key] ?? null),
  setItem: vi.fn((key: string, value: string) => { localStorageStore[key] = value; }),
  removeItem: vi.fn((key: string) => { delete localStorageStore[key]; }),
  clear: vi.fn(() => { for (const key in localStorageStore) delete localStorageStore[key]; }),
  get length() { return Object.keys(localStorageStore).length; },
  key: vi.fn((i: number) => Object.keys(localStorageStore)[i] ?? null),
};

Object.defineProperty(globalThis, "localStorage", {
  value: mockLocalStorage,
  writable: true,
});

describe("WhatsNewPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockLocalStorage.clear();
    for (const key in localStorageStore) delete localStorageStore[key];
  });

  // FT-H-08: WhatsNewPanel renders after version change, dismissable
  it("FT-H-08: renders when version has not been dismissed", () => {
    // No dismissed version in localStorage — panel should be visible
    render(<WhatsNewPanel />);

    expect(screen.getByText("What's New")).toBeInTheDocument();
    expect(screen.getByText("What's New in CrossTerm 2.0.15")).toBeInTheDocument();

    // Should show release notes content — "New Features" and "Bug Fixes"
    // each appear once per version section (this version's notes plus the
    // previous version's, kept for context), "Improvements" only in the
    // earlier section.
    expect(screen.getAllByText("New Features").length).toBeGreaterThan(0);
    expect(screen.getByText("Improvements")).toBeInTheDocument();
    expect(screen.getAllByText("Bug Fixes").length).toBeGreaterThan(0);
  });

  it("FT-H-08: dismiss button hides the panel", async () => {
    const user = userEvent.setup();
    render(<WhatsNewPanel />);

    expect(screen.getByText("What's New")).toBeInTheDocument();

    // Click the dismiss button
    const dismissBtn = screen.getByText("Got it");
    await user.click(dismissBtn);

    // Panel should be hidden
    expect(screen.queryByText("What's New in CrossTerm 2.0.15")).not.toBeInTheDocument();
  });

  it("FT-H-08: 'Don't show again' persists to localStorage", async () => {
    const user = userEvent.setup();
    render(<WhatsNewPanel />);

    const dontShowBtn = screen.getByText("Don't show again for this version");
    await user.click(dontShowBtn);

    // Panel should be hidden
    expect(screen.queryByText("What's New in CrossTerm 2.0.15")).not.toBeInTheDocument();

    // localStorage should have the dismissed version
    expect(localStorage.getItem("crossterm:whats-new-dismissed")).toBe("2.0.15");
  });

  it("FT-H-08: does not render when version is already dismissed", () => {
    localStorage.setItem("crossterm:whats-new-dismissed", "2.0.15");

    render(<WhatsNewPanel />);

    // Panel should not be visible
    expect(screen.queryByText("What's New")).not.toBeInTheDocument();
  });
});
