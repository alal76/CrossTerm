import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import AndroidTerminal, {
  AndroidTerminal as AndroidTerminalNamed,
} from "./AndroidTerminal";

// Mock visualViewport so useEffect doesn't throw in jsdom
beforeEach(() => {
  Object.defineProperty(window, "visualViewport", {
    configurable: true,
    value: {
      height: 800,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    },
  });
  Object.defineProperty(window, "innerHeight", {
    configurable: true,
    value: 800,
  });
});

describe("AndroidTerminal", () => {
  it("renders children in single-pane mode", () => {
    render(
      <AndroidTerminal>
        <span data-testid="child-content">Terminal content</span>
      </AndroidTerminal>
    );

    expect(screen.getByTestId("child-content")).toBeInTheDocument();
    expect(screen.getByTestId("child-content")).toHaveTextContent(
      "Terminal content"
    );
  });

  it("renders children in tablet split-pane mode with grid style", () => {
    const { container } = render(
      <AndroidTerminal isTablet>
        <span data-testid="tablet-child">Tablet content</span>
      </AndroidTerminal>
    );

    expect(screen.getByTestId("tablet-child")).toBeInTheDocument();

    // The wrapper div should use a CSS grid layout for split-pane
    const wrapper = container.firstElementChild as HTMLElement;
    expect(wrapper).not.toBeNull();
    expect(wrapper.style.display).toBe("grid");
    expect(wrapper.style.gridTemplateColumns).toBe("1fr 1fr");
  });

  it("exports both default and named export pointing to the same component", () => {
    expect(AndroidTerminal).toBeDefined();
    expect(AndroidTerminalNamed).toBeDefined();
    expect(AndroidTerminal).toBe(AndroidTerminalNamed);
  });
});
