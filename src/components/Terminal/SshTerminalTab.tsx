import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTerminalStore } from "@/stores/terminalStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useVaultStore } from "@/stores/vaultStore";
import { ConnectionStatus } from "@/types";
import { Loader2, KeyRound, Save } from "lucide-react";
import { useTranslation } from "react-i18next";
import SshTerminalView from "./SshTerminalView";
import ReconnectOverlay from "./ReconnectOverlay";

interface SshAuthPayload {
  type: "password" | "private_key" | "none";
  password?: string;
  key_data?: string;
  passphrase?: string;
}

interface AuthPromptInfo {
  prompt: string;
  echo: boolean;
}

interface AuthPromptEvent {
  connection_id: string;
  name: string;
  instructions: string;
  prompts: AuthPromptInfo[];
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
  const { t } = useTranslation();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const createTerminal = useTerminalStore((s) => s.createTerminal);
  const removeTerminal = useTerminalStore((s) => s.removeTerminal);
  const updateTabStatus = useSessionStore((s) => s.updateTabStatus);
  const [disconnectReason, setDisconnectReason] = useState<string | null>(null);

  // ── Interactive auth state ──
  const [authPrompt, setAuthPrompt] = useState<AuthPromptEvent | null>(null);
  const [authResponses, setAuthResponses] = useState<string[]>([]);
  const [authSubmitting, setAuthSubmitting] = useState(false);
  const [showSaveOffer, setShowSaveOffer] = useState(false);
  const lastAuthPassword = useRef<string | null>(null);
  const pendingConnId = useRef<string | null>(null);

