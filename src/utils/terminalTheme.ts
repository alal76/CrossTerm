import { useEffect } from "react";
import type { RefObject } from "react";
import type { Terminal } from "@xterm/xterm";
import { useAppStore } from "@/stores/appStore";

function getCssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

export function getTerminalTheme(): Record<string, string> {
  return {
    foreground: getCssVar("--terminal-fg"),
    background: getCssVar("--terminal-bg"),
    cursor: getCssVar("--terminal-cursor"),
    selectionBackground: getCssVar("--terminal-selection"),
    black: getCssVar("--terminal-ansi-0"),
    red: getCssVar("--terminal-ansi-1"),
    green: getCssVar("--terminal-ansi-2"),
    yellow: getCssVar("--terminal-ansi-3"),
    blue: getCssVar("--terminal-ansi-4"),
    magenta: getCssVar("--terminal-ansi-5"),
    cyan: getCssVar("--terminal-ansi-6"),
    white: getCssVar("--terminal-ansi-7"),
    brightBlack: getCssVar("--terminal-ansi-8"),
    brightRed: getCssVar("--terminal-ansi-9"),
    brightGreen: getCssVar("--terminal-ansi-10"),
    brightYellow: getCssVar("--terminal-ansi-11"),
    brightBlue: getCssVar("--terminal-ansi-12"),
    brightMagenta: getCssVar("--terminal-ansi-13"),
    brightCyan: getCssVar("--terminal-ansi-14"),
    brightWhite: getCssVar("--terminal-ansi-15"),
  };
}

/**
 * Live-updates an already-mounted xterm.js Terminal instance's colors when
 * the app theme changes, instead of requiring the tab to be closed and
 * reopened. `resolvedTheme` covers built-in dark/light switches (App.tsx
 * toggles a class/attribute on <html> that changes the underlying CSS
 * variables); `customThemeTokens` covers imported custom themes, which
 * SettingsPanel.tsx applies by writing CSS variables directly. xterm's
 * `options.theme` setter requires a new object reference to pick up the
 * change (reference equality is checked internally), so a fresh object is
 * always passed rather than mutating the existing one.
 */
export function useHotTerminalTheme(termRef: RefObject<Terminal | null>) {
  const resolvedTheme = useAppStore((s) => s.resolvedTheme);
  const customThemeTokens = useAppStore((s) => s.customThemeTokens);

  useEffect(() => {
    if (termRef.current) {
      termRef.current.options.theme = { ...getTerminalTheme() };
    }
  }, [resolvedTheme, customThemeTokens, termRef]);
}
