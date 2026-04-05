import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Settings,
  Palette,
  Terminal,
  Globe,
  Shield,
  Sun,
  Moon,
  Monitor,
  Check,
} from "lucide-react";
import { useAppStore } from "@/stores/appStore";
import { ThemeVariant } from "@/types";

// ── Types ──

interface SettingsData {
  general: {
    defaultProfile: string;
    language: string;
  };
  appearance: {
    theme: ThemeVariant;
    fontSize: number;
    fontFamily: string;
  };
  terminal: {
    scrollbackLines: number;
    cursorStyle: "block" | "underline" | "bar";
    cursorBlink: boolean;
    bellMode: "none" | "visual" | "sound" | "both";
  };
  ssh: {
    defaultPort: number;
    keepAliveInterval: number;
  };
  security: {
    vaultAutoLockMinutes: number;
    clipboardAutoClearSeconds: number;
    pasteConfirmationThreshold: number;
  };
}

const DEFAULT_SETTINGS: SettingsData = {
  general: { defaultProfile: "default", language: "en" },
  appearance: { theme: ThemeVariant.Dark, fontSize: 13, fontFamily: "Inter" },
  terminal: { scrollbackLines: 10000, cursorStyle: "block", cursorBlink: true, bellMode: "none" },
  ssh: { defaultPort: 22, keepAliveInterval: 60 },
  security: { vaultAutoLockMinutes: 15, clipboardAutoClearSeconds: 30, pasteConfirmationThreshold: 5000 },
};

type Category = "general" | "appearance" | "terminal" | "ssh" | "security";

const CATEGORIES: { id: Category; label: string; icon: React.ReactNode }[] = [
  { id: "general", label: "settings.general", icon: <Settings size={15} /> },
  { id: "appearance", label: "settings.appearance", icon: <Palette size={15} /> },
  { id: "terminal", label: "settings.terminalCategory", icon: <Terminal size={15} /> },
  { id: "ssh", label: "settings.ssh", icon: <Globe size={15} /> },
  { id: "security", label: "settings.security", icon: <Shield size={15} /> },
];

// ── Helpers ──

