import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTerminalStore } from "@/stores/terminalStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useVaultStore } from "@/stores/vaultStore";
import {
  ConnectionStatus,
  type SshConnectLogEvent,
  type SshBannerEvent,
  type SshAuthSuccessEvent,
} from "@/types";
import { Loader2, KeyRound, Save, Terminal, AlertTriangle, Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import SshTerminalView from "./SshTerminalView";
import ReconnectOverlay from "./ReconnectOverlay";
import ComplianceBanner from "@/components/Shared/ComplianceBanner";
import RecordingControls from "@/components/Recording/RecordingControls";
import CommandAssistant from "./CommandAssistant";

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
  readonly credentialRef?: string;
}

export default function SshTerminalTab({
  sessionId,
  isActive,
  host,
  port,
  username,
  auth,
  credentialRef,
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

  // ── Connection log state ──
  const [connectLogs, setConnectLogs] = useState<SshConnectLogEvent[]>([]);
  const [banner, setBanner] = useState<string | null>(null);
  // Track auth method for auto-save
  const authSuccessInfo = useRef<SshAuthSuccessEvent | null>(null);

  // ── Credential prompt state (when connecting with none auth and auth is required) ──
  const [showCredentialPrompt, setShowCredentialPrompt] = useState(false);
  const [credUsername, setCredUsername] = useState(username);
  const [credPassword, setCredPassword] = useState("");
  const [credSubmitting, setCredSubmitting] = useState(false);

  // ── Compliance recording (policy-driven) ──
  // Recording is opt-in per PolicyPanel's "recording.enabled" +
  // "require_recording_for" host patterns. When active, terminal output is
  // streamed into the recording via ssh:output (below) so it isn't just an
  // empty capture, and ComplianceBanner is shown when the policy also has
  // notify_user set.
  const recordingIdRef = useRef<string | null>(null);
  const [recordingActive, setRecordingActive] = useState(false);
  const [recordingNotify, setRecordingNotify] = useState(false);
  const [recordingAllowDisable, setRecordingAllowDisable] = useState(false);

  const startRecordingIfRequired = useCallback(async () => {
    try {
      const required = await invoke<boolean>("policy_check_recording_required", { hostname: host });
      if (!required) return;
      const id = await invoke<string>("recording_start", {
        sessionId,
        title: `${username}@${host}`,
        width: 80,
        height: 24,
      });
      recordingIdRef.current = id;
      setRecordingActive(true);
      try {
        const policy = await invoke<{ recording: { notify_user: boolean; allow_user_disable: boolean } }>("policy_get");
        setRecordingNotify(policy.recording.notify_user);
        setRecordingAllowDisable(policy.recording.allow_user_disable);
      } catch {
        // Policy details are cosmetic (banner visibility only) — recording itself already started.
      }
    } catch {
      // Recording is best-effort; a policy-check or recording-start failure must not fail the connection.
    }
  }, [host, sessionId, username]);

  const stopRecording = useCallback(async () => {
    const id = recordingIdRef.current;
    if (!id) return;
    recordingIdRef.current = null;
    setRecordingActive(false);
    try {
      await invoke("recording_stop", { recordingId: id });
    } catch {
      // Best-effort cleanup.
    }
  }, []);

  // User-initiated recording (RecordingControls) shares the same
  // recordingIdRef/append-listener as policy-driven recording above, so
  // output is captured regardless of which path started it. It's only
  // offered when policy recording isn't already running (see JSX below).
  const handleManualRecordingIdChange = useCallback((id: string | null) => {
    recordingIdRef.current = id;
  }, []);

  // ── AI command assistant ──
  const [showAssistant, setShowAssistant] = useState(false);
  const handleInsertCommand = useCallback(
    (command: string) => {
      invoke("ssh_write", { connectionId, data: command }).catch(() => {});
    },
    [connectionId],
  );

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

    // ── Scoped listeners for this connection attempt ──
    const unlisteners = [
      listen<SshConnectLogEvent>("ssh:connect_log", (event) => {
        if (!cancelled) setConnectLogs((prev) => [...prev, event.payload]);
      }),
      listen<SshBannerEvent>("ssh:banner", (event) => {
        if (!cancelled) setBanner(event.payload.banner);
      }),
      listen<SshAuthSuccessEvent>("ssh:auth_success", (event) => {
        if (!cancelled) { authSuccessInfo.current = event.payload; }
      }),
      listen<{ host: string; port: number; algorithm: string; fingerprint: string }>(
        "ssh:host_key_new",
        (event) => {
          if (cancelled) return;
          if (event.payload.host !== host || event.payload.port !== port) return;
          setConnectLogs((prev) => [
            ...prev,
            {
              level: "warn",
              message: `New host key trusted (TOFU): ${event.payload.algorithm} ${event.payload.fingerprint}`,
            } as SshConnectLogEvent,
          ]);
        },
      ),
    ];

    async function connect() {
      try {
        setLoading(true);
        setError(null);
        setAuthPrompt(null);
        setConnectLogs([]);
        setBanner(null);
        setShowCredentialPrompt(false);
        authSuccessInfo.current = null;

        // Resolve auth: if we have a credentialRef, try fetching from vault
        let resolvedAuth = auth;
        if (auth.type === "none" && credentialRef) {
          const cred = await useVaultStore.getState().getCredential(credentialRef);
          if (cred) {
            if (cred.credential_type === "password" && cred.data?.password) {
              resolvedAuth = { type: "password", password: cred.data.password as string };
            } else if (cred.credential_type === "private_key" && cred.data?.key_data) {
              resolvedAuth = {
                type: "private_key",
                key_data: cred.data.key_data as string,
                passphrase: (cred.data.passphrase as string) ?? undefined,
              };
            }
          }
        }

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
          auth: resolvedAuth,
        });
        if (cancelled) {
          invoke("ssh_disconnect", { connectionId: connId }).catch(() => {});
          return;
        }

        // Auth succeeded — offer to save credential (keyboard-interactive or password)
        const successAuthMethod = (authSuccessInfo.current as { auth_method?: string } | null)?.auth_method;
        if (lastAuthPassword.current) {
          setShowSaveOffer(true);
        } else if (
          auth.type === "password" &&
          auth.password &&
          successAuthMethod === "password"
        ) {
          lastAuthPassword.current = auth.password;
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
        void startRecordingIfRequired();
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
      for (const p of unlisteners) { p.then((fn) => fn()); }
      void stopRecording();
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
          void stopRecording();
        }
      }
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [connectionId, sessionId, updateTabStatus, stopRecording]);

  // Stream terminal output into the active recording (if any). Separate
  // from SshTerminalView's own ssh:output listener, which handles writing
  // to the visible terminal — Tauri events support multiple listeners.
  useEffect(() => {
    if (!connectionId) return;
    const unlisten = listen<{ connection_id: string; data: string }>(
      "ssh:output",
      (event) => {
        if (event.payload.connection_id !== connectionId) return;
        if (!recordingIdRef.current) return;
        invoke("recording_append", { recordingId: recordingIdRef.current, data: event.payload.data }).catch(() => {});
      }
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [connectionId]);

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

  const handleCredentialRetry = useCallback(async () => {
    if (!credPassword) return;
    setCredSubmitting(true);
    setError(null);
    setShowCredentialPrompt(false);
    setLoading(true);
    setConnectLogs([]);
    authSuccessInfo.current = null;
    try {
      const newAuth: SshAuthPayload = { type: "password", password: credPassword };
      const connId = await invoke<string>("ssh_connect", {
        sessionId,
        host,
        port,
        username: credUsername || username,
        auth: newAuth,
      });
      lastAuthPassword.current = credPassword;
      setShowSaveOffer(true);
      createTerminal(sessionId, connId);
      setConnectionId(connId);
      pendingConnId.current = null;
      const tabs = useSessionStore.getState().openTabs;
      const tab = tabs.find((tt) => tt.sessionId === sessionId);
      if (tab) {
        updateTabStatus(tab.id, ConnectionStatus.Connected);
      }
      void startRecordingIfRequired();
    } catch (e) {
      setError(String(e));
      setShowCredentialPrompt(true);
    } finally {
      setLoading(false);
      setCredSubmitting(false);
    }
  }, [credUsername, credPassword, sessionId, host, port, username, createTerminal, updateTabStatus]);

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
        const credId = await useVaultStore.getState().addCredential({
          name: `${username}@${host}:${port}`,
          credential_type: "password",
          username,
          data: { password: lastAuthPassword.current },
        });
        // Link the saved credential back to the session so future
        // connections auto-retrieve it from the vault.
        useSessionStore.getState().updateSession(sessionId, { credentialRef: credId });
        invoke("session_update", {
          id: sessionId,
          request: { credential_ref: credId },
        }).catch(() => {});
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
        <div className="flex flex-col items-center gap-4 max-w-md w-full px-6">
          <Loader2 size={24} className="animate-spin text-accent-primary" />
          <span className="text-sm text-text-secondary">
            {t("ssh.connectingTo", { target: `${username}@${host}:${port}` })}
          </span>
          {banner && (
            <div className="w-full rounded border border-border-default bg-surface-elevated p-3">
              <p className="text-xs text-text-secondary font-medium mb-1">{t("ssh.banner")}</p>
              <pre className="text-xs text-text-primary whitespace-pre-wrap">{banner}</pre>
            </div>
          )}
          {connectLogs.length > 0 && (
            <div className="w-full rounded border border-border-default bg-surface-elevated p-3 max-h-48 overflow-y-auto">
              <div className="flex items-center gap-1.5 mb-2">
                <Terminal size={14} className="text-text-secondary" />
                <span className="text-xs text-text-secondary font-medium">{t("ssh.connectionLog")}</span>
              </div>
              {connectLogs.map((log, i) => (
                <div key={`${log.level}-${i}-${log.message}`} className="flex items-start gap-1.5 text-xs leading-relaxed">
                  {log.level === "error" ? (
                    <AlertTriangle size={11} className="text-status-disconnected shrink-0 mt-0.5" />
                  ) : (
                    <span className={clsx("shrink-0 mt-0.5", log.level === "warn" ? "text-status-idle" : "text-text-secondary")}>·</span>
                  )}
                  <span className={clsx(
                    log.level === "error" && "text-status-disconnected",
                    log.level === "warn" && "text-status-idle",
                    log.level === "info" && "text-text-secondary"
                  )}>{log.message}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  }

  if (error) {
    const isHostKeyChanged = error.includes("Host key changed");
    const retryConnect = () => {
      setError(null);
      setLoading(true);
      setConnectLogs([]);
      authSuccessInfo.current = null;
      invoke<string>("ssh_connect", { sessionId, host, port, username, auth })
        .then((connId) => {
          createTerminal(sessionId, connId);
          setConnectionId(connId);
          setLoading(false);
          void startRecordingIfRequired();
        })
        .catch((e) => {
          setError(String(e));
          setLoading(false);
        });
    };
    const forgetAndRetry = async () => {
      try {
        await invoke("ssh_forget_host_key", { host, port });
      } catch (e) {
        // surface and abort if the forget itself fails
        setError(`Failed to forget host key: ${String(e)}`);
        return;
      }
      retryConnect();
    };
    return (
      <div className="flex items-center justify-center w-full h-full bg-surface-primary">
        <div className="flex flex-col items-center gap-3 max-w-md w-full px-6 text-center">
          <div className="w-10 h-10 rounded-full bg-status-disconnected/20 flex items-center justify-center">
            <AlertTriangle size={20} className="text-status-disconnected" />
          </div>
          <p className="text-sm text-text-primary">
            {t("ssh.connectionFailed", `Failed to connect to ${host}:${port}`)}
          </p>
          <p className="text-xs text-text-secondary break-all">{error}</p>
          {connectLogs.length > 0 && (
            <div className="w-full rounded border border-border-default bg-surface-elevated p-3 max-h-36 overflow-y-auto text-left">
              <div className="flex items-center gap-1.5 mb-2">
                <Terminal size={14} className="text-text-secondary" />
                <span className="text-xs text-text-secondary font-medium">{t("ssh.connectionLog")}</span>
              </div>
              {connectLogs.map((log, i) => (
                <div key={`${log.level}-${i}-${log.message}`} className="flex items-start gap-1.5 text-xs leading-relaxed">
                  <span className={clsx(
                    "shrink-0",
                    log.level === "error" && "text-status-disconnected",
                    log.level === "warn" && "text-status-idle",
                    log.level === "info" && "text-text-secondary"
                  )}>·</span>
                  <span className={clsx(
                    log.level === "error" && "text-status-disconnected",
                    log.level === "warn" && "text-status-idle",
                    log.level === "info" && "text-text-secondary"
                  )}>{log.message}</span>
                </div>
              ))}
            </div>
          )}
          {banner && (
            <div className="w-full rounded border border-border-default bg-surface-elevated p-3 text-left">
              <p className="text-xs text-text-secondary font-medium mb-1">{t("ssh.banner")}</p>
              <pre className="text-xs text-text-primary whitespace-pre-wrap">{banner}</pre>
            </div>
          )}
          {showCredentialPrompt ? (
            <form
              className="w-full flex flex-col gap-3 text-left"
              onSubmit={(e) => { e.preventDefault(); handleCredentialRetry(); }}
            >
              <label className="text-xs text-text-secondary">{t("ssh.enterPassword")}</label>
              <input
                type="text"
                autoFocus
                value={credUsername}
                onChange={(e) => setCredUsername(e.target.value)}
                placeholder={t("ssh.usernamePlaceholder", "Username")}
                autoComplete="username"
                className="w-full px-3 py-2 text-sm rounded bg-surface-elevated border border-border-default text-text-primary focus:border-accent-primary focus:outline-none"
              />
              <input
                type="password"
                value={credPassword}
                onChange={(e) => setCredPassword(e.target.value)}
                placeholder={t("ssh.passwordPlaceholder", "Password")}
                autoComplete="current-password"
                className="w-full px-3 py-2 text-sm rounded bg-surface-elevated border border-border-default text-text-primary focus:border-accent-primary focus:outline-none"
              />
              <button
                type="submit"
                disabled={credSubmitting || !credPassword}
                className="px-3 py-2 text-sm rounded bg-interactive-default hover:bg-interactive-hover text-text-primary disabled:opacity-50 transition-colors duration-[var(--duration-short)]"
              >
                {credSubmitting ? (
                  <Loader2 size={14} className="animate-spin mx-auto" />
                ) : (
                  t("ssh.connect")
                )}
              </button>
            </form>
          ) : (
            <div className="flex flex-wrap gap-2 justify-center">
              {isHostKeyChanged ? (
                <>
                  <button
                    onClick={forgetAndRetry}
                    className="px-3 py-1.5 text-xs rounded bg-status-disconnected/80 hover:bg-status-disconnected text-white transition-colors duration-[var(--duration-short)]"
                    title={t(
                      "ssh.forgetHostKeyHelp",
                      "Removes the saved host key for this server. Only do this if you trust the new key (e.g. the server was reinstalled or rekeyed)."
                    )}
                  >
                    {t("ssh.forgetHostKeyAndRetry", "Forget host key & retry")}
                  </button>
                  <button
                    onClick={() => setShowCredentialPrompt(true)}
                    className="px-3 py-1.5 text-xs rounded text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-short)]"
                  >
                    {t("ssh.retryWithCredentials")}
                  </button>
                </>
              ) : (
                <>
                  <button
                    onClick={() => setShowCredentialPrompt(true)}
                    className="px-3 py-1.5 text-xs rounded bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors duration-[var(--duration-short)]"
                  >
                    {t("ssh.retryWithCredentials")}
                  </button>
                  <button
                    onClick={retryConnect}
                    className="px-3 py-1.5 text-xs rounded text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-short)]"
                  >
                    {t("common.retry", "Retry")}
                  </button>
                </>
              )}
            </div>
          )}
        </div>
      </div>
    );
  }

  if (!connectionId) return null;

  const handleReconnect = async (): Promise<boolean> => {
    try {
      // Resolve vault credential for reconnection
      let reconnectAuth = auth;
      if (auth.type === "none" && credentialRef) {
        const cred = await useVaultStore.getState().getCredential(credentialRef);
        if (cred) {
          if (cred.credential_type === "password" && cred.data?.password) {
            reconnectAuth = { type: "password", password: cred.data.password as string };
          } else if (cred.credential_type === "private_key" && cred.data?.key_data) {
            reconnectAuth = {
              type: "private_key",
              key_data: cred.data.key_data as string,
              passphrase: (cred.data.passphrase as string) ?? undefined,
            };
          }
        }
      }
      const connId = await invoke<string>("ssh_connect", {
        sessionId,
        host,
        port,
        username,
        auth: reconnectAuth,
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
      void startRecordingIfRequired();
      return true;
    } catch {
      return false;
    }
  };

  return (
    <div className="relative w-full h-full">
      <ComplianceBanner
        isVisible={recordingActive && recordingNotify}
        hostname={host}
        sessionId={connectionId}
        allowUserDisable={recordingAllowDisable}
        onDisableRecording={stopRecording}
      />
      <SshTerminalView connectionId={connectionId} isActive={isActive} sessionName={`${username}@${host}`} />
      {!recordingActive && (
        <div className="absolute bottom-3 right-3 z-20 bg-surface-elevated border border-border-default rounded-lg shadow-lg px-2 py-1">
          <RecordingControls
            sessionId={sessionId}
            width={80}
            height={24}
            onRecordingIdChange={handleManualRecordingIdChange}
          />
        </div>
      )}
      {!showAssistant && (
        <button
          onClick={() => setShowAssistant(true)}
          title={t("terminal.aiAssistant")}
          className="absolute top-10 left-3 z-20 flex items-center gap-1 rounded-lg bg-surface-elevated border border-border-default shadow-lg px-2 py-1 text-xs text-text-secondary hover:text-text-primary transition-colors"
        >
          <Sparkles size={12} className="text-blue-400" />
          {t("terminal.aiAssistant")}
        </button>
      )}
      {showAssistant && (
        <div className="absolute bottom-0 inset-x-0 z-20">
          <CommandAssistant
            sessionId={sessionId}
            onInsertCommand={handleInsertCommand}
            onClose={() => setShowAssistant(false)}
          />
        </div>
      )}
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
