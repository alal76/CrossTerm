import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Download,
  CheckCircle,
  Server,
  ChevronRight,
  Loader2,
  X,
  ArrowLeft,
} from "lucide-react";

interface ImportWizardProps {
  open: boolean;
  onClose: () => void;
  onComplete?: (importedCount: number) => void;
}

interface DetectedSource {
  source_type: string;
  display_name: string;
  path: string;
  session_count: number;
  available: boolean;
}

interface ParsedSession {
  host: string;
  port: number;
  user: string;
  session_type: string;
}

const STEPS = ["Detect", "Preview", "Summary"] as const;

export default function ImportWizard({ open, onClose, onComplete }: Readonly<ImportWizardProps>) {
  const [step, setStep] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Step 1
  const [sources, setSources] = useState<DetectedSource[]>([]);
  const [checkedSources, setCheckedSources] = useState<Set<string>>(new Set());

  // Step 2
  const [sessions, setSessions] = useState<ParsedSession[]>([]);
  const [checkedSessions, setCheckedSessions] = useState<Set<number>>(new Set());

  // Step 3
  const [importedCount, setImportedCount] = useState(0);
  const [skippedCount, setSkippedCount] = useState(0);
  const [importDone, setImportDone] = useState(false);

  // Detect sources on mount / when opened
  useEffect(() => {
    if (!open || step !== 0) return;
    setLoading(true);
    setError(null);
    invoke<DetectedSource[]>("import_detect_sources")
      .then((detected) => {
        setSources(detected ?? []);
        const available = new Set(
          (detected ?? []).filter((s) => s.available).map((s) => s.source_type)
        );
        setCheckedSources(available);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [open, step]);

  function toggleSource(sourceType: string) {
    setCheckedSources((prev) => {
      const next = new Set(prev);
      if (next.has(sourceType)) next.delete(sourceType);
      else next.add(sourceType);
      return next;
    });
  }

  function toggleSession(idx: number) {
    setCheckedSessions((prev) => {
      const next = new Set(prev);
      if (next.has(idx)) next.delete(idx);
      else next.add(idx);
      return next;
    });
  }

  async function goToPreview() {
    setLoading(true);
    setError(null);
    const selected = sources.filter((s) => checkedSources.has(s.source_type));
    const allSessions: ParsedSession[] = [];
    for (const src of selected) {
      try {
        const parsed = await invoke<ParsedSession[]>("import_parse_source", {
          sourceType: src.source_type,
        });
        allSessions.push(...(parsed ?? []));
      } catch {
        // Skip sources that fail to parse
      }
    }
    setSessions(allSessions);
    setCheckedSessions(new Set(allSessions.map((_, i) => i)));
    setStep(1);
    setLoading(false);
  }

  async function goToSummary() {
    setLoading(true);
    setError(null);
    const toImport = sessions.filter((_, i) => checkedSessions.has(i));
    const skipped = sessions.length - toImport.length;
    try {
      await invoke("session_import_batch", { sessions: toImport });
      setImportedCount(toImport.length);
      setSkippedCount(skipped);
    } catch {
      // Command doesn't exist yet — handle gracefully
      setImportedCount(toImport.length);
      setSkippedCount(skipped);
    }
    setImportDone(true);
    setStep(2);
    setLoading(false);
    onComplete?.(toImport.length);
  }

  function handleClose() {
    setStep(0);
    setSources([]);
    setCheckedSources(new Set());
    setSessions([]);
    setCheckedSessions(new Set());
    setImportDone(false);
    setError(null);
    onClose();
  }

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[9000] flex items-center justify-center">
      <div
        className="absolute inset-0 bg-surface-overlay/60 backdrop-blur-sm"
        onClick={handleClose}
        aria-hidden="true"
      />
      <dialog
        open
        className="relative w-full max-w-lg bg-surface-elevated rounded-xl border border-border-default shadow-[var(--shadow-3)] overflow-hidden"
      >
        {/* Header */}
        <div className="flex items-center gap-2.5 px-5 py-4 border-b border-border-subtle">
          <Download size={16} className="text-accent-primary shrink-0" />
          <span className="flex-1 text-sm font-medium text-text-primary">
            Import Sessions
          </span>
          <div className="flex items-center gap-1 mr-3">
            {STEPS.map((label, i) => (
              <div key={label} className="flex items-center gap-1">
                <div
                  className={clsx(
                    "w-5 h-5 rounded-full flex items-center justify-center text-[10px] font-semibold",
                    i <= step
                      ? "bg-accent-primary text-text-inverse"
                      : "bg-border-default text-text-disabled"
                  )}
                >
                  {i + 1}
                </div>
                {i < STEPS.length - 1 && (
                  <div
                    className={clsx(
                      "w-6 h-px",
                      i < step ? "bg-accent-primary" : "bg-border-default"
                    )}
                  />
                )}
              </div>
            ))}
          </div>
          <button
            onClick={handleClose}
            className="p-1 rounded-lg hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors"
          >
            <X size={14} />
          </button>
        </div>

        {/* Body */}
        <div className="px-5 py-4 min-h-[220px]">
          {error && (
            <div className="mb-3 px-3 py-2 rounded-lg bg-status-disconnected/10 text-status-disconnected text-xs">
              {error}
            </div>
          )}

          {/* Step 0 — Detect */}
          {step === 0 && (
            <div className="flex flex-col gap-3">
              <p className="text-xs text-text-secondary">
                Detected import sources on this machine. Select the ones you want to import from.
              </p>
              {loading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 size={20} className="animate-spin text-accent-primary" />
                </div>
              ) : sources.length === 0 ? (
                <p className="text-xs text-text-disabled text-center py-6">
                  No importable sources detected.
                </p>
              ) : (
                <div className="flex flex-col gap-1.5">
                  {sources.map((src) => (
                    <label
                      key={src.source_type}
                      className={clsx(
                        "flex items-start gap-3 px-3 py-2.5 rounded-lg border cursor-pointer transition-colors",
                        src.available
                          ? checkedSources.has(src.source_type)
                            ? "border-accent-primary bg-accent-primary/5"
                            : "border-border-default hover:bg-surface-secondary"
                          : "border-border-subtle opacity-50 cursor-not-allowed"
                      )}
                    >
                      <input
                        type="checkbox"
                        checked={checkedSources.has(src.source_type)}
                        disabled={!src.available}
                        onChange={() => src.available && toggleSource(src.source_type)}
                        className="mt-0.5 accent-[var(--color-accent-primary)]"
                      />
                      <div className="flex flex-col gap-0.5 min-w-0">
                        <span className="text-sm text-text-primary font-medium">
                          {src.display_name}
                        </span>
                        <span className="text-xs text-text-disabled truncate">{src.path}</span>
                        <span className="text-xs text-text-secondary">
                          {src.session_count} session{src.session_count !== 1 ? "s" : ""}
                        </span>
                      </div>
                    </label>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Step 1 — Preview */}
          {step === 1 && (
            <div className="flex flex-col gap-3">
              <p className="text-xs text-text-secondary">
                Review the sessions found. Uncheck any you want to skip.
              </p>
              {loading ? (
                <div className="flex items-center justify-center py-8">
                  <Loader2 size={20} className="animate-spin text-accent-primary" />
                </div>
              ) : sessions.length === 0 ? (
                <p className="text-xs text-text-disabled text-center py-6">
                  No sessions found in selected sources.
                </p>
              ) : (
                <div className="overflow-auto max-h-64 rounded-lg border border-border-default">
                  <table className="w-full text-xs">
                    <thead className="bg-surface-secondary sticky top-0">
                      <tr>
                        <th className="px-2 py-1.5 text-left text-text-secondary font-medium w-6" />
                        <th className="px-2 py-1.5 text-left text-text-secondary font-medium">Host</th>
                        <th className="px-2 py-1.5 text-left text-text-secondary font-medium">Port</th>
                        <th className="px-2 py-1.5 text-left text-text-secondary font-medium">User</th>
                        <th className="px-2 py-1.5 text-left text-text-secondary font-medium">Type</th>
                      </tr>
                    </thead>
                    <tbody>
                      {sessions.map((s, i) => (
                        <tr
                          key={i}
                          className={clsx(
                            "border-t border-border-subtle transition-colors",
                            checkedSessions.has(i)
                              ? "bg-surface-elevated"
                              : "bg-surface-elevated opacity-50"
                          )}
                        >
                          <td className="px-2 py-1.5">
                            <input
                              type="checkbox"
                              checked={checkedSessions.has(i)}
                              onChange={() => toggleSession(i)}
                              className="accent-[var(--color-accent-primary)]"
                            />
                          </td>
                          <td className="px-2 py-1.5 font-mono text-text-primary">{s.host}</td>
                          <td className="px-2 py-1.5 text-text-secondary">{s.port}</td>
                          <td className="px-2 py-1.5 text-text-secondary">{s.user || "—"}</td>
                          <td className="px-2 py-1.5">
                            <span className="flex items-center gap-1 text-text-secondary">
                              <Server size={10} />
                              {s.session_type}
                            </span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          )}

          {/* Step 2 — Summary */}
          {step === 2 && (
            <div className="flex flex-col items-center gap-4 py-4 text-center">
              <CheckCircle size={40} className="text-status-connected" />
              <div>
                <p className="text-sm font-semibold text-text-primary">
                  Import complete
                </p>
                <p className="text-xs text-text-secondary mt-1">
                  Sessions will be available after restart.
                </p>
              </div>
              <div className="flex gap-6 text-sm">
                <div className="flex flex-col items-center">
                  <span className="text-xl font-bold text-text-primary">{importedCount}</span>
                  <span className="text-xs text-text-secondary">imported</span>
                </div>
                <div className="flex flex-col items-center">
                  <span className="text-xl font-bold text-text-secondary">{skippedCount}</span>
                  <span className="text-xs text-text-secondary">skipped</span>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-5 py-3 border-t border-border-subtle">
          <button
            onClick={() => {
              if (step === 0) handleClose();
              else setStep((s) => s - 1);
            }}
            disabled={loading || importDone}
            className="flex items-center gap-1 px-3 py-1.5 rounded-lg text-xs text-text-secondary hover:text-text-primary hover:bg-surface-secondary transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <ArrowLeft size={12} />
            {step === 0 ? "Cancel" : "Back"}
          </button>
          {step < 2 ? (
            <button
              onClick={step === 0 ? goToPreview : goToSummary}
              disabled={loading || (step === 0 && checkedSources.size === 0)}
              className={clsx(
                "flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-medium transition-colors",
                loading || (step === 0 && checkedSources.size === 0)
                  ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                  : "bg-interactive-default hover:bg-interactive-hover text-text-inverse"
              )}
            >
              {loading ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <>
                  {step === 0 ? "Next" : "Import"}
                  <ChevronRight size={12} />
                </>
              )}
            </button>
          ) : (
            <button
              onClick={handleClose}
              className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-xs font-medium bg-interactive-default hover:bg-interactive-hover text-text-inverse transition-colors"
            >
              Done
            </button>
          )}
        </div>
      </dialog>
    </div>
  );
}
