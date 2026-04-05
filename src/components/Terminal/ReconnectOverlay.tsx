import { useState, useEffect, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import { WifiOff, RotateCcw } from "lucide-react";

const MAX_ATTEMPTS = 5;
const MAX_BACKOFF_MS = 60_000;

function getBackoffMs(attempt: number): number {
  return Math.min(2_000 * Math.pow(2, attempt), MAX_BACKOFF_MS);
}

interface ReconnectOverlayProps {
  readonly reason: string;
  readonly onReconnect: () => Promise<boolean>;
  readonly onClose: () => void;
}

export default function ReconnectOverlay({ reason, onReconnect, onClose }: ReconnectOverlayProps) {
  const { t } = useTranslation();
  const [attempt, setAttempt] = useState(0);
  const [countdown, setCountdown] = useState(getBackoffMs(0) / 1000);
  const [reconnecting, setReconnecting] = useState(false);
  const [exhausted, setExhausted] = useState(false);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const doReconnect = useCallback(async () => {
    clearTimer();
    setReconnecting(true);
    const success = await onReconnect();
    setReconnecting(false);
    if (!success) {
      const nextAttempt = attempt + 1;
      if (nextAttempt >= MAX_ATTEMPTS) {
        setExhausted(true);
        return;
      }
      setAttempt(nextAttempt);
      setCountdown(getBackoffMs(nextAttempt) / 1000);
    }
  }, [attempt, onReconnect, clearTimer]);

  // Countdown timer
  useEffect(() => {
    if (exhausted || reconnecting) return;

    timerRef.current = setInterval(() => {
      setCountdown((prev) => {
        if (prev <= 1) {
          clearTimer();
          doReconnect();
          return 0;
        }
        return prev - 1;
      });
    }, 1000);

    return clearTimer;
  }, [attempt, exhausted, reconnecting, clearTimer, doReconnect]);

  // Cleanup on unmount
  useEffect(() => clearTimer, [clearTimer]);

  if (exhausted) {
    return (
      <div className="absolute inset-0 z-20 flex items-center justify-center bg-surface-primary/90 backdrop-blur-sm">
        <div className="flex flex-col items-center gap-4 text-center max-w-xs">
          <div className="w-12 h-12 rounded-full bg-status-disconnected/20 flex items-center justify-center">
            <WifiOff size={24} className="text-status-disconnected" />
          </div>
          <h3 className="text-sm font-semibold text-text-primary">
            {t("reconnect.failed")}
          </h3>
          <p className="text-xs text-text-secondary">
            {t("reconnect.failedDescription")}
          </p>
          <div className="flex gap-3">
            <button
              onClick={() => {
                setExhausted(false);
                setAttempt(0);
                setCountdown(getBackoffMs(0) / 1000);
              }}
              className="px-4 py-2 text-xs rounded-lg bg-interactive-default hover:bg-interactive-hover text-text-inverse transition-colors duration-[var(--duration-short)]"
            >
              {t("reconnect.retry")}
            </button>
            <button
              onClick={onClose}
              className="px-4 py-2 text-xs rounded-lg border border-border-default text-text-secondary hover:bg-surface-secondary transition-colors duration-[var(--duration-short)]"
            >
              {t("reconnect.close")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-surface-primary/90 backdrop-blur-sm">
      <div className="flex flex-col items-center gap-4 text-center max-w-xs">
        <div className={clsx(
          "w-12 h-12 rounded-full flex items-center justify-center",
          reconnecting ? "bg-status-connecting/20" : "bg-status-disconnected/20"
        )}>
          {reconnecting ? (
            <RotateCcw size={24} className="text-status-connecting animate-spin" />
          ) : (
            <WifiOff size={24} className="text-status-disconnected" />
          )}
        </div>
        <h3 className="text-sm font-semibold text-text-primary">
          {t("reconnect.connectionLost")}
        </h3>
        {reason && (
          <p className="text-xs text-text-secondary">{reason}</p>
        )}
        {!reconnecting && (
          <p className="text-xs text-text-secondary tabular-nums">
            {t("reconnect.reconnectingIn", { seconds: countdown })}
          </p>
        )}
        <div className="flex gap-3">
          <button
            onClick={doReconnect}
            disabled={reconnecting}
            className={clsx(
              "px-4 py-2 text-xs rounded-lg transition-colors duration-[var(--duration-short)]",
              reconnecting
                ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                : "bg-interactive-default hover:bg-interactive-hover text-text-inverse"
            )}
          >
            {t("reconnect.reconnectNow")}
          </button>
          <button
            onClick={onClose}
            className="px-4 py-2 text-xs rounded-lg border border-border-default text-text-secondary hover:bg-surface-secondary transition-colors duration-[var(--duration-short)]"
          >
            {t("reconnect.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
