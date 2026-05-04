import { useEffect, useState, useCallback } from "react";
import { WifiOff, AlertTriangle, Zap } from "lucide-react";

// ── Types ────────────────────────────────────────────────────────────────────

interface ReconnectOverlayProps {
  sessionId: string;
  status: "ok" | "degraded" | "dropped";
  latencyMs: number | null;
  attempt: number;
  maxAttempts?: number; // default 5
  onReconnect: () => void;
  onDismiss: () => void;
  onGiveUp: () => void;
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/** Exponential backoff capped at 30s: 5, 10, 20, 30, 30, … */
function backoffSeconds(attempt: number): number {
  return Math.min(5 * Math.pow(2, attempt - 1), 30);
}

// ── Degraded Banner ───────────────────────────────────────────────────────────

function DegradedBanner({ latencyMs }: { latencyMs: number | null }) {
  return (
    <div
      className="absolute top-0 left-0 right-0 z-10 flex items-center gap-2 px-3 py-1.5 bg-yellow-500/20 border-b border-yellow-500/40"
      role="status"
    >
      <Zap size={13} className="text-yellow-400 shrink-0" />
      <span className="text-xs text-yellow-300 font-medium">
        High latency
        {latencyMs !== null ? ` (${latencyMs}ms)` : ""}
        {" "}— connection may be unstable
      </span>
    </div>
  );
}

// ── Countdown Progress Bar ────────────────────────────────────────────────────

function CountdownBar({
  countdown,
  total,
}: {
  countdown: number;
  total: number;
}) {
  const pct = total > 0 ? (countdown / total) * 100 : 0;

  return (
    <div className="w-full h-1.5 rounded-full bg-white/10 overflow-hidden">
      <div
        className="h-full rounded-full bg-red-500/70 transition-[width] duration-1000 ease-linear animate-pulse"
        style={{ width: `${pct}%` }}
      />
    </div>
  );
}

// ── Main Component ────────────────────────────────────────────────────────────

export default function ReconnectOverlay({
  sessionId: _sessionId,
  status,
  latencyMs,
  attempt,
  maxAttempts = 5,
  onReconnect,
  onDismiss,
  onGiveUp,
}: ReconnectOverlayProps) {
  const totalSeconds = backoffSeconds(attempt);
  const [countdown, setCountdown] = useState(totalSeconds);
  const gaveUp = attempt >= maxAttempts;

  // Reset countdown when attempt or status changes
  useEffect(() => {
    if (status !== "dropped") return;
    setCountdown(backoffSeconds(attempt));
  }, [attempt, status]);

  // Tick down and auto-reconnect when countdown hits 0
  useEffect(() => {
    if (status !== "dropped" || gaveUp) return;

    const interval = setInterval(() => {
      setCountdown((prev) => {
        if (prev <= 1) {
          clearInterval(interval);
          onReconnect();
          return 0;
        }
        return prev - 1;
      });
    }, 1000);

    return () => clearInterval(interval);
  }, [status, gaveUp, onReconnect, attempt]);

  const handleReconnectNow = useCallback(() => {
    setCountdown(0);
    onReconnect();
  }, [onReconnect]);

  if (status === "ok") return null;

  if (status === "degraded") {
    return <DegradedBanner latencyMs={latencyMs} />;
  }

  // status === "dropped"
  return (
    <div
      className="absolute inset-0 z-20 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      role="alertdialog"
      aria-modal="true"
      aria-label="Connection lost"
    >
      <div className="flex flex-col items-center gap-4 rounded-2xl border border-white/10 bg-surface-elevated p-8 shadow-[var(--shadow-3)] w-72">
        {/* Icon */}
        <div className="flex h-12 w-12 items-center justify-center rounded-full bg-red-500/15">
          <WifiOff size={24} className="text-red-400" />
        </div>

        {/* Title */}
        <h2 className="text-base font-semibold text-text-primary">
          Connection lost
        </h2>

        {/* Subtitle / state-specific copy */}
        {gaveUp ? (
          <p className="text-center text-xs text-text-secondary leading-relaxed">
            Reconnect failed after{" "}
            <span className="font-medium text-text-primary">{maxAttempts}</span>{" "}
            attempts
          </p>
        ) : (
          <>
            <p className="text-center text-xs text-text-secondary leading-relaxed">
              Reconnecting in{" "}
              <span className="font-medium text-text-primary tabular-nums">
                {countdown}s
              </span>
              …{" "}
              <span className="text-text-disabled">
                (attempt {attempt}/{maxAttempts})
              </span>
            </p>

            {/* Progress bar */}
            <CountdownBar countdown={countdown} total={totalSeconds} />
          </>
        )}

        {/* Latency note when available */}
        {latencyMs !== null && (
          <div className="flex items-center gap-1.5 text-xs text-text-secondary">
            <AlertTriangle size={12} className="text-yellow-400 shrink-0" />
            <span>Last seen at {latencyMs}ms latency</span>
          </div>
        )}

        {/* Action buttons */}
        <div className="flex flex-col gap-2 w-full">
          {gaveUp ? (
            <>
              <button
                onClick={handleReconnectNow}
                className="w-full rounded-lg bg-accent-primary px-4 py-2 text-xs font-medium text-text-inverse hover:bg-interactive-hover transition-colors duration-[var(--duration-micro)]"
              >
                Try again
              </button>
              <button
                onClick={onGiveUp}
                className="w-full rounded-lg border border-red-500/40 px-4 py-2 text-xs font-medium text-red-400 hover:bg-red-500/10 transition-colors duration-[var(--duration-micro)]"
              >
                Give up
              </button>
            </>
          ) : (
            <>
              <button
                onClick={handleReconnectNow}
                className="w-full rounded-lg bg-accent-primary px-4 py-2 text-xs font-medium text-text-inverse hover:bg-interactive-hover transition-colors duration-[var(--duration-micro)]"
              >
                Reconnect now
              </button>
              <div className="flex gap-2">
                <button
                  onClick={onDismiss}
                  className="flex-1 rounded-lg border border-white/10 px-4 py-2 text-xs font-medium text-text-secondary hover:bg-surface-secondary transition-colors duration-[var(--duration-micro)]"
                >
                  Dismiss
                </button>
                {attempt >= maxAttempts - 1 && (
                  <button
                    onClick={onGiveUp}
                    className="flex-1 rounded-lg border border-red-500/40 px-4 py-2 text-xs font-medium text-red-400 hover:bg-red-500/10 transition-colors duration-[var(--duration-micro)]"
                  >
                    Give up
                  </button>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
