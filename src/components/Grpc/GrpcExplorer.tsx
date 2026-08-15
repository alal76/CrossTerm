import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Loader2, AlertTriangle, Network, Play, RefreshCw, ChevronRight } from "lucide-react";
import type { GrpcConfig, GrpcService, GrpcMethod, GrpcRpcResult } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface GrpcExplorerProps {
  readonly sessionId: string;
  readonly config: GrpcConfig;
}

export default function GrpcExplorer({ sessionId, config }: GrpcExplorerProps) {
  const { toast } = useToast();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [status, setStatus] = useState<"connecting" | "connected" | "error">("connecting");
  const [error, setError] = useState<string | null>(null);

  const [services, setServices] = useState<string[]>([]);
  const [loadingServices, setLoadingServices] = useState(false);
  const [expandedService, setExpandedService] = useState<string | null>(null);
  const [serviceMethods, setServiceMethods] = useState<Record<string, GrpcMethod[]>>({});

  const [selected, setSelected] = useState<{ service: string; method: GrpcMethod } | null>(null);
  const [requestBody, setRequestBody] = useState("{}");
  const [invoking, setInvoking] = useState(false);
  const [result, setResult] = useState<GrpcRpcResult | null>(null);

  useEffect(() => {
    let cancelled = false;
    setStatus("connecting");
    setError(null);

    invoke<string>("grpc_connect", { config })
      .then((id) => {
        if (cancelled) {
          invoke("grpc_disconnect", { id }).catch(() => {});
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
        if (id) invoke("grpc_disconnect", { id }).catch(() => {});
        return null;
      });
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  const loadServices = useCallback(async () => {
    if (!connectionId) return;
    setLoadingServices(true);
    try {
      const list = await invoke<string[]>("grpc_list_services", { id: connectionId });
      setServices(list);
    } catch (e) {
      toast("error", `Reflection failed: ${String(e)}`);
    } finally {
      setLoadingServices(false);
    }
  }, [connectionId, toast]);

  useEffect(() => {
    if (status === "connected") loadServices();
  }, [status, loadServices]);

  const toggleService = useCallback(
    async (service: string) => {
      if (expandedService === service) {
        setExpandedService(null);
        return;
      }
      setExpandedService(service);
      if (!connectionId || serviceMethods[service]) return;
      try {
        const desc = await invoke<GrpcService>("grpc_describe_service", { id: connectionId, service });
        setServiceMethods((prev) => ({ ...prev, [service]: desc.methods }));
      } catch (e) {
        toast("error", `Describe failed: ${String(e)}`);
      }
    },
    [connectionId, expandedService, serviceMethods, toast]
  );

  const selectMethod = useCallback((service: string, method: GrpcMethod) => {
    setSelected({ service, method });
    setRequestBody("{}");
    setResult(null);
  }, []);

  const handleInvoke = useCallback(async () => {
    if (!connectionId || !selected) return;
    setInvoking(true);
    try {
      const res = await invoke<GrpcRpcResult>("grpc_invoke", {
        id: connectionId,
        service: selected.service,
        method: selected.method.name,
        jsonBody: requestBody,
      });
      setResult(res);
      if (res.status_code !== 0) toast("error", `Status ${res.status_code}: ${res.message}`);
    } catch (e) {
      toast("error", `Invoke failed: ${String(e)}`);
    } finally {
      setInvoking(false);
    }
  }, [connectionId, selected, requestBody, toast]);

  if (status === "connecting") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Connecting to {config.endpoint}…</span>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't connect to {config.endpoint}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full overflow-hidden">
      <div className="flex w-64 shrink-0 flex-col border-r border-border-default">
        <div className="flex items-center justify-between border-b border-border-subtle px-3 py-2">
          <span className="flex items-center gap-1.5 text-xs font-semibold text-text-primary">
            <Network size={14} className="text-accent-primary" />
            Services
          </span>
          <button onClick={loadServices} disabled={loadingServices} title="Refresh" className="text-text-secondary hover:text-text-primary disabled:opacity-50">
            <RefreshCw size={12} className={loadingServices ? "animate-spin" : ""} />
          </button>
        </div>
        <div className="flex-1 overflow-auto p-1 text-xs">
          {services.length === 0 && !loadingServices && <div className="p-2 text-text-disabled">No services found via reflection.</div>}
          {services.map((service) => (
            <div key={service}>
              <button
                onClick={() => toggleService(service)}
                className="flex w-full items-center gap-1 rounded px-2 py-1.5 text-left text-text-primary hover:bg-surface-elevated"
              >
                <ChevronRight size={12} className={`shrink-0 transition-transform ${expandedService === service ? "rotate-90" : ""}`} />
                <span className="truncate">{service}</span>
              </button>
              {expandedService === service && (
                <div className="ml-4 flex flex-col gap-0.5 border-l border-border-subtle pl-2">
                  {(serviceMethods[service] ?? []).map((method) => (
                    <button
                      key={method.name}
                      onClick={() => selectMethod(service, method)}
                      className={`truncate rounded px-2 py-1 text-left hover:bg-surface-elevated ${
                        selected?.service === service && selected.method.name === method.name ? "bg-surface-elevated text-accent-primary" : "text-text-secondary"
                      }`}
                    >
                      {method.name}
                      {(method.client_streaming || method.server_streaming) && <span className="ml-1 text-text-disabled">(stream)</span>}
                    </button>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      </div>

      <div className="flex flex-1 flex-col gap-3 overflow-hidden p-4">
        {!selected ? (
          <div className="flex flex-1 items-center justify-center text-sm text-text-disabled">Select a method to invoke it.</div>
        ) : (
          <>
            <h2 className="text-sm font-semibold text-text-primary">
              {selected.service}
              <span className="text-text-disabled"> / </span>
              {selected.method.name}
            </h2>
            <div className="text-xs text-text-disabled">
              {selected.method.input_type} → {selected.method.output_type}
            </div>

            {selected.method.client_streaming || selected.method.server_streaming ? (
              <div className="flex flex-1 items-center justify-center text-sm text-text-disabled">Streaming methods aren't supported yet — only unary RPCs can be invoked.</div>
            ) : (
              <>
                <textarea
                  value={requestBody}
                  onChange={(e) => setRequestBody(e.target.value)}
                  rows={8}
                  spellCheck={false}
                  placeholder="{}"
                  className="w-full flex-1 resize-none rounded-md border border-border-default bg-surface-primary p-2 font-mono text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
                />
                <button
                  onClick={handleInvoke}
                  disabled={invoking}
                  className="flex w-fit items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1.5 text-xs font-medium text-text-inverse hover:bg-interactive-hover transition-colors disabled:opacity-50"
                >
                  {invoking ? <Loader2 size={12} className="animate-spin" /> : <Play size={12} />}
                  Invoke
                </button>
                {result && (
                  <div className="flex-1 overflow-auto rounded-md border border-border-default bg-surface-sunken p-2 font-mono text-xs">
                    <div className={result.status_code === 0 ? "text-status-connected" : "text-status-disconnected"}>
                      status {result.status_code} — {result.message}
                    </div>
                    <pre className="whitespace-pre-wrap break-all text-text-secondary">{result.body}</pre>
                  </div>
                )}
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
}
