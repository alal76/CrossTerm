import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import {
  FileText,
  Search,
  Download,
  Trash2,
  Filter,
  CheckCircle,
  XCircle,
  AlertCircle,
  Loader2,
} from "lucide-react";
import type { AuditEntry } from "@/types";

const ACTION_FILTERS = [
  "login",
  "logout",
  "connect",
  "disconnect",
  "file_access",
  "config_change",
  "vault_access",
  "key_operation",
];

export default function AuditLog() {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [actionFilter, setActionFilter] = useState<string | null>(null);
  const [showClearConfirm, setShowClearConfirm] = useState(false);

  const fetchEntries = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<AuditEntry[]>("security_audit_list", {
        limit: 500,
      });
      setEntries(result);
    } catch {
      // Handle error silently
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchEntries();
  }, [fetchEntries]);

  const handleSearch = useCallback(async () => {
    if (!searchQuery.trim()) {
      await fetchEntries();
      return;
    }
    try {
      const result = await invoke<AuditEntry[]>("security_audit_search", {
        query: searchQuery,
      });
      setEntries(result);
    } catch {
      // Handle error silently
    }
  }, [searchQuery, fetchEntries]);

  const handleClear = useCallback(async () => {
    try {
      await invoke<number>("security_clear_audit_log");
      setEntries([]);
      setShowClearConfirm(false);
    } catch {
      // Handle error silently
    }
  }, []);

  const handleExport = useCallback(
    (format: "csv" | "json") => {
      const data =
        format === "json"
          ? JSON.stringify(filteredEntries, null, 2)
          : [
              "Timestamp,Action,Resource,Details,Success",
              ...filteredEntries.map(
                (e) =>
                  `${e.timestamp},${e.action},${e.resource},${e.details ?? ""},${e.success}`
              ),
            ].join("\n");

      const blob = new Blob([data], {
        type: format === "json" ? "application/json" : "text/csv",
      });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `audit-log.${format}`;
      a.click();
      URL.revokeObjectURL(url);
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    []
  );

  const filteredEntries = entries.filter((e) => {
    if (actionFilter && e.action !== actionFilter) return false;
    return true;
  });

  return (
    <div className="flex flex-col h-full bg-surface-primary text-text-primary">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border-default">
        <div className="flex items-center gap-2">
          <FileText size={20} className="text-accent-primary" />
          <h2 className="text-base font-semibold">{t("security.auditLog")}</h2>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => handleExport("csv")}
            className="flex items-center gap-1 px-2 py-1 text-xs rounded hover:bg-interactive-hover"
            title="Export CSV"
          >
            <Download size={14} />
            CSV
          </button>
          <button
            onClick={() => handleExport("json")}
            className="flex items-center gap-1 px-2 py-1 text-xs rounded hover:bg-interactive-hover"
            title="Export JSON"
          >
            <Download size={14} />
            JSON
          </button>
          <button
            onClick={() => setShowClearConfirm(true)}
            className="flex items-center gap-1 px-2 py-1 text-xs rounded text-status-disconnected hover:bg-status-disconnected/10"
          >
            <Trash2 size={14} />
            {t("security.clearLog")}
          </button>
        </div>
      </div>

      {/* Search & Filter */}
      <div className="flex items-center gap-2 px-4 py-2 border-b border-border-subtle">
        <div className="flex-1 flex items-center gap-2">
          <Search size={14} className="text-text-secondary" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            placeholder="Search audit entries..."
            className="flex-1 bg-transparent text-sm outline-none placeholder-text-disabled"
          />
        </div>
        <div className="flex items-center gap-1">
          <Filter size={14} className="text-text-secondary" />
          <select
            value={actionFilter ?? ""}
            onChange={(e) =>
              setActionFilter(e.target.value || null)
            }
            className="text-xs bg-surface-secondary border border-border-default rounded px-2 py-1"
          >
            <option value="">All Actions</option>
            {ACTION_FILTERS.map((a) => (
              <option key={a} value={a}>
                {a.replace("_", " ")}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Clear confirmation */}
      {showClearConfirm && (
        <div className="flex items-center justify-between px-4 py-2 bg-status-disconnected/10 text-sm">
          <span className="flex items-center gap-2 text-status-disconnected">
            <AlertCircle size={14} />
            Are you sure you want to clear the entire audit log?
          </span>
          <div className="flex gap-2">
            <button
              onClick={handleClear}
              className="px-3 py-1 text-xs rounded bg-status-disconnected text-text-inverse"
            >
              {t("actions.confirm")}
            </button>
            <button
              onClick={() => setShowClearConfirm(false)}
              className="px-3 py-1 text-xs rounded bg-surface-secondary"
            >
              {t("actions.cancel")}
            </button>
          </div>
        </div>
      )}

      {/* Table */}
      <div className="flex-1 overflow-auto">
        {(() => {
          if (loading) {
            return (
              <div className="flex items-center justify-center h-32">
                <Loader2 size={20} className="animate-spin text-text-secondary" />
              </div>
            );
          }
          if (filteredEntries.length === 0) {
            return (
              <div className="flex flex-col items-center justify-center h-32 text-text-secondary">
                <FileText size={32} className="mb-2 opacity-50" />
                <p className="text-sm">{t("audit.noEntries")}</p>
              </div>
            );
          }
          return (
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-text-secondary border-b border-border-subtle sticky top-0 bg-surface-primary">
                <th className="py-2 px-3 font-medium">{t("audit.timestamp")}</th>
                <th className="py-2 px-3 font-medium">{t("audit.action")}</th>
                <th className="py-2 px-3 font-medium">{t("audit.resource")}</th>
                <th className="py-2 px-3 font-medium">{t("audit.details")}</th>
                <th className="py-2 px-3 font-medium text-center">
                  {t("audit.success")}
                </th>
              </tr>
            </thead>
            <tbody>
              {filteredEntries.map((entry) => (
                <tr
                  key={entry.id}
                  className="border-b border-border-subtle hover:bg-surface-secondary"
                >
                  <td className="py-2 px-3 text-xs text-text-secondary whitespace-nowrap">
                    {new Date(entry.timestamp).toLocaleString()}
                  </td>
                  <td className="py-2 px-3">
                    <span className="px-1.5 py-0.5 rounded bg-surface-elevated text-xs">
                      {entry.action}
                    </span>
                  </td>
                  <td className="py-2 px-3 font-mono text-xs">{entry.resource}</td>
                  <td className="py-2 px-3 text-xs text-text-secondary truncate max-w-[200px]">
                    {entry.details ?? "—"}
                  </td>
                  <td className="py-2 px-3 text-center">
                    {entry.success ? (
                      <CheckCircle
                        size={14}
                        className="inline text-status-connected"
                      />
                    ) : (
                      <XCircle
                        size={14}
                        className="inline text-status-disconnected"
                      />
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          );
        })()}
      </div>
    </div>
  );
}
