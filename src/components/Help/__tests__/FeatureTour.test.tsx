import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@/i18n";
import FeatureTour from "@/components/Help/FeatureTour";

// Mock localStorage for jsdom
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

describe("FeatureTour", () => {
  const onComplete = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockLocalStorage.clear();
    for (const key in localStorageStore) delete localStorageStore[key];

    // Create a mock target element for the tour spotlight
    const el = document.createElement("div");
    el.dataset.tour = "new-session";
    el.style.position = "fixed";
    el.style.top = "100px";
    el.style.left = "100px";
    el.style.width = "200px";
    el.style.height = "40px";
    document.body.appendChild(el);
  });

  // FT-H-07: Spotlight overlay highlights target, step navigation works
  it("FT-H-07: renders spotlight overlay and step navigation", () => {
    render(<FeatureTour tourId="ssh" onComplete={onComplete} />);

    // Tour popover should be visible with the first step
    expect(screen.getByText("Step 1 of 3")).toBeInTheDocument();
    expect(screen.getByText("Create a Session")).toBeInTheDocument();

    // Should have Next button
    expect(screen.getByText("Next")).toBeInTheDocument();

    // Should have Skip Tour button
    expect(screen.getByText("Skip Tour")).toBeInTheDocument();

    // SVG spotlight overlay should be rendered
    const svgs = document.querySelectorAll("svg");
    expect(svgs.length).toBeGreaterThan(0);
  });

  it("FT-H-07: Next button advances steps", () => {
    render(<FeatureTour tourId="ssh" onComplete={onComplete} />);

    // Click Next to advance to step 2
    fireEvent.click(screen.getByText("Next"));
    expect(screen.getByText("Step 2 of 3")).toBeInTheDocument();
    expect(screen.getByText("Quick Connect")).toBeInTheDocument();

    // Previous button should now be visible
    expect(screen.getByText("Previous")).toBeInTheDocument();

    // Click Next again to advance to step 3 (last step)
    fireEvent.click(screen.getByText("Next"));
    expect(screen.getByText("Step 3 of 3")).toBeInTheDocument();

    // On the last step, the button should say "Finish"
    expect(screen.getByText("Finish")).toBeInTheDocument();

    // Click Finish
    fireEvent.click(screen.getByText("Finish"));
    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  it("FT-H-07: Escape key closes tour", () => {
    render(<FeatureTour tourId="ssh" onComplete={onComplete} />);

    fireEvent.keyDown(globalThis, { key: "Escape" });

    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  it("FT-H-07: Previous button goes back", () => {
    render(<FeatureTour tourId="ssh" onComplete={onComplete} />);

    // Advance to step 2
    fireEvent.click(screen.getByText("Next"));
    expect(screen.getByText("Step 2 of 3")).toBeInTheDocument();

    // Go back
    fireEvent.click(screen.getByText("Previous"));
    expect(screen.getByText("Step 1 of 3")).toBeInTheDocument();
  });
});
