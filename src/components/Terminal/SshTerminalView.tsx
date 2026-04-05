import { useEffect, useRef, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTerminalStore } from "@/stores/terminalStore";
import { ConnectionStatus } from "@/types";
import "@xterm/xterm/css/xterm.css";

interface SshTerminalViewProps {
  readonly connectionId: string;
  readonly isActive: boolean;
}

function getCSSVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function getTerminalTheme(): Record<string, string> {
  return {
    foreground: getCSSVar("--terminal-fg"),
    background: getCSSVar("--terminal-bg"),
    cursor: getCSSVar("--terminal-cursor"),
    selectionBackground: getCSSVar("--terminal-selection"),
    black: getCSSVar("--terminal-ansi-0"),
    red: getCSSVar("--terminal-ansi-1"),
    green: getCSSVar("--terminal-ansi-2"),
    yellow: getCSSVar("--terminal-ansi-3"),
    blue: getCSSVar("--terminal-ansi-4"),
    magenta: getCSSVar("--terminal-ansi-5"),
    cyan: getCSSVar("--terminal-ansi-6"),
    white: getCSSVar("--terminal-ansi-7"),
    brightBlack: getCSSVar("--terminal-ansi-8"),
    brightRed: getCSSVar("--terminal-ansi-9"),
    brightGreen: getCSSVar("--terminal-ansi-10"),
    brightYellow: getCSSVar("--terminal-ansi-11"),
    brightBlue: getCSSVar("--terminal-ansi-12"),
    brightMagenta: getCSSVar("--terminal-ansi-13"),
    brightCyan: getCSSVar("--terminal-ansi-14"),
    brightWhite: getCSSVar("--terminal-ansi-15"),
  };
}

export default function SshTerminalView({ connectionId, isActive }: SshTerminalViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const updateTerminalDimensions = useTerminalStore((s) => s.updateTerminalDimensions);
  const updateTerminalStatus = useTerminalStore((s) => s.updateTerminalStatus);

  const handleResize = useCallback(() => {
    const fitAddon = fitAddonRef.current;
    if (!fitAddon) return;
    try {
      fitAddon.fit();
      const term = termRef.current;
      if (term) {
        const { cols, rows } = term;
        updateTerminalDimensions(connectionId, cols, rows);
        invoke("ssh_resize", { connectionId, rows, cols }).catch(() => {});
      }
    } catch {
      // fit may fail if container has zero size
    }
  }, [connectionId, updateTerminalDimensions]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const theme = getTerminalTheme();

    const term = new Terminal({
      fontFamily: "'JetBrains Mono', monospace",
      fontSize: 14,
      lineHeight: 1.2,
      scrollback: 10000,
      cursorBlink: true,
      cursorStyle: "block",
      allowProposedApi: true,
      theme,
    });

    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    const webLinksAddon = new WebLinksAddon();

    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);
    term.loadAddon(webLinksAddon);

    term.open(container);

    try {
      const webglAddon = new WebglAddon();
      webglAddon.onContextLoss(() => webglAddon.dispose());
      term.loadAddon(webglAddon);
    } catch {
      // WebGL not available
    }

    fitAddon.fit();
    termRef.current = term;
    fitAddonRef.current = fitAddon;

    // Send initial dimensions
    const { cols, rows } = term;
    updateTerminalDimensions(connectionId, cols, rows);
    invoke("ssh_resize", { connectionId, rows, cols }).catch(() => {});

    // Forward user input to SSH backend
    const dataDisposable = term.onData((data) => {
      invoke("ssh_write", { connectionId, data }).catch(() => {});
    });

    // Listen for SSH output
    const outputUnlisten = listen<{ connection_id: string; data: string }>(
      "ssh:output",
      (event) => {
        if (event.payload.connection_id === connectionId) {
          term.write(event.payload.data);
        }
      }
    );

    // Listen for SSH disconnection
    const disconnectUnlisten = listen<{ connection_id: string; reason: string }>(
      "ssh:disconnected",
      (event) => {
        if (event.payload.connection_id === connectionId) {
          term.write(`\r\n\x1b[90m[SSH disconnected: ${event.payload.reason}]\x1b[0m\r\n`);
          updateTerminalStatus(connectionId, ConnectionStatus.Disconnected);
        }
      }
    );

    updateTerminalStatus(connectionId, ConnectionStatus.Connected);

    // ResizeObserver for auto-fit
    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        try {
          fitAddon.fit();
          const { cols, rows } = term;
          updateTerminalDimensions(connectionId, cols, rows);
          invoke("ssh_resize", { connectionId, rows, cols }).catch(() => {});
        } catch {
          // ignore
        }
      });
    });
    resizeObserver.observe(container);

    return () => {
      resizeObserver.disconnect();
      dataDisposable.dispose();
      outputUnlisten.then((fn) => fn());
      disconnectUnlisten.then((fn) => fn());
      term.dispose();
      termRef.current = null;
      fitAddonRef.current = null;
    };
  }, [connectionId, updateTerminalDimensions, updateTerminalStatus]);

  // Re-fit when tab becomes active
  useEffect(() => {
    if (isActive) {
      requestAnimationFrame(() => {
        handleResize();
        termRef.current?.focus();
      });
    }
  }, [isActive, handleResize]);

  return (
    <div
      ref={containerRef}
      className="w-full h-full bg-[var(--terminal-bg)]"
      style={{ padding: "4px 0 0 4px" }}
    />
  );
}
