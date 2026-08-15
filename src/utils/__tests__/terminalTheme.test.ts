import { describe, it, expect, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { Terminal } from "@xterm/xterm";
import { getTerminalTheme, useHotTerminalTheme } from "@/utils/terminalTheme";
import { useAppStore } from "@/stores/appStore";
import { ThemeVariant } from "@/types";
import type { ThemeTokens } from "@/types";

function fakeTermRef(): { current: { options: { theme?: unknown } } } {
  return { current: { options: {} } };
}

describe("getTerminalTheme", () => {
  it("reads all 16 ANSI colors plus foreground/background/cursor/selection from CSS custom properties", () => {
    const theme = getTerminalTheme();
    expect(Object.keys(theme).sort()).toEqual(
      [
        "background",
        "black",
        "blue",
        "brightBlack",
        "brightBlue",
        "brightCyan",
        "brightGreen",
        "brightMagenta",
        "brightRed",
        "brightWhite",
        "brightYellow",
        "cursor",
        "cyan",
        "foreground",
        "green",
        "magenta",
        "red",
        "selectionBackground",
        "white",
        "yellow",
      ].sort()
    );
  });
});

describe("useHotTerminalTheme", () => {
  beforeEach(() => {
    useAppStore.setState({ resolvedTheme: ThemeVariant.Dark, customThemeTokens: null });
  });

  it("does nothing if the terminal ref is not yet set", () => {
    const termRef = { current: null } as unknown as React.RefObject<Terminal | null>;
    expect(() => renderHook(() => useHotTerminalTheme(termRef))).not.toThrow();
  });

  it("assigns a theme object onto the ref'd terminal when resolvedTheme changes", () => {
    const termRef = fakeTermRef() as unknown as React.RefObject<Terminal | null>;
    renderHook(() => useHotTerminalTheme(termRef));

    act(() => {
      useAppStore.setState({ resolvedTheme: ThemeVariant.Light });
    });

    expect(termRef.current?.options.theme).toBeTruthy();
  });

  it("assigns a fresh object reference on each change, not a mutated one", () => {
    const termRef = fakeTermRef() as unknown as React.RefObject<Terminal | null>;
    renderHook(() => useHotTerminalTheme(termRef));

    act(() => {
      useAppStore.setState({ resolvedTheme: ThemeVariant.Light });
    });
    const first = termRef.current?.options.theme;

    act(() => {
      useAppStore.setState({ resolvedTheme: ThemeVariant.Dark });
    });
    const second = termRef.current?.options.theme;

    expect(first).toBeTruthy();
    expect(second).toBeTruthy();
    expect(second).not.toBe(first);
  });

  it("also reacts to customThemeTokens changing (imported custom themes)", () => {
    const termRef = fakeTermRef() as unknown as React.RefObject<Terminal | null>;
    renderHook(() => useHotTerminalTheme(termRef));

    act(() => {
      useAppStore.setState({ resolvedTheme: ThemeVariant.Light });
    });
    const afterFirst = termRef.current?.options.theme;

    act(() => {
      useAppStore.setState({ customThemeTokens: { "terminal-foreground": "#ffffff" } as Partial<ThemeTokens> });
    });

    expect(termRef.current?.options.theme).not.toBe(afterFirst);
  });
});
