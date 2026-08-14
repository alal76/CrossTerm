import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, AlertTriangle, Terminal as TerminalIcon, Play } from "lucide-react";
import type { WinRmConfig, WinRmCommandResult } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface WinRmConsoleProps {
  readonly sessionId: string;
  readonly config: WinRmConfig;
}

interface HistoryEntry extends WinRmCommandResult {
  command: string;
}

const MAX_HISTORY = 200;

export default function WinRmConsole({ sessionId, config }: WinRmConsoleProps) {
  const { toast } = useToast();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [status, setStatus] = useState<"connecting" | "connected" | "error">("connecting");
  const [error, setError] = useState<string | null>(null);

  const [command, setCommand] = useState("");
  const [running, setRunning] = useState(false);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const historyEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    setStatus("connecting");
    setError(null);

    invoke<string>("winrm_connect", { config })
      .then((id) => {
        if (cancelled) {
          invoke("winrm_disconnect", { id }).catch(() => {});
          return;
        }
        setConnectionId(id);
        setStatus("connected");
      })
      .catch((e) => {
        if (!cancelled) {
          setStatus("error");
          setError(String(e));
        }
      });

    return () => {
      cancelled = true;
      setConnectionId((id) => {
        if (id) invoke("winrm_disconnect", { id }).catch(() => {});
        return null;
      });
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  useEffect(() => {
    historyEndRef.current?.scrollIntoView({ block: "end" });
  }, [history]);

  const handleRun = useCallback(async () => {
    const cmd = command.trim();
    if (!connectionId || !cmd) return;
    setRunning(true);
    try {
      const result = await invoke<WinRmCommandResult>("winrm_run_command", { id: connectionId, command: cmd });
      setHistory((prev) => {
        const next = [...prev, { ...result, command: cmd }];
        return next.length > MAX_HISTORY ? next.slice(next.length - MAX_HISTORY) : next;
      });
      setCommand("");
      if (result.exit_code !== 0) toast("error", `Exited ${result.exit_code}`);
    } catch (e) {
      toast("error", `Command failed: ${String(e)}`);
    } finally {
      setRunning(false);
    }
  }, [connectionId, command, toast]);

  if (status === "connecting") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Connecting to {config.host}:{config.port}…</span>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't connect to {config.host}:{config.port}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-3 overflow-hidden p-4">
      <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
        <TerminalIcon size={16} className="text-accent-primary" />
        WinRM — {config.host}:{config.port}
        <span className="text-xs font-normal text-text-disabled">{config.auth}</span>
      </h2>

      <div className="flex-1 overflow-auto rounded-md border border-border-default bg-surface-sunken p-2 font-mono text-xs">
        {history.length === 0 ? (
          <div className="p-2 text-text-disabled">No commands run yet.</div>
        ) : (
          history.map((entry, i) => (
            <div key={i} className="border-b border-border-subtle py-1 last:border-0">
              <div className="flex items-center gap-2">
                <span className="text-accent-primary">&gt;</span>
                <span className="text-text-primary">{entry.command}</span>
                <span className={entry.exit_code === 0 ? "text-status-connected" : "text-status-disconnected"}>
                  exit {entry.exit_code}
                </span>
              </div>
              {entry.stdout && <pre className="whitespace-pre-wrap break-all text-text-secondary">{entry.stdout}</pre>}
              {entry.stderr && <pre className="whitespace-pre-wrap break-all text-status-disconnected">{entry.stderr}</pre>}
            </div>
          ))
        )}
        <div ref={historyEndRef} />
      </div>

      <div className="flex items-center gap-2">
        <input
          value={command}
          onChange={(e) => setCommand(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleRun()}
          placeholder="e.g. ipconfig /all"
          spellCheck={false}
          className="flex-1 rounded-md border border-border-default bg-surface-primary px-2 py-1.5 font-mono text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
        />
        <button
          onClick={handleRun}
          disabled={running || !command.trim()}
          className="flex items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1.5 text-xs font-medium text-text-inverse hover:bg-interactive-hover transition-colors disabled:opacity-50"
        >
          {running ? <Loader2 size={12} className="animate-spin" /> : <Play size={12} />}
          Run
        </button>
      </div>
    </div>
  );
}
