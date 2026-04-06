import { useState, useEffect, useCallback, useMemo, createContext, useContext } from "react";
import { createPortal } from "react-dom";
import { v4 as uuidv4 } from "uuid";
import clsx from "clsx";
import { X, CheckCircle2, Info, AlertTriangle, AlertCircle } from "lucide-react";

type ToastType = "success" | "info" | "warning" | "error";

interface ToastItem {
  id: string;
  type: ToastType;
  message: string;
  duration: number;
  createdAt: number;
}

const DURATIONS: Record<ToastType, number> = {
  success: 4000,
  info: 5000,
  warning: 8000,
  error: 0, // persistent
};

const MAX_VISIBLE = 3;

const ICONS: Record<ToastType, React.ReactNode> = {
  success: <CheckCircle2 size={16} className="text-status-connected shrink-0" />,
  info: <Info size={16} className="text-accent-secondary shrink-0" />,
  warning: <AlertTriangle size={16} className="text-status-connecting shrink-0" />,
  error: <AlertCircle size={16} className="text-status-disconnected shrink-0" />,
};

const BG_CLASSES: Record<ToastType, string> = {
  success: "border-status-connected/30",
  info: "border-accent-secondary/30",
  warning: "border-status-connecting/30",
  error: "border-status-disconnected/30",
};

// ── Context ──

interface ToastContextValue {
  toast: (type: ToastType, message: string) => void;
  dismiss: (id: string) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function useToast() {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToast must be used within <ToastProvider>");
  return ctx;
}

// ── Single Toast ──

function ToastCard({
  item,
  onDismiss,
}: Readonly<{
  item: ToastItem;
  onDismiss: (id: string) => void;
}>) {
  const [progress, setProgress] = useState(100);
  const [exiting, setExiting] = useState(false);

  useEffect(() => {
    if (item.duration <= 0) return;
    const start = item.createdAt;
    let raf: number;

    function tick() {
      const elapsed = Date.now() - start;
      const remaining = Math.max(0, 1 - elapsed / item.duration) * 100;
      setProgress(remaining);
      if (remaining <= 0) {
        setExiting(true);
        setTimeout(() => onDismiss(item.id), 250);
        return;
      }
      raf = requestAnimationFrame(tick);
    }

    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [item, onDismiss]);

  return (
    <div
      className={clsx(
        "relative flex items-start gap-2 px-3 py-2.5 rounded-lg border bg-surface-elevated shadow-[var(--shadow-2)]",
        "transition-all duration-[var(--duration-medium)] ease-[var(--ease-decelerate)]",
        BG_CLASSES[item.type],
        exiting ? "opacity-0 translate-x-4" : "opacity-100 translate-x-0"
      )}
      role="alert"
    >
      {ICONS[item.type]}
      <p className="text-xs text-text-primary flex-1 leading-relaxed">{item.message}</p>
      <button
        onClick={() => {
          setExiting(true);
          setTimeout(() => onDismiss(item.id), 250);
        }}
        className="shrink-0 p-0.5 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
      >
        <X size={12} />
      </button>
      {item.duration > 0 && (
        <div className="absolute bottom-0 left-0 right-0 h-0.5 rounded-b-lg overflow-hidden">
          <div
            className={clsx(
              "h-full transition-none",
              item.type === "success" && "bg-status-connected/50",
              item.type === "info" && "bg-accent-secondary/50",
              item.type === "warning" && "bg-status-connecting/50"
            )}
            style={{ width: `${progress}%` }}
          />
        </div>
      )}
    </div>
  );
}

// ── Provider ──

export function ToastProvider({ children }: Readonly<{ children: React.ReactNode }>) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const dismiss = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const toast = useCallback((type: ToastType, message: string) => {
    const item: ToastItem = {
      id: uuidv4(),
      type,
      message,
      duration: DURATIONS[type],
      createdAt: Date.now(),
    };
    setToasts((prev) => [...prev, item].slice(-10)); // keep at most 10
  }, []);

  const visible = toasts.slice(-MAX_VISIBLE);

  return (
    <ToastContext.Provider value={useMemo(() => ({ toast, dismiss }), [toast, dismiss])}>
      {children}
      {createPortal(
        <div
          className="fixed bottom-4 right-4 z-[9999] flex flex-col gap-2 w-80 pointer-events-none"
          aria-live="polite"
          aria-relevant="additions removals"
        >
          {visible.map((item) => (
            <div
              key={item.id}
              className="pointer-events-auto animate-[slideIn_0.25s_ease-out]"
              role={item.type === "error" ? "alert" : undefined}
              aria-live={item.type === "error" ? "assertive" : undefined}
            >
              <ToastCard item={item} onDismiss={dismiss} />
            </div>
          ))}
        </div>,
        document.body
      )}
    </ToastContext.Provider>
  );
}
