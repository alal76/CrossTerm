import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTerminalStore } from "@/stores/terminalStore";
import { useSessionStore } from "@/stores/sessionStore";
import { ConnectionStatus } from "@/types";
import { Loader2 } from "lucide-react";
import SshTerminalView from "./SshTerminalView";
import ReconnectOverlay from "./ReconnectOverlay";

interface SshAuthPayload {
  type: "password";
  password: string;
}

interface SshTerminalTabProps {
  readonly sessionId: string;
  readonly isActive: boolean;
  readonly host: string;
  readonly port: number;
  readonly username: string;
  readonly auth: SshAuthPayload;
}

export default function SshTerminalTab({
  sessionId,
  isActive,
  host,
  port,
  username,
  auth,
}: SshTerminalTabProps) {
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const createTerminal = useTerminalStore((s) => s.createTerminal);
  const removeTerminal = useTerminalStore((s) => s.removeTerminal);
  const updateTabStatus = useSessionStore((s) => s.updateTabStatus);
  const [disconnectReason, setDisconnectReason] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function connect() {
      try {
        setLoading(true);
        setError(null);
        const connId = await invoke<string>("ssh_connect", {
          sessionId,
          host,
          port,
          username,
          auth,
        });
        if (cancelled) {
          invoke("ssh_disconnect", { connectionId: connId }).catch(() => {});
          return;
        }
        createTerminal(sessionId, connId);
        setConnectionId(connId);
        // Find the tab for this session and update its status
        const tabs = useSessionStore.getState().openTabs;
        const tab = tabs.find((t) => t.sessionId === sessionId);
        if (tab) {
          updateTabStatus(tab.id, ConnectionStatus.Connected);
        }
      } catch (e) {
        if (!cancelled) {
          setError(String(e));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    connect();

    return () => {
      cancelled = true;
      if (connectionId) {
        invoke("ssh_disconnect", { connectionId }).catch(() => {});
        removeTerminal(connectionId);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, host, port, username]);

  // Listen for disconnection events to update tab status
  useEffect(() => {
    if (!connectionId) return;

    const unlisten = listen<{ connection_id: string; reason: string }>(
      "ssh:disconnected",
      (event) => {
        if (event.payload.connection_id === connectionId) {
          const tabs = useSessionStore.getState().openTabs;
          const tab = tabs.find((t) => t.sessionId === sessionId);
          if (tab) {
            updateTabStatus(tab.id, ConnectionStatus.Disconnected);
          }
          setDisconnectReason(event.payload.reason);
        }
      }
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [connectionId, sessionId, updateTabStatus]);

  if (loading) {
    return (
      <div className="flex items-center justify-center w-full h-full bg-surface-primary">
        <div className="flex flex-col items-center gap-3 text-text-secondary">
          <Loader2 size={24} className="animate-spin text-accent-primary" />
          <span className="text-sm">
            Connecting to {username}@{host}:{port}…
          </span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center w-full h-full bg-surface-primary">
        <div className="flex flex-col items-center gap-3 max-w-sm text-center">
          <div className="w-10 h-10 rounded-full bg-status-disconnected/20 flex items-center justify-center">
            <span className="text-status-disconnected text-lg">!</span>
          </div>
          <p className="text-sm text-text-primary">
            Failed to connect to {host}:{port}
          </p>
          <p className="text-xs text-text-secondary">{error}</p>
          <button
            onClick={() => {
              setError(null);
              setLoading(true);
              invoke<string>("ssh_connect", { sessionId, host, port, username, auth })
                .then((connId) => {
                  createTerminal(sessionId, connId);
                  setConnectionId(connId);
                  setLoading(false);
                })
                .catch((e) => {
                  setError(String(e));
                  setLoading(false);
                });
            }}
            className="px-3 py-1.5 text-xs rounded bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            Retry
          </button>
        </div>
      </div>
    );
  }

  if (!connectionId) return null;

  const handleReconnect = async (): Promise<boolean> => {
    try {
      const connId = await invoke<string>("ssh_connect", {
        sessionId,
        host,
        port,
        username,
        auth,
      });
      removeTerminal(connectionId);
      createTerminal(sessionId, connId);
      setConnectionId(connId);
      setDisconnectReason(null);
      const tabs = useSessionStore.getState().openTabs;
      const tab = tabs.find((tt) => tt.sessionId === sessionId);
      if (tab) {
        updateTabStatus(tab.id, ConnectionStatus.Connected);
      }
      return true;
    } catch {
      return false;
    }
  };

  return (
    <div className="relative w-full h-full">
      <SshTerminalView connectionId={connectionId} isActive={isActive} />
      {disconnectReason !== null && (
        <ReconnectOverlay
          reason={disconnectReason}
          onReconnect={handleReconnect}
          onClose={() => setDisconnectReason(null)}
        />
      )}
    </div>
  );
}
