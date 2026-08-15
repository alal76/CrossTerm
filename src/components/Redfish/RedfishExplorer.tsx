import { useEffect, useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, AlertTriangle, Power, PowerOff, RotateCcw, Zap, RefreshCw, Server } from "lucide-react";
import type { RedfishConfig, RedfishSystem, RedfishPowerAction } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface RedfishExplorerProps {
  readonly sessionId: string;
  readonly config: RedfishConfig;
}

const POWER_ACTIONS: { action: RedfishPowerAction; label: string; icon: typeof Power; confirm?: boolean }[] = [
  { action: "On", label: "Power On", icon: Power },
  { action: "GracefulRestart", label: "Restart", icon: RotateCcw },
  { action: "GracefulShutdown", label: "Shutdown", icon: PowerOff },
  { action: "ForceRestart", label: "Force Restart", icon: RotateCcw, confirm: true },
  { action: "ForceOff", label: "Force Off", icon: PowerOff, confirm: true },
  { action: "Nmi", label: "Send NMI", icon: Zap, confirm: true },
];

export default function RedfishExplorer({ sessionId, config }: RedfishExplorerProps) {
  const { toast } = useToast();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const connectionIdRef = useRef<string | null>(null);
  const [systems, setSystems] = useState<RedfishSystem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actingOn, setActingOn] = useState<string | null>(null);

  const loadSystems = useCallback(async (id: string) => {
    setLoading(true);
    try {
      const result = await invoke<RedfishSystem[]>("redfish_get_systems", { id });
      setSystems(result);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    invoke<string>("redfish_connect", { config })
      .then(async (id) => {
        if (cancelled) {
          invoke("redfish_disconnect", { id }).catch(() => {});
          return;
        }
        connectionIdRef.current = id;
        setConnectionId(id);
        await loadSystems(id);
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e));
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
      const id = connectionIdRef.current;
      if (id) invoke("redfish_disconnect", { id }).catch(() => {});
      connectionIdRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  const handlePowerAction = useCallback(
    async (systemId: string, action: RedfishPowerAction, requiresConfirm: boolean | undefined, label: string) => {
      if (!connectionId) return;
      if (requiresConfirm && !window.confirm(`${label} on ${systemId}? This cannot be undone.`)) return;
      setActingOn(systemId);
      try {
        await invoke("redfish_power_control", { id: connectionId, systemId, action });
        toast("success", `${label} sent to ${systemId}`);
        await loadSystems(connectionId);
      } catch (e) {
        toast("error", `${label} failed: ${String(e)}`);
      } finally {
        setActingOn(null);
      }
    },
    [connectionId, loadSystems, toast]
  );

  if (loading && systems.length === 0) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Connecting to {config.host}…</span>
      </div>
    );
  }

  if (error && systems.length === 0) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't reach the Redfish service on {config.host}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-3 overflow-auto p-4">
      <div className="flex items-center justify-between">
        <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
          <Server size={16} className="text-accent-primary" />
          Redfish — {config.host}
        </h2>
        <button
          onClick={() => connectionId && loadSystems(connectionId)}
          disabled={loading}
          className="flex items-center gap-1.5 rounded-md border border-border-default px-2 py-1 text-xs text-text-secondary hover:text-text-primary transition-colors disabled:opacity-50"
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
          Refresh
        </button>
      </div>

      {systems.length === 0 ? (
        <div className="flex flex-1 items-center justify-center text-sm text-text-disabled">No systems found.</div>
      ) : (
        <div className="flex flex-col gap-3">
          {systems.map((system) => (
            <div key={system.id} className="rounded-md border border-border-default bg-surface-secondary p-3">
              <div className="mb-2 flex items-center justify-between">
                <div>
                  <div className="text-sm font-medium text-text-primary">{system.name}</div>
                  <div className="text-xs text-text-secondary">
                    {[system.manufacturer, system.model, system.serial].filter(Boolean).join(" · ") || system.id}
                  </div>
                </div>
                <span className="rounded bg-surface-elevated px-2 py-1 text-xs text-text-secondary">
                  {system.power_state ?? "unknown"}
                </span>
              </div>
              <div className="flex flex-wrap gap-1.5">
                {POWER_ACTIONS.map(({ action, label, icon: Icon, confirm }) => (
                  <button
                    key={action}
                    onClick={() => handlePowerAction(system.id, action, confirm, label)}
                    disabled={actingOn === system.id}
                    className="flex items-center gap-1 rounded px-2 py-1 text-xs text-text-secondary hover:bg-surface-elevated hover:text-text-primary transition-colors disabled:opacity-50"
                  >
                    <Icon size={11} />
                    {label}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
