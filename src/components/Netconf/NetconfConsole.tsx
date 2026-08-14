import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, AlertTriangle, Network, Play, RefreshCw } from "lucide-react";
import type { NetconfConfig, NetconfSessionInfo, NetconfRpcResult } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface NetconfConsoleProps {
  readonly sessionId: string;
  readonly config: NetconfConfig;
}

type Datastore = "running" | "candidate" | "startup";
const DATASTORES: Datastore[] = ["running", "candidate", "startup"];

interface HistoryEntry extends NetconfRpcResult {
  label: string;
}

const MAX_HISTORY = 100;

export default function NetconfConsole({ sessionId, config }: NetconfConsoleProps) {
  const { toast } = useToast();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [sessionInfo, setSessionInfo] = useState<NetconfSessionInfo | null>(null);
  const [status, setStatus] = useState<"connecting" | "connected" | "error">("connecting");
  const [error, setError] = useState<string | null>(null);

  const [datastore, setDatastore] = useState<Datastore>("running");
  const [filter, setFilter] = useState("");
  const [rpcBody, setRpcBody] = useState("<get/>");
  const [running, setRunning] = useState(false);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const historyEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    setStatus("connecting");
    setError(null);

    invoke<string>("netconf_connect", { config })
      .then(async (id) => {
        if (cancelled) {
          invoke("netconf_disconnect", { id }).catch(() => {});
          return;
        }
        setConnectionId(id);
        setStatus("connected");
        try {
          const sessions = await invoke<NetconfSessionInfo[]>("netconf_list");
          setSessionInfo(sessions.find((s) => s.id === id) ?? null);
        } catch {
          // Session info is a nice-to-have for the capability list; the
          // console still works without it.
        }
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
        if (id) invoke("netconf_disconnect", { id }).catch(() => {});
        return null;
      });
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  useEffect(() => {
    historyEndRef.current?.scrollIntoView({ block: "end" });
  }, [history]);

  const pushHistory = useCallback((label: string, result: NetconfRpcResult) => {
    setHistory((prev) => {
      const next = [...prev, { ...result, label }];
      return next.length > MAX_HISTORY ? next.slice(next.length - MAX_HISTORY) : next;
    });
  }, []);

  const handleGetConfig = useCallback(async () => {
    if (!connectionId) return;
    setRunning(true);
    try {
      const result = await invoke<NetconfRpcResult>("netconf_get_config", {
        id: connectionId,
        datastore,
        filter: filter.trim() || null,
      });
      pushHistory(`get-config(${datastore})`, result);
      if (!result.ok) toast("error", result.error ?? "get-config returned an error");
    } catch (e) {
      toast("error", `get-config failed: ${String(e)}`);
    } finally {
      setRunning(false);
    }
  }, [connectionId, datastore, filter, pushHistory, toast]);

  const handleRunRpc = useCallback(async () => {
    if (!connectionId || !rpcBody.trim()) return;
    setRunning(true);
    try {
      const result = await invoke<NetconfRpcResult>("netconf_rpc", { id: connectionId, xmlBody: rpcBody });
      pushHistory("rpc", result);
      if (!result.ok) toast("error", result.error ?? "RPC returned an error");
    } catch (e) {
      toast("error", `RPC failed: ${String(e)}`);
    } finally {
      setRunning(false);
    }
  }, [connectionId, rpcBody, pushHistory, toast]);

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
        <Network size={16} className="text-accent-primary" />
        NETCONF — {config.host}:{config.port}
        {sessionInfo && <span className="text-xs font-normal text-text-disabled">session {sessionInfo.session_id}</span>}
      </h2>

      {sessionInfo && sessionInfo.server_capabilities.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {sessionInfo.server_capabilities.slice(0, 12).map((cap) => (
            <span key={cap} className="rounded bg-surface-elevated px-1.5 py-0.5 text-[10px] text-text-disabled" title={cap}>
              {cap.replace("urn:ietf:params:netconf:capability:", "").replace("urn:ietf:params:netconf:base:", "base:")}
            </span>
          ))}
        </div>
      )}

      <div className="flex flex-col gap-2 rounded-md border border-border-default p-2">
        <div className="flex items-center gap-2">
          <span className="text-xs text-text-secondary">get-config</span>
          <select
            value={datastore}
            onChange={(e) => setDatastore(e.target.value as Datastore)}
            className="rounded-md border border-border-default bg-surface-primary px-2 py-1 text-xs text-text-primary"
          >
            {DATASTORES.map((ds) => (
              <option key={ds} value={ds}>{ds}</option>
            ))}
          </select>
          <input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Optional subtree filter XML"
            className="flex-1 rounded-md border border-border-default bg-surface-primary px-2 py-1 text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
          />
          <button
            onClick={handleGetConfig}
            disabled={running}
            className="flex items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1 text-xs font-medium text-text-inverse hover:bg-interactive-hover transition-colors disabled:opacity-50"
          >
            <RefreshCw size={12} className={running ? "animate-spin" : ""} />
            Run
          </button>
        </div>
        <div className="flex items-start gap-2">
          <span className="pt-1.5 text-xs text-text-secondary">rpc</span>
          <textarea
            value={rpcBody}
            onChange={(e) => setRpcBody(e.target.value)}
            rows={2}
            spellCheck={false}
            placeholder="<get/>"
            className="flex-1 resize-y rounded-md border border-border-default bg-surface-primary px-2 py-1 font-mono text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
          />
          <button
            onClick={handleRunRpc}
            disabled={running || !rpcBody.trim()}
            className="flex items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1 text-xs font-medium text-text-inverse hover:bg-interactive-hover transition-colors disabled:opacity-50"
          >
            <Play size={12} />
            Send
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-auto rounded-md border border-border-default bg-surface-sunken p-2 font-mono text-xs">
        {history.length === 0 ? (
          <div className="p-2 text-text-disabled">No RPCs run yet.</div>
        ) : (
          history.map((entry, i) => (
            <div key={i} className="border-b border-border-subtle py-1 last:border-0">
              <div className="flex items-center gap-2">
                <span className={entry.ok ? "text-status-connected" : "text-status-disconnected"}>
                  {entry.ok ? "ok" : "error"}
                </span>
                <span className="text-accent-primary">{entry.label}</span>
                {entry.error && <span className="text-text-disabled">{entry.error}</span>}
              </div>
              <pre className="whitespace-pre-wrap break-all text-text-secondary">{entry.xml}</pre>
            </div>
          ))
        )}
        <div ref={historyEndRef} />
      </div>
    </div>
  );
}
