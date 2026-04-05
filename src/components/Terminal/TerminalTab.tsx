import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { useTerminalStore } from "@/stores/terminalStore";
import { Loader2 } from "lucide-react";
import TerminalView from "./TerminalView";

interface TerminalTabProps {
  readonly sessionId: string;
  readonly isActive: boolean;
  readonly shell?: string;
}

export default function TerminalTab({ sessionId, isActive, shell }: TerminalTabProps) {
  const [terminalId, setTerminalId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const createTerminal = useTerminalStore((s) => s.createTerminal);
  const removeTerminal = useTerminalStore((s) => s.removeTerminal);
  const { t } = useTranslation();

  useEffect(() => {
    let cancelled = false;

    async function init() {
      try {
        setLoading(true);
        setError(null);
        const info = await invoke<{ id: string }>("terminal_create", { shell: shell ?? null });
        if (cancelled) {
          invoke("terminal_close", { id: info.id }).catch(() => {});
          return;
        }
        createTerminal(sessionId, info.id);
        setTerminalId(info.id);
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

    init();

    return () => {
      cancelled = true;
      if (terminalId) {
        invoke("terminal_close", { id: terminalId }).catch(() => {});
        removeTerminal(terminalId);
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, shell]);

  if (loading) {
    return (
      <div className="flex items-center justify-center w-full h-full bg-surface-primary">
        <div className="flex flex-col items-center gap-3 text-text-secondary">
          <Loader2 size={24} className="animate-spin text-accent-primary" />
          <span className="text-sm">{t("terminal.starting")}</span>
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
          <p className="text-sm text-text-primary">{t("terminal.failedCreate")}</p>
          <p className="text-xs text-text-secondary">{error}</p>
          <button
            onClick={() => {
              setError(null);
              setLoading(true);
              invoke<{ id: string }>("terminal_create", { shell: shell ?? null })
                .then((info) => {
                  createTerminal(sessionId, info.id);
                  setTerminalId(info.id);
                  setLoading(false);
                })
                .catch((e) => {
                  setError(String(e));
                  setLoading(false);
                });
            }}
            className="px-3 py-1.5 text-xs rounded bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            {t("terminal.retry")}
          </button>
        </div>
      </div>
    );
  }

  if (!terminalId) return null;

  return <TerminalView terminalId={terminalId} isActive={isActive} />;
}
