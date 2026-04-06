import { useState, useEffect, useCallback, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import { X, Trash2, Info, AlertTriangle, AlertCircle, Search } from "lucide-react";

interface NotificationEntry {
  id: string;
  timestamp: string;
  severity: string;
  message: string;
  session_id: string | null;
  category: string;
  dismissed: boolean;
}

interface NotificationHistoryPanelProps {
  readonly onClose: () => void;
}

function getSeverityIcon(severity: string) {
  switch (severity) {
    case "error":
      return <AlertCircle size={14} className="text-status-disconnected shrink-0" />;
    case "warn":
    case "warning":
      return <AlertTriangle size={14} className="text-status-connecting shrink-0" />;
    default:
      return <Info size={14} className="text-accent-primary shrink-0" />;
  }
}

function formatTime(timestamp: string): string {
  try {
    return new Date(timestamp).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  } catch {
    return timestamp;
  }
}

function getDateGroup(timestamp: string, today: string, yesterday: string, older: string): string {
  try {
    const date = new Date(timestamp);
    const now = new Date();
    const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterdayStart = new Date(todayStart.getTime() - 86400000);

    if (date >= todayStart) return today;
    if (date >= yesterdayStart) return yesterday;
    return older;
  } catch {
    return older;
  }
}

export default function NotificationHistoryPanel({ onClose }: NotificationHistoryPanelProps) {
  const { t } = useTranslation();
  const [notifications, setNotifications] = useState<NotificationEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState("");

  const fetchNotifications = useCallback(async () => {
    try {
      const result = await invoke<NotificationEntry[]>("notification_list");
      setNotifications(result);
    } catch {
      // Backend may not be available
    }
  }, []);

  useEffect(() => {
    fetchNotifications();
  }, [fetchNotifications]);

  const handleDismiss = useCallback(
    async (id: string) => {
      try {
        await invoke("notification_dismiss", { id });
        setNotifications((prev) => prev.map((n) => (n.id === id ? { ...n, dismissed: true } : n)));
      } catch {
        // ignore
      }
    },
    [],
  );

  const handleClearAll = useCallback(async () => {
    try {
      await invoke("notification_clear_all");
      setNotifications([]);
    } catch {
      // ignore
    }
  }, []);

  const filtered = useMemo(() => {
    if (!searchQuery.trim()) return notifications;
    const q = searchQuery.toLowerCase();
    return notifications.filter((n) => n.message.toLowerCase().includes(q));
  }, [notifications, searchQuery]);

  const grouped = useMemo(() => {
    const todayLabel = t("notifications.today");
    const yesterdayLabel = t("notifications.yesterday");
    const olderLabel = t("notifications.older");
    const groups = new Map<string, NotificationEntry[]>();

    for (const entry of filtered) {
      const group = getDateGroup(entry.timestamp, todayLabel, yesterdayLabel, olderLabel);
      const list = groups.get(group) ?? [];
      list.push(entry);
      groups.set(group, list);
    }

    return groups;
  }, [filtered, t]);

  return (
    <div className="fixed inset-0 z-[9000] flex justify-end">
      {/* Backdrop */}
      <button
        type="button"
        className="absolute inset-0 bg-black/30 cursor-default border-none"
        onClick={onClose}
        aria-label="Close notifications"
        tabIndex={-1}
      />

      {/* Panel */}
      <aside
        className="relative w-[380px] max-w-full h-full bg-surface-primary border-l border-border-default shadow-[var(--shadow-3)] flex flex-col animate-slide-right"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-subtle shrink-0">
          <h2 className="text-sm font-semibold text-text-primary">{t("notifications.title")}</h2>
          <div className="flex items-center gap-1">
            {notifications.length > 0 && (
              <button
                onClick={handleClearAll}
                className="flex items-center gap-1 px-2 py-1 text-xs rounded text-status-disconnected hover:bg-status-disconnected/10 transition-colors"
              >
                <Trash2 size={12} />
                {t("notifications.clearAll")}
              </button>
            )}
            <button
              onClick={onClose}
              className="p-1.5 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
            >
              <X size={16} />
            </button>
          </div>
        </div>

        {/* Search */}
        <div className="px-4 py-2 border-b border-border-subtle shrink-0">
          <div className="flex items-center gap-2 px-2 py-1.5 rounded bg-surface-secondary border border-border-subtle">
            <Search size={13} className="text-text-disabled shrink-0" />
            <input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t("notifications.search")}
              className="flex-1 bg-transparent text-xs text-text-primary placeholder:text-text-disabled outline-none"
            />
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto">
          {filtered.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-3 py-12 text-center">
              <Info size={28} className="text-text-disabled" />
              <p className="text-xs text-text-secondary">{t("notifications.empty")}</p>
            </div>
          ) : (
            <div className="py-2">
              {Array.from(grouped.entries()).map(([group, entries]) => (
                <div key={group}>
                  <div className="px-4 py-1.5">
                    <span className="text-[10px] font-semibold uppercase tracking-wider text-text-disabled">
                      {group}
                    </span>
                  </div>
                  {entries.map((entry) => (
                    <div
                      key={entry.id}
                      className={clsx(
                        "flex items-start gap-2.5 px-4 py-2.5 hover:bg-surface-secondary transition-colors",
                        entry.dismissed && "opacity-50",
                      )}
                    >
                      <div className="mt-0.5">{getSeverityIcon(entry.severity)}</div>
                      <div className="flex-1 min-w-0">
                        <p className="text-xs text-text-primary leading-snug">{entry.message}</p>
                        <span className="text-[10px] text-text-disabled mt-0.5 block">
                          {formatTime(entry.timestamp)}
                        </span>
                      </div>
                      {!entry.dismissed && (
                        <button
                          onClick={() => handleDismiss(entry.id)}
                          className="shrink-0 p-1 rounded text-text-disabled hover:text-text-secondary hover:bg-surface-elevated transition-colors"
                          title={t("notifications.dismiss")}
                        >
                          <X size={12} />
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              ))}
            </div>
          )}
        </div>
      </aside>
    </div>
  );
}