function SettingRow({
  label,
  description,
  children,
}: {
  readonly label: string;
  readonly description?: string;
  readonly children: React.ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-4 py-3 border-b border-border-subtle last:border-0">
      <div className="flex-1 min-w-0">
        <p className="text-xs text-text-primary">{label}</p>
        {description && <p className="text-[10px] text-text-secondary mt-0.5">{description}</p>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Toggle({
  value,
  onChange,
}: {
  readonly value: boolean;
  readonly onChange: (v: boolean) => void;
}) {
  return (
    <button
      onClick={() => onChange(!value)}
      className={clsx(
        "relative w-9 h-5 rounded-full transition-colors duration-[var(--duration-short)]",
        value ? "bg-accent-primary" : "bg-surface-secondary border border-border-default"
      )}
    >
      <span
        className={clsx(
          "absolute top-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform duration-[var(--duration-short)]",
          value ? "translate-x-4" : "translate-x-0.5"
        )}
      />
    </button>
  );
}

function SelectInput({
  value,
  options,
  onChange,
}: {
  readonly value: string;
  readonly options: { value: string; label: string }[];
  readonly onChange: (v: string) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors duration-[var(--duration-short)]"
    >
      {options.map((opt) => (
        <option key={opt.value} value={opt.value}>
          {opt.label}
        </option>
      ))}
    </select>
  );
}

function NumberInput({
  value,
  onChange,
  min,
  max,
  step,
}: {
  readonly value: number;
  readonly onChange: (v: number) => void;
  readonly min?: number;
  readonly max?: number;
  readonly step?: number;
}) {
  return (
    <input
      type="number"
      value={value}
      onChange={(e) => {
        const v = Number.parseInt(e.target.value, 10);
        if (!Number.isNaN(v)) onChange(v);
      }}
      min={min}
      max={max}
      step={step}
      className="w-20 px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus text-right transition-colors duration-[var(--duration-short)]"
    />
  );
}

// ── Main Component ──

export default function SettingsPanel() {
  const [category, setCategory] = useState<Category>("general");
  const [settings, setSettings] = useState<SettingsData>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const { t } = useTranslation();

  const appTheme = useAppStore((s) => s.theme);
  const setAppTheme = useAppStore((s) => s.setTheme);
  const profiles = useAppStore((s) => s.profiles);

  // Load settings from backend
  useEffect(() => {
    async function load() {
      try {
        const data = await invoke<SettingsData>("settings_get");
        setSettings({ ...DEFAULT_SETTINGS, ...data });
      } catch {
        // Use defaults
      } finally {
        setLoading(false);
      }
    }
    load();
  }, []);

  // Persist a setting change
  const updateSetting = useCallback(
    <K extends keyof SettingsData>(section: K, key: keyof SettingsData[K], value: SettingsData[K][keyof SettingsData[K]]) => {
      setSettings((prev) => {
        const next = {
          ...prev,
          [section]: { ...prev[section], [key]: value },
        };
        // Fire and forget save
        invoke("settings_update", { settings: next }).catch(() => {});
        return next;
      });
    },
    []
  );

  function renderCategory() {
    switch (category) {
      case "general":
        return (
          <div>
            <SettingRow label={t("settings.defaultProfile")} description={t("settings.profileOnStartup")}>
              <SelectInput
                value={settings.general.defaultProfile}
                options={profiles.map((p) => ({ value: p.id, label: p.name }))}
                onChange={(v) => updateSetting("general", "defaultProfile", v)}
              />
            </SettingRow>
            <SettingRow label={t("settings.language")}>
              <SelectInput
                value={settings.general.language}
                options={[
                  { value: "en", label: "English" },
                  { value: "zh", label: "中文" },
                  { value: "ja", label: "日本語" },
                  { value: "ko", label: "한국어" },
                  { value: "es", label: "Español" },
                  { value: "de", label: "Deutsch" },
                  { value: "fr", label: "Français" },
                ]}
                onChange={(v) => updateSetting("general", "language", v)}
              />
            </SettingRow>
          </div>
        );

      case "appearance":
        return (
          <div>
            <SettingRow label={t("settings.theme")} description={t("settings.themeDescription")}>
              <div className="flex gap-2">
                {[
                  { value: ThemeVariant.Dark, icon: <Moon size={14} />, label: "Dark" },
                  { value: ThemeVariant.Light, icon: <Sun size={14} />, label: "Light" },
                  { value: ThemeVariant.System, icon: <Monitor size={14} />, label: "System" },
                ].map((t) => (
                  <button
                    key={t.value}
                    onClick={() => {
                      setAppTheme(t.value);
                      updateSetting("appearance", "theme", t.value);
                    }}
                    className={clsx(
                      "flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs",
                      "transition-colors duration-[var(--duration-short)]",
                      appTheme === t.value
                        ? "border-border-focus bg-interactive-default/10 text-text-primary"
                        : "border-border-default hover:bg-surface-secondary text-text-secondary"
                    )}
                  >
                    {t.icon}
                    {t.label}
                    {appTheme === t.value && <Check size={12} className="text-accent-primary" />}
                  </button>
                ))}
              </div>
            </SettingRow>
            <SettingRow label={t("settings.fontSize")} description={t("settings.fontSizeDescription")}>
              <NumberInput
                value={settings.appearance.fontSize}
                onChange={(v) => updateSetting("appearance", "fontSize", v)}
                min={10}
                max={20}
              />
            </SettingRow>
            <SettingRow label={t("settings.fontFamily")}>
              <SelectInput
                value={settings.appearance.fontFamily}
                options={[
                  { value: "Inter", label: "Inter" },
                  { value: "SF Pro", label: "SF Pro" },
                  { value: "system-ui", label: "System" },
                ]}
                onChange={(v) => updateSetting("appearance", "fontFamily", v)}
              />
            </SettingRow>
          </div>
        );

      case "terminal":
        return (
          <div>
            <SettingRow label={t("settings.scrollbackLines")} description={t("settings.scrollbackDescription")}>
              <NumberInput
                value={settings.terminal.scrollbackLines}
                onChange={(v) => updateSetting("terminal", "scrollbackLines", v)}
                min={1000}
                max={100000}
                step={1000}
              />
            </SettingRow>
            <SettingRow label={t("settings.cursorStyle")}>
              <SelectInput
                value={settings.terminal.cursorStyle}
                options={[
                  { value: "block", label: "Block" },
                  { value: "underline", label: "Underline" },
                  { value: "bar", label: "Bar" },
                ]}
                onChange={(v) => updateSetting("terminal", "cursorStyle", v as "block" | "underline" | "bar")}
              />
            </SettingRow>
            <SettingRow label={t("settings.cursorBlink")}>
              <Toggle
                value={settings.terminal.cursorBlink}
                onChange={(v) => updateSetting("terminal", "cursorBlink", v)}
              />
            </SettingRow>
            <SettingRow label={t("settings.bellMode")} description={t("settings.bellDescription")}>
              <SelectInput
                value={settings.terminal.bellMode}
                options={[
                  { value: "none", label: "None" },
                  { value: "visual", label: "Visual" },
                  { value: "sound", label: "Sound" },
                  { value: "both", label: "Both" },
                ]}
                onChange={(v) => updateSetting("terminal", "bellMode", v as "none" | "visual" | "sound" | "both")}
              />
            </SettingRow>
          </div>
        );

      case "ssh":
        return (
          <div>
            <SettingRow label={t("settings.defaultPort")} description={t("settings.defaultPortDescription")}>
              <NumberInput
                value={settings.ssh.defaultPort}
                onChange={(v) => updateSetting("ssh", "defaultPort", v)}
                min={1}
                max={65535}
              />
            </SettingRow>
            <SettingRow
              label={t("settings.keepAliveInterval")}
              description={t("settings.keepAliveDescription")}
            >
              <NumberInput
                value={settings.ssh.keepAliveInterval}
                onChange={(v) => updateSetting("ssh", "keepAliveInterval", v)}
                min={0}
                max={3600}
              />
            </SettingRow>
          </div>
        );

      case "security":
        return (
          <div>
            <SettingRow
              label={t("settings.vaultAutoLock")}
              description={t("settings.vaultAutoLockDescription")}
            >
              <NumberInput
                value={settings.security.vaultAutoLockMinutes}
                onChange={(v) => updateSetting("security", "vaultAutoLockMinutes", v)}
                min={0}
                max={1440}
              />
            </SettingRow>
            <SettingRow
              label={t("settings.clipboardAutoClear")}
              description={t("settings.clipboardAutoClearDescription")}
            >
              <NumberInput
                value={settings.security.clipboardAutoClearSeconds}
                onChange={(v) => updateSetting("security", "clipboardAutoClearSeconds", v)}
                min={0}
                max={600}
              />
            </SettingRow>
            <SettingRow
              label={t("settings.pasteConfirmation")}
              description={t("settings.pasteConfirmationDescription")}
            >
              <NumberInput
                value={settings.security.pasteConfirmationThreshold}
                onChange={(v) => updateSetting("security", "pasteConfirmationThreshold", v)}
                min={0}
                max={100000}
                step={100}
              />
            </SettingRow>
          </div>
        );
    }
  }

  return (
    <div className="flex h-full bg-surface-primary">
      {/* Sidebar */}
      <nav className="w-48 border-r border-border-subtle shrink-0 py-3 px-2">
        <div className="text-[10px] uppercase tracking-wider text-text-disabled px-2 mb-2">
          {t("settings.title")}
        </div>
        {CATEGORIES.map((cat) => (
          <button
            key={cat.id}
            onClick={() => setCategory(cat.id)}
            className={clsx(
              "flex items-center gap-2.5 w-full px-2.5 py-2 rounded-lg text-xs",
              "transition-colors duration-[var(--duration-micro)]",
              category === cat.id
                ? "bg-interactive-default/15 text-text-primary"
                : "text-text-secondary hover:bg-surface-elevated hover:text-text-primary"
            )}
          >
            <span className="shrink-0">{cat.icon}</span>
            {t(cat.label)}
          </button>
        ))}
      </nav>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-6 py-4">
        <h2 className="text-sm font-semibold text-text-primary mb-1">
          {t(`settings.${category === "terminal" ? "terminalCategory" : category}`)}
        </h2>
        <p className="text-[11px] text-text-secondary mb-4">
          {t(`settings.${category}Description`)}
        </p>
        {loading ? (
          <div className="text-xs text-text-secondary py-8 text-center">{t("settings.loading")}</div>
        ) : (
          renderCategory()
        )}
      </div>
    </div>
  );
}
