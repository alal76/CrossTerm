import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  ArrowDown,
  ArrowUp,
  Check,
  ChevronRight,
  FolderSync,
  Loader2,
  SkipForward,
  TriangleAlert,
} from "lucide-react";
import type { SyncEntry, SyncResult } from "@/types";

interface FolderSyncWizardProps {
  readonly sessionId: string;
  readonly onClose: () => void;
}

type WizardStep = "select" | "compare" | "review" | "execute";

export default function FolderSyncWizard({
  sessionId,
  onClose,
}: FolderSyncWizardProps) {
  const { t } = useTranslation();
  const [step, setStep] = useState<WizardStep>("select");
  const [localDir, setLocalDir] = useState("");
  const [remoteDir, setRemoteDir] = useState("");
  const [entries, setEntries] = useState<SyncEntry[]>([]);
  const [result, setResult] = useState<SyncResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleCompare = useCallback(async () => {
    if (!localDir || !remoteDir) return;
    setLoading(true);
    setError(null);
    try {
      const compared = await invoke<SyncEntry[]>("sftp_sync_compare", {
        sessionId,
        localDir,
        remoteDir,
      });
      setEntries(compared);
      setStep("review");
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [sessionId, localDir, remoteDir]);

  const handleToggleAction = useCallback(
    (index: number) => {
      setEntries((prev) => {
        const next = [...prev];
        const entry = { ...next[index] };
        const actions: SyncEntry["sync_action"][] = [
          "upload",
          "download",
          "skip",
        ];
        const current = actions.indexOf(entry.sync_action);
        entry.sync_action = actions[(current + 1) % actions.length];
        next[index] = entry;
        return next;
      });
    },
    []
  );

  const handleExecute = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const syncResult = await invoke<SyncResult>("sftp_sync_execute", {
        sessionId,
        entries,
        localDir,
        remoteDir,
      });
      setResult(syncResult);
      setStep("execute");
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, [sessionId, entries, localDir, remoteDir]);

  const actionIcon = (action: SyncEntry["sync_action"]) => {
    switch (action) {
      case "upload":
        return <ArrowUp size={12} className="text-status-connected" />;
      case "download":
        return <ArrowDown size={12} className="text-accent-primary" />;
      case "skip":
        return <SkipForward size={12} className="text-text-disabled" />;
      case "conflict":
        return <TriangleAlert size={12} className="text-status-connecting" />;
    }
  };

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <FolderSync size={16} className="text-accent-primary" />
          <h2 className="text-sm font-medium text-text-primary">
            {t("sftp.syncWizard")}
          </h2>
        </div>
        <button
          onClick={onClose}
          className="text-xs text-text-secondary hover:text-text-primary"
        >
          ✕
        </button>
      </div>

      {/* Step indicator */}
      <div className="flex items-center gap-1 text-[10px] text-text-disabled">
        {(["select", "compare", "review", "execute"] as const).map(
          (s, i) => (
            <span key={s} className="flex items-center gap-1">
              {i > 0 && <ChevronRight size={10} />}
              <span
                className={clsx(
                  step === s
                    ? "font-medium text-accent-primary"
                    : "text-text-disabled"
                )}
              >
                {s.charAt(0).toUpperCase() + s.slice(1)}
              </span>
            </span>
          )
        )}
      </div>

      {error && (
        <div className="rounded border border-status-disconnected/30 bg-status-disconnected/10 px-3 py-2 text-xs text-status-disconnected">
          {error}
        </div>
      )}

      {/* Step: Select directories */}
      {step === "select" && (
        <div className="flex flex-col gap-3">
          <div>
            <label className="mb-1 block text-xs text-text-secondary">
              {t("sftp.selectLocalDir")}
            </label>
            <input
              type="text"
              value={localDir}
              onChange={(e) => setLocalDir(e.target.value)}
              placeholder="/home/user/project"
              className="w-full rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
            />
          </div>
          <div>
            <label className="mb-1 block text-xs text-text-secondary">
              {t("sftp.selectRemoteDir")}
            </label>
            <input
              type="text"
              value={remoteDir}
              onChange={(e) => setRemoteDir(e.target.value)}
              placeholder="/var/www/project"
              className="w-full rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
            />
          </div>
          <button
            onClick={handleCompare}
            disabled={!localDir || !remoteDir || loading}
            className={clsx(
              "flex items-center justify-center gap-2 rounded px-3 py-1.5 text-xs font-medium",
              "bg-accent-primary text-text-inverse hover:bg-interactive-hover",
              "disabled:cursor-not-allowed disabled:opacity-50"
            )}
          >
            {loading ? (
              <Loader2 size={12} className="animate-spin" />
            ) : null}
            {t("sftp.compare")}
          </button>
        </div>
      )}

      {/* Step: Review sync actions */}
      {step === "review" && (
        <div className="flex flex-col gap-3">
          {entries.length === 0 ? (
            <p className="py-8 text-center text-xs text-text-disabled">
              {t("sftp.noChanges")}
            </p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b border-border-subtle text-left text-text-secondary">
                    <th className="px-2 py-1.5">File</th>
                    <th className="px-2 py-1.5">Size</th>
                    <th className="px-2 py-1.5">Action</th>
                  </tr>
                </thead>
                <tbody>
                  {entries.map((entry, index) => (
                    <tr
                      key={entry.path}
                      className="border-b border-border-subtle hover:bg-surface-secondary"
                    >
                      <td className="px-2 py-1.5 font-mono">{entry.path}</td>
                      <td className="px-2 py-1.5">{formatSize(entry.size)}</td>
                      <td className="px-2 py-1.5">
                        <button
                          onClick={() => handleToggleAction(index)}
                          className="flex items-center gap-1 rounded px-1.5 py-0.5 hover:bg-interactive-hover"
                        >
                          {actionIcon(entry.sync_action)}
                          <span>{t(`sftp.${entry.sync_action}`)}</span>
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          <div className="flex items-center gap-2">
            <button
              onClick={() => setStep("select")}
              className="rounded border border-border-default px-3 py-1.5 text-xs text-text-secondary hover:bg-surface-secondary"
            >
              Back
            </button>
            <button
              onClick={handleExecute}
              disabled={loading || entries.length === 0}
              className={clsx(
                "flex items-center gap-2 rounded px-3 py-1.5 text-xs font-medium",
                "bg-accent-primary text-text-inverse hover:bg-interactive-hover",
                "disabled:cursor-not-allowed disabled:opacity-50"
              )}
            >
              {loading ? (
                <Loader2 size={12} className="animate-spin" />
              ) : null}
              {t("sftp.syncExecute")}
            </button>
          </div>
        </div>
      )}

      {/* Step: Execute results */}
      {step === "execute" && result && (
        <div className="flex flex-col items-center gap-3 py-6">
          <Check size={32} className="text-status-connected" />
          <h3 className="text-sm font-medium text-text-primary">
            {t("sftp.syncComplete")}
          </h3>
          <div className="flex gap-4 text-xs text-text-secondary">
            <span>
              ↑ {result.uploaded} {t("sftp.upload")}
            </span>
            <span>
              ↓ {result.downloaded} {t("sftp.download")}
            </span>
            <span>
              ⏭ {result.skipped} {t("sftp.skip")}
            </span>
          </div>
          {result.errors.length > 0 && (
            <div className="mt-2 w-full rounded border border-status-disconnected/30 bg-status-disconnected/10 p-2 text-xs text-status-disconnected">
              {result.errors.map((err, i) => (
                <p key={`err-${i}-${err.slice(0, 20)}`}>{err}</p>
              ))}
            </div>
          )}
          <button
            onClick={onClose}
            className="mt-2 rounded border border-border-default px-3 py-1.5 text-xs text-text-secondary hover:bg-surface-secondary"
          >
            Close
          </button>
        </div>
      )}
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
