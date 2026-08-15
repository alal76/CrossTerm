import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, AlertTriangle, Router, Play, ListTree } from "lucide-react";
import type { SnmpConfig, SnmpVarBind } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface SnmpBrowserProps {
  readonly sessionId: string;
  readonly config: SnmpConfig;
}

const SYS_DESCR_OID = "1.3.6.1.2.1.1.1.0";
const MIB2_ROOT_OID = "1.3.6.1.2.1";

export default function SnmpBrowser({ sessionId, config }: SnmpBrowserProps) {
  const { toast } = useToast();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [getOid, setGetOid] = useState(SYS_DESCR_OID);
  const [walkOid, setWalkOid] = useState(MIB2_ROOT_OID);
  const [maxVars, setMaxVars] = useState(50);
  const [running, setRunning] = useState(false);
  const [results, setResults] = useState<SnmpVarBind[]>([]);

  useEffect(() => {
    let cancelled = false;
    setError(null);

    invoke<string>("snmp_add_session", { config })
      .then((id) => {
        if (!cancelled) setConnectionId(id);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });

    return () => {
      cancelled = true;
      setConnectionId((id) => {
        if (id) invoke("snmp_remove_session", { id }).catch(() => {});
        return null;
      });
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  const handleGet = useCallback(async () => {
    if (!connectionId || !getOid.trim()) return;
    setRunning(true);
    try {
      const varbinds = await invoke<SnmpVarBind[]>("snmp_get", { id: connectionId, oid: getOid.trim() });
      setResults(varbinds);
    } catch (e) {
      toast("error", `GET failed: ${String(e)}`);
    } finally {
      setRunning(false);
    }
  }, [connectionId, getOid, toast]);

  const handleWalk = useCallback(async () => {
    if (!connectionId || !walkOid.trim()) return;
    setRunning(true);
    try {
      const varbinds = await invoke<SnmpVarBind[]>("snmp_walk", { id: connectionId, rootOid: walkOid.trim(), maxVars });
      setResults(varbinds);
      if (varbinds.length === 0) toast("error", "Walk returned no results");
    } catch (e) {
      toast("error", `WALK failed: ${String(e)}`);
    } finally {
      setRunning(false);
    }
  }, [connectionId, walkOid, maxVars, toast]);

  if (error) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't set up SNMP session for {config.host}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  if (!connectionId) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Setting up SNMP session…</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-3 overflow-hidden p-4">
      <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
        <Router size={16} className="text-accent-primary" />
        SNMP {config.version} — {config.host}:{config.port}
      </h2>

      <div className="flex flex-col gap-2 rounded-md border border-border-default p-2">
        <div className="flex items-center gap-2">
          <span className="w-12 shrink-0 text-xs text-text-secondary">get</span>
          <input
            value={getOid}
            onChange={(e) => setGetOid(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleGet()}
            placeholder="OID, e.g. 1.3.6.1.2.1.1.1.0"
            spellCheck={false}
            className="flex-1 rounded-md border border-border-default bg-surface-primary px-2 py-1 font-mono text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
          />
          <button
            onClick={handleGet}
            disabled={running || !getOid.trim()}
            className="flex items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1 text-xs font-medium text-text-inverse hover:bg-interactive-hover transition-colors disabled:opacity-50"
          >
            <Play size={12} />
            Get
          </button>
        </div>
        <div className="flex items-center gap-2">
          <span className="w-12 shrink-0 text-xs text-text-secondary">walk</span>
          <input
            value={walkOid}
            onChange={(e) => setWalkOid(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleWalk()}
            placeholder="Root OID, e.g. 1.3.6.1.2.1"
            spellCheck={false}
            className="flex-1 rounded-md border border-border-default bg-surface-primary px-2 py-1 font-mono text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
          />
          <input
            type="number"
            min={1}
            max={100}
            value={maxVars}
            onChange={(e) => setMaxVars(Number(e.target.value))}
            className="w-16 rounded-md border border-border-default bg-surface-primary px-2 py-1 text-xs text-text-primary focus:border-border-focus focus:outline-none"
          />
          <button
            onClick={handleWalk}
            disabled={running || !walkOid.trim()}
            className="flex items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1 text-xs font-medium text-text-inverse hover:bg-interactive-hover transition-colors disabled:opacity-50"
          >
            <ListTree size={12} />
            Walk
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-auto rounded-md border border-border-default">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-surface-secondary">
            <tr>
              <th className="px-2 py-1.5 text-left font-medium text-text-secondary">OID</th>
              <th className="px-2 py-1.5 text-left font-medium text-text-secondary">Type</th>
              <th className="px-2 py-1.5 text-left font-medium text-text-secondary">Value</th>
            </tr>
          </thead>
          <tbody>
            {running && results.length === 0 ? (
              <tr>
                <td colSpan={3} className="p-4 text-center text-text-disabled">
                  <Loader2 size={14} className="mx-auto animate-spin" />
                </td>
              </tr>
            ) : results.length === 0 ? (
              <tr>
                <td colSpan={3} className="p-4 text-center text-text-disabled">
                  No results yet.
                </td>
              </tr>
            ) : (
              results.map((vb, i) => (
                <tr key={i} className="border-t border-border-subtle hover:bg-surface-secondary">
                  <td className="px-2 py-1 font-mono text-accent-primary">{vb.oid}</td>
                  <td className="px-2 py-1 text-text-disabled">{vb.value_type}</td>
                  <td className="max-w-xs truncate px-2 py-1 text-text-primary" title={vb.value}>
                    {vb.value}
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
