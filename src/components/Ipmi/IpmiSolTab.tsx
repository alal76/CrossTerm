import { useEffect, useRef, useState, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Loader2, AlertTriangle, Power, PowerOff, RotateCcw, Zap } from "lucide-react";
import type { IpmiConfig, IpmiSolDataEvent, IpmiPowerStatus, IpmiPowerAction } from "@/types";
import { useToast } from "@/components/Shared/Toast";
import "@xterm/xterm/css/xterm.css";

interface IpmiSolTabProps {
  readonly sessionId: string;
  readonly config: IpmiConfig;
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

const POWER_ACTIONS: { action: IpmiPowerAction; label: string; icon: typeof Power; confirm?: boolean }[] = [
  { action: "up", label: "Power On", icon: Power },
  { action: "cycle", label: "Power Cycle", icon: RotateCcw, confirm: true },
  { action: "down", label: "Power Off", icon: PowerOff, confirm: true },
  { action: "hard_reset", label: "Hard Reset", icon: Zap, confirm: true },
];

/** IPMI Serial-over-LAN — a real character stream over a hand-rolled RAKP+
 * session (see src-tauri/src/ipmi), not a PTY, so there's no resize
 * protocol the way SSH/mosh have one. */
export default function IpmiSolTab({ sessionId, config }: IpmiSolTabProps) {
  const { toast } = useToast();
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const connectionIdRef = useRef<string | null>(null);

  const [status, setStatus] = useState<"connecting" | "connected" | "disconnected">("connecting");
  const [error, setError] = useState<string | null>(null);
  const [powerStatus, setPowerStatus] = useState<IpmiPowerStatus | null>(null);
  const [actingOn, setActingOn] = useState(false);

  useEffect(() => {
    let cancelled = false;
    setStatus("connecting");
    setError(null);

    invoke<string>("ipmi_sol_connect", { config })
      .then((id) => {
        if (cancelled) {
          invoke("ipmi_sol_disconnect", { id }).catch(() => {});
          return;
        }
        connectionIdRef.current = id;
        setStatus("connected");
        invoke<IpmiPowerStatus>("ipmi_power_status", { id })
          .then((s) => !cancelled && setPowerStatus(s))
          .catch(() => {});
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
        invoke("ipmi_sol_disconnect", { id }).catch(() => {});
        connectionIdRef.current = null;
      }
    };
  }, [sessionId, config]);

  const handleResize = useCallback(() => {
    try {
      fitAddonRef.current?.fit();
    } catch {
      // fit may fail if container has zero size
    }
  }, []);

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
      invoke("ipmi_sol_send", { id: connectionId, data }).catch(() => {});
    });

    const outputUnlisten = listen<IpmiSolDataEvent>("ipmi:sol_data", (event) => {
      if (event.payload.session_id === connectionId) {
        term.write(event.payload.data);
      }
    });

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(handleResize);
    });
    resizeObserver.observe(container);

    return () => {
      resizeObserver.disconnect();
      dataDisposable.dispose();
      outputUnlisten.then((fn) => fn());
      term.dispose();
      termRef.current = null;
      fitAddonRef.current = null;
    };
  }, [status, handleResize]);

  const handlePowerAction = useCallback(
    async (action: IpmiPowerAction, label: string, requiresConfirm: boolean | undefined) => {
      const id = connectionIdRef.current;
      if (!id) return;
      if (requiresConfirm && !window.confirm(`${label} ${config.host}? This cannot be undone.`)) return;
      setActingOn(true);
      try {
        await invoke("ipmi_power_control", { id, action });
        toast("success", `${label} sent`);
        const s = await invoke<IpmiPowerStatus>("ipmi_power_status", { id });
        setPowerStatus(s);
      } catch (e) {
        toast("error", `${label} failed: ${String(e)}`);
      } finally {
        setActingOn(false);
      }
    },
    [config.host, toast]
  );

  if (status === "connecting") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 bg-[var(--terminal-bg)] text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Establishing RAKP+ session with {config.host}…</span>
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
    <div className="flex h-full w-full flex-col bg-[var(--terminal-bg)]">
      <div className="flex shrink-0 items-center justify-between border-b border-border-default px-3 py-1.5">
        <span className="text-xs text-text-secondary">
          IPMI SOL — {config.host}
          {powerStatus && (
            <span className={powerStatus.powered_on ? "ml-2 text-status-connected" : "ml-2 text-status-disconnected"}>
              {powerStatus.powered_on ? "powered on" : "powered off"}
            </span>
          )}
        </span>
        <div className="flex items-center gap-1">
          {POWER_ACTIONS.map(({ action, label, icon: Icon, confirm }) => (
            <button
              key={action}
              onClick={() => handlePowerAction(action, label, confirm)}
              disabled={actingOn}
              title={label}
              className="flex items-center gap-1 rounded px-2 py-1 text-xs text-text-secondary hover:bg-surface-elevated hover:text-text-primary transition-colors disabled:opacity-50"
            >
              <Icon size={12} />
              {label}
            </button>
          ))}
        </div>
      </div>
      <div ref={containerRef} data-testid={`ipmi-sol-container-${sessionId}`} className="min-h-0 flex-1" style={{ padding: "4px 0 0 4px" }} />
    </div>
  );
}
