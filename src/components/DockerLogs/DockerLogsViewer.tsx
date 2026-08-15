import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Loader2, AlertTriangle, Container } from "lucide-react";
import type { DockerLogsConfig, DockerLogLine } from "@/types";

interface DockerLogsViewerProps {
  readonly sessionId: string;
  readonly config: DockerLogsConfig;
}

const MAX_LINES = 2000;

export default function DockerLogsViewer({ sessionId, config }: DockerLogsViewerProps) {
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const connectionIdRef = useRef<string | null>(null);
  const [status, setStatus] = useState<"connecting" | "connected" | "error">("connecting");
  const [error, setError] = useState<string | null>(null);
  const [lines, setLines] = useState<DockerLogLine[]>([]);
  const logEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    setStatus("connecting");
    setError(null);

    invoke<string>("docker_logs_connect", { config })
      .then((id) => {
        if (cancelled) {
          invoke("docker_logs_disconnect", { id }).catch(() => {});
          return;
        }
        connectionIdRef.current = id;
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
      const id = connectionIdRef.current;
      if (id) invoke("docker_logs_disconnect", { id }).catch(() => {});
      connectionIdRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  useEffect(() => {
    if (!connectionId) return;
    const unlisten = listen<DockerLogLine>("docker_logs:line", (event) => {
      if (event.payload.session_id !== connectionId) return;
      setLines((prev) => {
        const next = [...prev, event.payload];
        return next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next;
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [connectionId]);

  useEffect(() => {
    logEndRef.current?.scrollIntoView({ block: "end" });
  }, [lines]);

  if (status === "connecting") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Streaming logs for {config.container_id.slice(0, 12)}…</span>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't stream logs for {config.container_id.slice(0, 12)}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-2 overflow-hidden p-4">
      <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
        <Container size={16} className="text-accent-primary" />
        Docker Logs — {config.container_id.slice(0, 12)}
      </h2>
      <div className="flex-1 overflow-auto rounded-md border border-border-default bg-surface-sunken p-2 font-mono text-xs">
        {lines.length === 0 ? (
          <div className="p-2 text-text-disabled">Waiting for log output…</div>
        ) : (
          lines.map((line, i) => (
            <div key={i} className="whitespace-pre-wrap break-all">
              {line.stream === "stderr" && <span className="mr-1 text-status-disconnected">[stderr]</span>}
              <span className="text-text-primary">{line.data}</span>
            </div>
          ))
        )}
        <div ref={logEndRef} />
      </div>
    </div>
  );
}
