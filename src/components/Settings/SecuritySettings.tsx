import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Shield,
  Clock,
  ClipboardX,
  FileText,
  ShieldAlert,
  Lock,
  Save,
  Loader2,
} from "lucide-react";
import type { SecurityConfig, CertFingerprint } from "@/types";

export default function SecuritySettings() {
  const { t } = useTranslation();
  const [config, setConfig] = useState<SecurityConfig | null>(null);
  const [certPins, setCertPins] = useState<[string, CertFingerprint][]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);

  const fetchConfig = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<SecurityConfig>("security_get_config");
      setConfig(result);
      const pins = await invoke<[string, CertFingerprint][]>(
        "security_cert_list_pins"
      );
      setCertPins(pins);
    } catch {
      // Handle error silently
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchConfig();
  }, [fetchConfig]);

  const handleSave = useCallback(async () => {
    if (!config) return;
    setSaving(true);
    try {
      await invoke("security_set_config", { config });
    } catch {
      // Handle error silently
    } finally {
      setSaving(false);
    }
  }, [config]);

  if (loading || !config) {
    return (
      <div className="flex items-center justify-center h-32">
        <Loader2 size={20} className="animate-spin text-text-secondary" />
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-2">
        <Shield size={20} className="text-accent-primary" />
        <h2 className="text-base font-semibold">{t("security.settings")}</h2>
      </div>

      {/* Vault Timeout */}
      <div className="space-y-2">
        <label className="flex items-center gap-2 text-sm font-medium">
          <Clock size={14} />
          {t("security.vaultTimeout")}
        </label>
        <select
          value={config.vault_timeout_secs}
          onChange={(e) =>
            setConfig({ ...config, vault_timeout_secs: Number(e.target.value) })
          }
          className="w-full px-3 py-2 text-sm rounded border border-border-default bg-surface-secondary"
        >
          <option value={60}>1 minute</option>
          <option value={300}>5 minutes</option>
          <option value={600}>10 minutes</option>
          <option value={1800}>30 minutes</option>
          <option value={3600}>1 hour</option>
          <option value={0}>Never</option>
        </select>
      </div>

      {/* Clipboard Auto-Clear */}
      <div className="space-y-2">
        <label className="flex items-center gap-2 text-sm font-medium">
          <ClipboardX size={14} />
          {t("security.clipboardClear")}
        </label>
        <select
          value={config.clipboard_clear_secs}
          onChange={(e) =>
            setConfig({
              ...config,
              clipboard_clear_secs: Number(e.target.value),
            })
          }
          className="w-full px-3 py-2 text-sm rounded border border-border-default bg-surface-secondary"
        >
          <option value={10}>10 seconds</option>
          <option value={30}>30 seconds</option>
          <option value={60}>1 minute</option>
          <option value={0}>Never</option>
        </select>
      </div>

      {/* Audit Log Toggle */}
      <div className="flex items-center justify-between">
        <label className="flex items-center gap-2 text-sm font-medium">
          <FileText size={14} />
          {t("security.auditLog")}
        </label>
        <button
          onClick={() =>
            setConfig({ ...config, audit_enabled: !config.audit_enabled })
          }
          className={clsx(
            "w-10 h-5 rounded-full relative transition-colors",
            config.audit_enabled ? "bg-accent-primary" : "bg-surface-sunken"
          )}
        >
          <span
            className={clsx(
              "absolute top-0.5 w-4 h-4 rounded-full bg-text-inverse transition-transform",
              config.audit_enabled ? "left-5" : "left-0.5"
            )}
          />
        </button>
      </div>

      {/* Rate Limiting */}
      <div className="space-y-2">
        <label className="flex items-center gap-2 text-sm font-medium">
          <ShieldAlert size={14} />
          {t("security.rateLimit")}
        </label>
        <div className="grid grid-cols-3 gap-2">
          <div>
            <label htmlFor="security-max-attempts" className="text-xs text-text-secondary">Max Attempts</label>
            <input
              id="security-max-attempts"
              type="number"
              min={1}
              value={config.rate_limit.max_attempts}
              onChange={(e) =>
                setConfig({
                  ...config,
                  rate_limit: {
                    ...config.rate_limit,
                    max_attempts: Number(e.target.value),
                  },
                })
              }
              className="w-full px-2 py-1 text-sm rounded border border-border-default bg-surface-secondary"
            />
          </div>
          <div>
            <label htmlFor="security-window" className="text-xs text-text-secondary">Window (s)</label>
            <input
              id="security-window"
              type="number"
              min={1}
              value={config.rate_limit.window_secs}
              onChange={(e) =>
                setConfig({
                  ...config,
                  rate_limit: {
                    ...config.rate_limit,
                    window_secs: Number(e.target.value),
                  },
                })
              }
              className="w-full px-2 py-1 text-sm rounded border border-border-default bg-surface-secondary"
            />
          </div>
          <div>
            <label htmlFor="security-lockout" className="text-xs text-text-secondary">Lockout (s)</label>
            <input
              id="security-lockout"
              type="number"
              min={1}
              value={config.rate_limit.lockout_secs}
              onChange={(e) =>
                setConfig({
                  ...config,
                  rate_limit: {
                    ...config.rate_limit,
                    lockout_secs: Number(e.target.value),
                  },
                })
              }
              className="w-full px-2 py-1 text-sm rounded border border-border-default bg-surface-secondary"
            />
          </div>
        </div>
      </div>

      {/* Certificate Pins */}
      <div className="space-y-2">
        <label className="flex items-center gap-2 text-sm font-medium">
          <Lock size={14} />
          {t("security.certPins")}
        </label>
        {certPins.length === 0 ? (
          <p className="text-xs text-text-secondary">
            No certificate pins configured.
          </p>
        ) : (
          <div className="space-y-1">
            {certPins.map(([host, pin]) => (
              <div
                key={host}
                className="flex items-center justify-between p-2 rounded bg-surface-secondary text-xs"
              >
                <span className="font-mono">{host}</span>
                <span className="text-text-secondary truncate max-w-[200px]">
                  {pin.sha256}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Save Button */}
      <button
        onClick={handleSave}
        disabled={saving}
        className="flex items-center gap-2 px-4 py-2 text-sm rounded bg-accent-primary text-text-inverse hover:opacity-90 disabled:opacity-50"
      >
        {saving ? <Loader2 size={14} className="animate-spin" /> : <Save size={14} />}
        {t("actions.save")}
      </button>
    </div>
  );
}
