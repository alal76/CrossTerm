import { useEffect, useRef, useCallback, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTerminalStore } from "@/stores/terminalStore";
import { ConnectionStatus } from "@/types";
import ReconnectOverlay from "@/components/Shared/ReconnectOverlay";
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

// Shape of the session_health Tauri event payload
interface SessionHealth {
  sessionId: string;
  status: "ok" | "degraded" | "dropped";
  latencyMs: number | null;
  lastSeenSecs: number;
}

export default function SshTerminalView({ connectionId, isActive }: SshTerminalViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const updateTerminalDimensions = useTerminalStore((s) => s.updateTerminalDimensions);
  const updateTerminalStatus = useTerminalStore((s) => s.updateTerminalStatus);

  // ── Health / reconnect state ──────────────────────────────────────────────
  const [healthStatus, setHealthStatus] = useState<"ok" | "degraded" | "dropped">("ok");
  const [healthLatency, setHealthLatency] = useState<number | null>(null);
  const [reconnectAttempt, setReconnectAttempt] = useState(0);

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

    // We must register listeners BEFORE draining the buffer.
    // The drain switches the backend from buffering to event emission,
    // so any data arriving after drain is emitted as ssh:output.
    // If listeners aren't registered yet, those events would be lost.
    const outputUnlisten = listen<{ connection_id: string; data: string }>(
      "ssh:output",
      (event) => {
        if (event.payload.connection_id === connectionId) {
          term.write(event.payload.data);
        }
      }
    );

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

    // Wait for listeners to be registered, THEN drain buffered output.
    // This ensures no data is lost between drain and listener registration.
    Promise.all([outputUnlisten, disconnectUnlisten]).then(() => {
      invoke<string>("ssh_drain_buffer", { connectionId })
        .then((buffered) => {
          if (buffered) term.write(buffered);
        })
        .catch(() => {});
    });

    const doResize = () => {
      try {
        fitAddon.fit();
        const { cols, rows } = term;
        updateTerminalDimensions(connectionId, cols, rows);
        invoke("ssh_resize", { connectionId, rows, cols }).catch(() => {});
      } catch {
        // ignore
      }
    };

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(doResize);
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

  // ── Session health monitor ────────────────────────────────────────────────
  useEffect(() => {
    invoke("ssh_start_health_monitor", { connectionId }).catch(console.error);

    const unlistenPromise = listen<SessionHealth>("session_health", (event) => {
      const payload = event.payload;
      if (payload.sessionId !== connectionId) return;

      setHealthStatus((prev) => {
        // Transition from dropped → ok resets the attempt counter
        if (prev === "dropped" && payload.status === "ok") {
          setReconnectAttempt(0);
        }
        return payload.status;
      });
      setHealthLatency(payload.latencyMs);
    });

    return () => {
      unlistenPromise.then((fn) => fn());
    };
  }, [connectionId]);

  // ── Reconnect handler ─────────────────────────────────────────────────────
  const handleReconnect = useCallback(() => {
    setReconnectAttempt((a) => a + 1);
    invoke("ssh_connect", { connectionId }).catch(console.error);
  }, [connectionId]);

  return (
    <div className="relative w-full h-full">
      <div
        ref={containerRef}
        className="w-full h-full bg-[var(--terminal-bg)]"
        style={{ padding: "4px 0 0 4px" }}
      />
      <ReconnectOverlay
        sessionId={connectionId}
        status={healthStatus}
        latencyMs={healthLatency}
        attempt={reconnectAttempt}
        onReconnect={handleReconnect}
        onDismiss={() => setHealthStatus("ok")}
        onGiveUp={() => {
          setHealthStatus("ok");
          updateTerminalStatus(connectionId, ConnectionStatus.Disconnected);
        }}
      />
    </div>
  );
}
