import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { save, open } from "@tauri-apps/plugin-dialog";
import { readFile, writeFile } from "@tauri-apps/plugin-fs";
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  Clock,
  Loader2,
  RefreshCw,
} from "lucide-react";
import type { SyncStatus } from "@/types";

export default function ProfileSync() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<SyncStatus>({});
  const [loading, setLoading] = useState(false);

  const loadStatus = useCallback(async () => {
    try {
      const result = await invoke<SyncStatus>("sync_get_status");
      if (result) setStatus(result);
    } catch {
      // Status fetch failed
    }
  }, []);

  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  const handleExport = useCallback(async () => {
    setLoading(true);
    try {
      const data = await invoke<number[]>("sync_export");
      const filePath = await save({
        defaultPath: "crossterm-settings.ctbundle",
        filters: [{ name: "CrossTerm Bundle", extensions: ["ctbundle"] }],
      });
      if (filePath) {
        await writeFile(filePath, new Uint8Array(data));
        loadStatus();
      }
    } catch {
      // Export failed
    } finally {
      setLoading(false);
    }
  }, [loadStatus]);

  const handleImport = useCallback(async () => {
    setLoading(true);
    try {
      const filePath = await open({
        filters: [{ name: "CrossTerm Bundle", extensions: ["ctbundle"] }],
        multiple: false,
      });
      if (filePath) {
        const data = await readFile(filePath);
        await invoke("sync_import", { data: Array.from(data) });
        loadStatus();
      }
    } catch {
      // Import failed
    } finally {
      setLoading(false);
    }
  }, [loadStatus]);

  return (
    <div className="flex flex-col gap-4 rounded-lg border border-border-default bg-surface-secondary p-4">
      <div className="flex items-center gap-2">
        <RefreshCw size={16} className="text-accent-primary" />
        <h3 className="text-sm font-semibold text-text-primary">
          {t("sync.title")}
        </h3>
      </div>

      <p className="text-xs text-text-secondary">{t("sync.description")}</p>

      <div className="flex items-center gap-3">
        <button
          onClick={handleExport}
          disabled={loading}
          className="flex items-center gap-1.5 rounded bg-accent-primary px-3 py-1.5 text-xs font-medium text-text-inverse hover:bg-interactive-hover disabled:opacity-50"
        >
          {loading ? (
            <Loader2 size={14} className="animate-spin" />
          ) : (
            <ArrowUpFromLine size={14} />
          )}
          {t("sync.export")}
        </button>
        <button
          onClick={handleImport}
          disabled={loading}
          className="flex items-center gap-1.5 rounded border border-border-default px-3 py-1.5 text-xs font-medium text-text-primary hover:bg-surface-secondary disabled:opacity-50"
        >
          {loading ? (
            <Loader2 size={14} className="animate-spin" />
          ) : (
            <ArrowDownToLine size={14} />
          )}
          {t("sync.import")}
        </button>
      </div>

      <div className="flex items-center gap-4 text-xs text-text-secondary">
        <span className="flex items-center gap-1">
          <Clock size={12} />
          {t("sync.lastExport")}: {status.last_export ?? t("sync.never")}
        </span>
        <span className="flex items-center gap-1">
          <Clock size={12} />
          {t("sync.lastImport")}: {status.last_import ?? t("sync.never")}
        </span>
      </div>
    </div>
  );
}
