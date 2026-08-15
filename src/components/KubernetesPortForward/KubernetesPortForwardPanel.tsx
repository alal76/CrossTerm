import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Loader2, AlertTriangle, Network } from "lucide-react";
import type { K8sPortForwardConfig, K8sPortForwardConnectResult, K8sPortForwardConnEvent, K8sPortForwardErrorEvent } from "@/types";

interface KubernetesPortForwardPanelProps {
  readonly sessionId: string;
  readonly config: K8sPortForwardConfig;
}

export default function KubernetesPortForwardPanel({ sessionId, config }: KubernetesPortForwardPanelProps) {
  const [status, setStatus] = useState<"connecting" | "connected" | "error">("connecting");
  const [error, setError] = useState<string | null>(null);
  const [localPort, setLocalPort] = useState<number | null>(null);
  const [lines, setLines] = useState<string[]>([]);
  const connectionIdRef = useRef<string | null>(null);
  const logEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    setStatus("connecting");
    setError(null);

    invoke<K8sPortForwardConnectResult>("k8s_port_forward_connect", { config })
      .then((result) => {
        if (cancelled) {
          invoke("k8s_port_forward_disconnect", { id: result.id }).catch(() => {});
          return;
        }
        connectionIdRef.current = result.id;
        setLocalPort(result.local_port);
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
      const id = connectionIdRef.current;
      if (id) {
        invoke("k8s_port_forward_disconnect", { id }).catch(() => {});
        connectionIdRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  useEffect(() => {
    if (status !== "connected") return;
    const connectionId = connectionIdRef.current;
    const connUnlisten = listen<K8sPortForwardConnEvent>("k8s_port_forward:connection", (event) => {
      if (event.payload.session_id !== connectionId) return;
      setLines((prev) => [...prev, `${event.payload.peer} — ${event.payload.state}`]);
    });
    const errorUnlisten = listen<K8sPortForwardErrorEvent>("k8s_port_forward:error", (event) => {
      if (event.payload.session_id !== connectionId) return;
      setLines((prev) => [...prev, `[error] ${event.payload.message}`]);
    });
    return () => {
      connUnlisten.then((fn) => fn());
      errorUnlisten.then((fn) => fn());
    };
  }, [status]);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ block: "end" });
  }, [lines]);

  if (status === "connecting") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Forwarding to {config.pod_name}:{config.remote_port}…</span>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't forward to {config.pod_name}:{config.remote_port}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-2 overflow-hidden p-4">
      <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
        <Network size={16} className="text-accent-primary" />
        Port Forward — {config.namespace}/{config.pod_name}
      </h2>
      <p className="text-xs text-text-secondary">
        Listening on <span className="font-mono text-text-primary">127.0.0.1:{localPort}</span>, forwarding to port {config.remote_port} on the pod.
        Point any local client at that address — each connection opens a fresh forward to the pod.
      </p>
      <div className="flex-1 overflow-auto rounded-md border border-border-default bg-surface-sunken p-2 font-mono text-xs">
        {lines.length === 0 ? (
          <div className="p-2 text-text-disabled">No connections yet.</div>
        ) : (
          lines.map((line, i) => (
            <div key={i} className="whitespace-pre-wrap break-all text-text-primary">
              {line}
            </div>
          ))
        )}
        <div ref={logEndRef} />
      </div>
    </div>
  );
}
