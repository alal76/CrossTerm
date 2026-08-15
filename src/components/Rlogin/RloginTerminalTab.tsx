import { useEffect, useRef, useState } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Loader2, AlertTriangle } from "lucide-react";
import type { RloginConfig, RloginDataEvent } from "@/types";
import { getTerminalTheme, useHotTerminalTheme } from "@/utils/terminalTheme";
import "@xterm/xterm/css/xterm.css";

interface RloginTerminalTabProps {
  readonly sessionId: string;
  readonly config: RloginConfig;
}

/** rlogin has no PTY-resize protocol of its own (unlike SSH/mosh) and no
 * Telnet-style option negotiation (unlike ttyd) — it's the simplest of the
 * xterm-backed tabs: connect, relay bytes, done. */
export default function RloginTerminalTab({ sessionId, config }: RloginTerminalTabProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const connectionIdRef = useRef<string | null>(null);

  useHotTerminalTheme(termRef);

  const [status, setStatus] = useState<"connecting" | "connected" | "disconnected">("connecting");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setStatus("connecting");
    setError(null);

    invoke<string>("rlogin_connect", { config })
      .then((id) => {
        if (cancelled) {
          invoke("rlogin_disconnect", { id }).catch(() => {});
          return;
        }
        connectionIdRef.current = id;
        setStatus("connected");
      })
      .catch((e) => {
        if (!cancelled) {
          setStatus("disconnected");
          setError(String(e));
        }
      });

    return () => {
      cancelled = true;
      const id = connectionIdRef.current;
      if (id) {
        invoke("rlogin_disconnect", { id }).catch(() => {});
        connectionIdRef.current = null;
      }
    };
  }, [sessionId, config]);

  useEffect(() => {
    if (status !== "connected") return;
    const container = containerRef.current;
    const connectionId = connectionIdRef.current;
    if (!container || !connectionId) return;

    const term = new Terminal({
      fontFamily: "'JetBrains Mono', monospace",
      fontSize: 14,
      lineHeight: 1.2,
      scrollback: 10000,
      cursorBlink: true,
      cursorStyle: "block",
      allowProposedApi: true,
      theme: getTerminalTheme(),
    });
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());
    term.open(container);
    fitAddon.fit();
    term.focus();
    termRef.current = term;
    fitAddonRef.current = fitAddon;

    const dataDisposable = term.onData((data) => {
      invoke("rlogin_send", { id: connectionId, data }).catch(() => {});
    });

    const outputUnlisten = listen<RloginDataEvent>("rlogin:data", (event) => {
      if (event.payload.session_id === connectionId) {
        term.write(event.payload.data);
      }
    });

    const disconnectUnlisten = listen<string>("rlogin:disconnected", (event) => {
      if (event.payload === connectionId) {
        term.write("\r\n\x1b[90m[disconnected]\x1b[0m\r\n");
        setStatus("disconnected");
      }
    });

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        try {
          fitAddon.fit();
        } catch {
          // fit may fail if container has zero size
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
  }, [status]);

  if (status === "connecting") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 bg-[var(--terminal-bg)] text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Connecting to {config.host}…</span>
      </div>
    );
  }

  if (status === "disconnected" && error) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 bg-[var(--terminal-bg)] p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Failed to connect to {config.host}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      data-testid={`rlogin-container-${sessionId}`}
      className="h-full w-full bg-[var(--terminal-bg)]"
      style={{ padding: "4px 0 0 4px" }}
    />
  );
}