  // ── Listen for auth prompts from backend ──
  useEffect(() => {
    const unlisten = listen<AuthPromptEvent>("ssh:auth_prompt", (event) => {
      if (
        pendingConnId.current &&
        event.payload.connection_id === pendingConnId.current
      ) {
        setAuthPrompt(event.payload);
        setAuthResponses(event.payload.prompts.map(() => ""));
        setAuthSubmitting(false);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function connect() {
      try {
        setLoading(true);
        setError(null);
        setAuthPrompt(null);

        // Generate a temporary connection ID reference so auth_prompt listener
        // can match events during the blocked ssh_connect call
        const tempId = `pending-${sessionId}-${Date.now()}`;
        pendingConnId.current = tempId;

        // The ssh_connect call may block if keyboard-interactive auth is needed.
        // The backend will emit ssh:auth_prompt events referencing the real
        // connection_id — but we won't know that ID until the command returns.
        // Instead, the backend uses the real connection_id in the event, and
        // the command only returns once auth succeeds. We need the listener to
        // accept any auth_prompt while we're connecting.
        pendingConnId.current = null; // We'll match on any prompt while connecting

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

        // Auth succeeded — if we went through interactive auth, offer to save
        if (lastAuthPassword.current) {
          setShowSaveOffer(true);
        }

        createTerminal(sessionId, connId);
        setConnectionId(connId);
        pendingConnId.current = null;
        const tabs = useSessionStore.getState().openTabs;
        const tab = tabs.find((tt) => tt.sessionId === sessionId);
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
          setAuthPrompt(null);
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

  // ── Auth prompt listener (match any prompt while connecting) ──
  useEffect(() => {
    if (!loading) return;

    const unlisten = listen<AuthPromptEvent>("ssh:auth_prompt", (event) => {
      // While loading (ssh_connect in progress), accept all auth prompts
      pendingConnId.current = event.payload.connection_id;
      setAuthPrompt(event.payload);
      setAuthResponses(event.payload.prompts.map(() => ""));
      setAuthSubmitting(false);
      setLoading(false); // Hide the loading spinner, show the prompt
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loading]);

  const handleAuthSubmit = async () => {
    if (!authPrompt) return;
    setAuthSubmitting(true);

    // Remember the password for vault save offer
    if (authPrompt.prompts.length === 1 && !authPrompt.prompts[0].echo) {
      lastAuthPassword.current = authResponses[0];
    }

    try {
      await invoke("ssh_auth_respond", {
        connectionId: authPrompt.connection_id,
        responses: authResponses,
      });
      // Don't clear prompt yet — the backend may send another round,
      // or ssh_connect will return (handled in the connect effect)
    } catch (e) {
      setError(String(e));
      setAuthPrompt(null);
    }
  };

  const handleSaveToVault = async () => {
    if (!lastAuthPassword.current) return;
    try {
      const activeVaultId = useVaultStore.getState().activeVaultId;
      if (activeVaultId) {
        await useVaultStore.getState().addCredential({
          name: `${username}@${host}:${port}`,
          credential_type: "password",
          username,
          data: { password: lastAuthPassword.current },
        });
      }
    } catch {
      // Silent — vault may be locked
    }
    setShowSaveOffer(false);
    lastAuthPassword.current = null;
  };

  // ── Auth prompt UI ──
  if (authPrompt) {
    return (
      <div className="flex items-center justify-center w-full h-full bg-surface-primary">
        <div className="flex flex-col items-center gap-4 max-w-sm w-full px-6">
          <div className="w-10 h-10 rounded-full bg-accent-primary/20 flex items-center justify-center">
            <KeyRound size={20} className="text-accent-primary" />
          </div>
          <p className="text-sm text-text-primary font-medium">
            {t("ssh.authRequired", `${username}@${host}:${port}`)}
          </p>
          {authPrompt.name && (
            <p className="text-xs text-text-secondary">{authPrompt.name}</p>
          )}
          {authPrompt.instructions && (
            <p className="text-xs text-text-secondary">
              {authPrompt.instructions}
            </p>
          )}
          <form
            className="w-full flex flex-col gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              handleAuthSubmit();
            }}
          >
            {authPrompt.prompts.map((p, i) => (
              <div key={p.prompt} className="flex flex-col gap-1">
                <label className="text-xs text-text-secondary">
                  {p.prompt}
                </label>
                <input
                  type={p.echo ? "text" : "password"}
                  autoFocus={i === 0}
                  value={authResponses[i] ?? ""}
                  onChange={(e) => {
                    setAuthResponses((prev) => {
                      const next = [...prev];
                      next[i] = e.target.value;
                      return next;
                    });
                  }}
                  className="w-full px-3 py-2 text-sm rounded bg-surface-elevated border border-border-default text-text-primary focus:border-accent-primary focus:outline-none"
                />
              </div>
            ))}
            <button
              type="submit"
              disabled={authSubmitting}
              className="mt-1 px-3 py-2 text-sm rounded bg-interactive-default hover:bg-interactive-hover text-text-primary disabled:opacity-50 transition-colors duration-[var(--duration-short)]"
            >
              {authSubmitting ? (
                <Loader2 size={14} className="animate-spin mx-auto" />
              ) : (
                t("ssh.authenticate", "Authenticate")
              )}
            </button>
          </form>
        </div>
      </div>
    );
  }

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
      {showSaveOffer && (
        <div className="absolute top-3 right-3 z-20 bg-surface-elevated border border-border-default rounded-lg shadow-lg p-3 flex items-center gap-3">
          <Save size={16} className="text-accent-primary shrink-0" />
          <span className="text-xs text-text-primary">
            {t("ssh.saveCredentials", "Save credentials to vault?")}
          </span>
          <button
            onClick={handleSaveToVault}
            className="px-2 py-1 text-xs rounded bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors"
          >
            {t("ssh.save", "Save")}
          </button>
          <button
            onClick={() => {
              setShowSaveOffer(false);
              lastAuthPassword.current = null;
            }}
            className="px-2 py-1 text-xs rounded text-text-secondary hover:text-text-primary transition-colors"
          >
            {t("ssh.dismiss", "Dismiss")}
          </button>
        </div>
      )}
    </div>
  );
}
