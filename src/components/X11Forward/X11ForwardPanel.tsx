import { useEffect, useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Loader2, AlertTriangle, MonitorSmartphone } from "lucide-react";
import type { X11ForwardConfig, X11ForwardOutputEvent } from "@/types";

interface X11ForwardPanelProps {
  readonly sessionId: string;
  readonly config: X11ForwardConfig;
}

export default function X11ForwardPanel({ sessionId, config }: X11ForwardPanelProps) {
  const [status, setStatus] = useState<"connecting" | "connected" | "error">("connecting");
  const [error, setError] = useState<string | null>(null);
  const [lines, setLines] = useState<string[]>([]);
  const connectionIdRef = useRef<string | null>(null);
  const logEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    setStatus("connecting");
    setError(null);

    invoke<string>("x11_forward_connect", { config })
      .then((id) => {
        if (cancelled) {
          invoke("x11_forward_disconnect", { id }).catch(() => {});
          return;
        }
        connectionIdRef.current = id;
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
        invoke("x11_forward_disconnect", { id }).catch(() => {});
        connectionIdRef.current = null;
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  useEffect(() => {
    if (status !== "connected") return;
    const connectionId = connectionIdRef.current;
    const outputUnlisten = listen<X11ForwardOutputEvent>("x11_forward:output", (event) => {
      if (event.payload.session_id === connectionId) {
        setLines((prev) => [...prev, event.payload.data]);
      }
    });
    const errorUnlisten = listen<X11ForwardOutputEvent>("x11_forward:error", (event) => {
      if (event.payload.session_id === connectionId) {
        setLines((prev) => [...prev, `[x11] ${event.payload.data}`]);
      }
    });
    return () => {
      outputUnlisten.then((fn) => fn());
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
        <span className="text-sm">Setting up X11 forwarding to {config.host}…</span>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't forward X11 to {config.host}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-2 overflow-hidden p-4">
      <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
        <MonitorSmartphone size={16} className="text-accent-primary" />
        X11 Forward — {config.host}
        <span className="text-xs font-normal text-text-disabled">running “{config.remote_command}” on display :{config.local_display}</span>
      </h2>
      <p className="text-xs text-text-secondary">
        The remote app's window renders in your local X server (e.g. XQuartz), not in this tab — this pane just shows connection status and any
        stdout/stderr the remote command prints.
      </p>
      <div className="flex-1 overflow-auto rounded-md border border-border-default bg-surface-sunken p-2 font-mono text-xs">
        {lines.length === 0 ? (
          <div className="p-2 text-text-disabled">No output yet.</div>
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
