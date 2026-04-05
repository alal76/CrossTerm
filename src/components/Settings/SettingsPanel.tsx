import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { readTextFile } from "@tauri-apps/plugin-fs";
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
  Upload,
} from "lucide-react";
import { useAppStore } from "@/stores/appStore";
import { ThemeVariant } from "@/types";
import type { BellStyle, CursorStyle, ThemeFile, ThemeTokens } from "@/types";

import darkTheme from "@/themes/dark.json";
import lightTheme from "@/themes/light.json";
import solarizedDarkTheme from "@/themes/solarized-dark.json";
import solarizedLightTheme from "@/themes/solarized-light.json";
import draculaTheme from "@/themes/dracula.json";
import nordTheme from "@/themes/nord.json";
import monokaiProTheme from "@/themes/monokai-pro.json";
import highContrastTheme from "@/themes/high-contrast.json";

const BUILT_IN_THEMES: ThemeFile[] = [
  darkTheme as ThemeFile,
  lightTheme as ThemeFile,
  solarizedDarkTheme as ThemeFile,
  solarizedLightTheme as ThemeFile,
  draculaTheme as ThemeFile,
  nordTheme as ThemeFile,
  monokaiProTheme as ThemeFile,
  highContrastTheme as ThemeFile,
];

type Category = "general" | "appearance" | "terminal" | "ssh" | "security";

interface BackendSettings {
  theme: string;
  font_size: number;
  font_family: string;
  font_ligatures: boolean;
  cursor_style: string;
  cursor_blink: boolean;
  scrollback_lines: number;
  line_height: number;
  letter_spacing: number;
  tab_title_format: string;
  default_shell: string | null;
  copy_on_select: boolean;
  paste_warning_lines: number;
  idle_lock_timeout_secs: number;
  auto_update: boolean;
  gpu_acceleration: boolean;
  bell_style: string;
  terminal_opacity: number;
}

const DEFAULT_SETTINGS: BackendSettings = {
  theme: ThemeVariant.Dark,
  font_size: 14,
  font_family: "JetBrains Mono",
  font_ligatures: true,
  cursor_style: "block",
  cursor_blink: true,
  scrollback_lines: 10000,
  line_height: 1.2,
  letter_spacing: 0,
  tab_title_format: "{name} - {host}",
  default_shell: null,
  copy_on_select: false,
  paste_warning_lines: 5,
  idle_lock_timeout_secs: 900,
  auto_update: true,
  gpu_acceleration: true,
  bell_style: "visual",
  terminal_opacity: 1,
};

const CATEGORIES: { id: Category; label: string; icon: React.ReactNode }[] = [
  { id: "general", label: "settings.general", icon: <Settings size={15} /> },
  { id: "appearance", label: "settings.appearance", icon: <Palette size={15} /> },
  { id: "terminal", label: "settings.terminalCategory", icon: <Terminal size={15} /> },
  { id: "ssh", label: "settings.ssh", icon: <Globe size={15} /> },
  { id: "security", label: "settings.security", icon: <Shield size={15} /> },
];

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
        {description ? <p className="text-[10px] text-text-secondary mt-0.5">{description}</p> : null}
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
  readonly onChange: (value: boolean) => void;
}) {
  return (
    <button
      onClick={() => onChange(!value)}
      className={clsx(
        "relative w-9 h-5 rounded-full transition-colors duration-[var(--duration-short)]",
        value ? "bg-accent-primary" : "bg-surface-secondary border border-border-default",
      )}
    >
      <span
        className={clsx(
          "absolute top-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform duration-[var(--duration-short)]",
          value ? "translate-x-4" : "translate-x-0.5",
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
  readonly options: Array<{ value: string; label: string }>;
  readonly onChange: (value: string) => void;
}) {
  return (
    <select
      value={value}
      onChange={(event) => onChange(event.target.value)}
      className="px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors duration-[var(--duration-short)]"
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
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
  readonly onChange: (value: number) => void;
  readonly min?: number;
  readonly max?: number;
  readonly step?: number;
}) {
  return (
    <input
      type="number"
      value={value}
      onChange={(event) => {
        const nextValue = Number.parseFloat(event.target.value);
        if (!Number.isNaN(nextValue)) {
          onChange(nextValue);
        }
      }}
      min={min}
      max={max}
      step={step}
      className="w-24 px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus text-right transition-colors duration-[var(--duration-short)]"
    />
  );
}

export default function SettingsPanel() {
  const { t } = useTranslation();
  const [category, setCategory] = useState<Category>("general");
  const [settings, setSettings] = useState<BackendSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);

  const appTheme = useAppStore((state) => state.theme);
  const setAppTheme = useAppStore((state) => state.setTheme);
  const profiles = useAppStore((state) => state.profiles);
  const customThemeName = useAppStore((state) => state.customThemeName);
  const setCustomTheme = useAppStore((state) => state.setCustomTheme);
  const bellStyle = useAppStore((state) => state.bellStyle);
  const setBellStyle = useAppStore((state) => state.setBellStyle);
  const cursorStyle = useAppStore((state) => state.cursorStyle);
  const setCursorStyle = useAppStore((state) => state.setCursorStyle);
  const cursorBlink = useAppStore((state) => state.cursorBlink);
  const setCursorBlink = useAppStore((state) => state.setCursorBlink);

  useEffect(() => {
    let mounted = true;

    async function loadSettings() {
      try {
        const loaded = await invoke<BackendSettings>("settings_get");
        if (!mounted) {
          return;
        }
        const next = { ...DEFAULT_SETTINGS, ...loaded };
        setSettings(next);
        setAppTheme((next.theme as ThemeVariant) ?? ThemeVariant.Dark);
        setBellStyle((next.bell_style as BellStyle) ?? "visual");
        setCursorStyle((next.cursor_style as CursorStyle) ?? "block");
        setCursorBlink(next.cursor_blink);
      } catch {
        if (mounted) {
          setSettings(DEFAULT_SETTINGS);
        }
      } finally {
        if (mounted) {
          setLoading(false);
        }
      }
    }

    loadSettings();
    return () => {
      mounted = false;
    };
  }, [setAppTheme, setBellStyle, setCursorBlink, setCursorStyle]);

  const persistSettings = useCallback(async (next: BackendSettings) => {
    setSettings(next);
    try {
      await invoke("settings_update", { settings: next });
    } catch {
      // Keep optimistic local state.
    }
  }, []);

  const updateSettings = useCallback(
    <K extends keyof BackendSettings>(key: K, value: BackendSettings[K]) => {
      const next = { ...settings, [key]: value };
      void persistSettings(next);
    },
    [persistSettings, settings],
  );

  const applyThemeTokens = useCallback((tokens: Partial<ThemeTokens>) => {
    const root = document.documentElement;
    for (const [key, value] of Object.entries(tokens)) {
      root.style.setProperty(`--${key}`, value);
    }
  }, []);

  const handleSelectBuiltInTheme = useCallback(
    (themeFile: ThemeFile) => {
      applyThemeTokens(themeFile.tokens);
      setCustomTheme(themeFile.name, themeFile.tokens);
      setAppTheme(themeFile.variant);
      updateSettings("theme", themeFile.variant);
    },
    [applyThemeTokens, setAppTheme, setCustomTheme, updateSettings],
  );

  const handleImportTheme = useCallback(async () => {
    try {
      const selected = await open({
        title: t("themeImport.selectFile"),
        filters: [{ name: "JSON", extensions: ["json"] }],
        multiple: false,
      });
      if (!selected || Array.isArray(selected)) {
        return;
      }
      const content = await readTextFile(selected);
      const parsed = JSON.parse(content) as ThemeFile;
      applyThemeTokens(parsed.tokens);
      setCustomTheme(parsed.name ?? t("themeImport.customTheme"), parsed.tokens);
      setAppTheme(parsed.variant ?? ThemeVariant.Dark);
    } catch {
      // Ignore import failures.
    }
  }, [applyThemeTokens, setAppTheme, setCustomTheme, t]);

  const renderCategory = () => {
    switch (category) {
      case "general":
        return (
          <div>
            <SettingRow label={t("settings.defaultProfile")} description={t("settings.generalDescription")}>
              <SelectInput
                value={profiles[0]?.id ?? "default"}
                options={profiles.map((profile) => ({ value: profile.id, label: profile.name }))}
                onChange={() => {}}
              />
            </SettingRow>
            <SettingRow label={t("settings.autoUpdate")} description={t("settings.securityDescription")}>
              <Toggle value={settings.auto_update} onChange={(value) => updateSettings("auto_update", value)} />
            </SettingRow>
            <SettingRow label={t("settings.gpuAcceleration")} description={t("settings.terminalDescription")}>
              <Toggle value={settings.gpu_acceleration} onChange={(value) => updateSettings("gpu_acceleration", value)} />
            </SettingRow>
          </div>
        );
      case "appearance":
        return (
          <div>
            <SettingRow label={t("settings.theme")} description={t("settings.themeDescription")}>
              <div className="flex flex-wrap gap-2 justify-end">
                {[
                  { value: ThemeVariant.Dark, label: t("theme.dark"), icon: <Moon size={12} /> },
                  { value: ThemeVariant.Light, label: t("theme.light"), icon: <Sun size={12} /> },
                  { value: ThemeVariant.System, label: t("theme.system"), icon: <Monitor size={12} /> },
                ].map((themeOption) => (
                  <button
                    key={themeOption.value}
                    onClick={() => {
                      setAppTheme(themeOption.value);
                      setCustomTheme(null, null);
                      updateSettings("theme", themeOption.value);
                    }}
                    className={clsx(
                      "flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs transition-colors duration-[var(--duration-short)]",
                      appTheme === themeOption.value && !customThemeName
                        ? "border-border-focus bg-interactive-default/10 text-text-primary"
                        : "border-border-default hover:bg-surface-secondary text-text-secondary",
                    )}
                  >
                    {themeOption.icon}
                    {themeOption.label}
                    {appTheme === themeOption.value && !customThemeName ? (
                      <Check size={12} className="text-accent-primary" />
                    ) : null}
                  </button>
                ))}
              </div>
            </SettingRow>
            <SettingRow label={t("themeImport.customTheme")} description={t("settings.themeDescription")}>
              <div className="flex flex-col gap-2">
                <div className="grid grid-cols-2 gap-1.5">
                  {BUILT_IN_THEMES.map((themeFile) => (
                    <button
                      key={themeFile.name}
                      onClick={() => handleSelectBuiltInTheme(themeFile)}
                      className={clsx(
                        "px-2.5 py-1.5 rounded-lg border text-xs text-left transition-colors duration-[var(--duration-short)]",
                        customThemeName === themeFile.name
                          ? "border-border-focus bg-interactive-default/10 text-text-primary"
                          : "border-border-default hover:bg-surface-secondary text-text-secondary",
                      )}
                    >
                      {themeFile.name}
                      {customThemeName === themeFile.name ? (
                        <Check size={10} className="inline ml-1 text-accent-primary" />
                      ) : null}
                    </button>
                  ))}
                </div>
                <button
                  onClick={() => {
                    void handleImportTheme();
                  }}
                  className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary transition-colors"
                >
                  <Upload size={13} />
                  {t("themeImport.importTheme")}
                </button>
              </div>
            </SettingRow>
            <SettingRow label={t("settings.fontSize")} description={t("settings.fontSizeDescription")}>
              <NumberInput value={settings.font_size} onChange={(value) => updateSettings("font_size", value)} min={10} max={24} />
            </SettingRow>
            <SettingRow label={t("settings.fontFamily")}>
              <SelectInput
                value={settings.font_family}
                options={[
                  { value: "JetBrains Mono", label: "JetBrains Mono" },
                  { value: "SF Mono", label: "SF Mono" },
                  { value: "Menlo", label: "Menlo" },
                  { value: "monospace", label: "Monospace" },
                ]}
                onChange={(value) => updateSettings("font_family", value)}
              />
            </SettingRow>
          </div>
        );
      case "terminal":
        return (
          <div>
            <SettingRow label={t("settings.scrollbackLines")} description={t("settings.scrollbackDescription")}>
              <NumberInput
                value={settings.scrollback_lines}
                onChange={(value) => updateSettings("scrollback_lines", value)}
                min={1000}
                max={100000}
                step={1000}
              />
            </SettingRow>
            <SettingRow label={t("settings.cursorStyle")}>
              <SelectInput
                value={cursorStyle}
                options={[
                  { value: "block", label: t("cursorStyles.block") },
                  { value: "underline", label: t("cursorStyles.underline") },
                  { value: "bar", label: t("cursorStyles.bar") },
                ]}
                onChange={(value) => {
                  setCursorStyle(value as CursorStyle);
                  updateSettings("cursor_style", value);
                }}
              />
            </SettingRow>
            <SettingRow label={t("settings.cursorBlink")}>
              <Toggle
                value={cursorBlink}
                onChange={(value) => {
                  setCursorBlink(value);
                  updateSettings("cursor_blink", value);
                }}
              />
            </SettingRow>
            <SettingRow label={t("settings.bellMode")} description={t("settings.bellDescription")}>
              <SelectInput
                value={bellStyle}
                options={[
                  { value: "none", label: t("bellStyles.none") },
                  { value: "visual", label: t("bellStyles.visual") },
                  { value: "audio", label: t("bellStyles.audio") },
                ]}
                onChange={(value) => {
                  setBellStyle(value as BellStyle);
                  updateSettings("bell_style", value);
                }}
              />
            </SettingRow>
            <SettingRow label={t("settings.opacity") || "Opacity"}>
              <NumberInput value={settings.terminal_opacity} onChange={(value) => updateSettings("terminal_opacity", value)} min={0.2} max={1} step={0.1} />
            </SettingRow>
          </div>
        );
      case "ssh":
        return (
          <div>
            <SettingRow label={t("settings.defaultShell")} description={t("settings.sshDescription")}>
              <input
                value={settings.default_shell ?? ""}
                onChange={(event) => updateSettings("default_shell", event.target.value || null)}
                className="w-56 px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors duration-[var(--duration-short)]"
              />
            </SettingRow>
            <SettingRow label={t("settings.tabTitleFormat") || "Tab title format"}>
              <input
                value={settings.tab_title_format}
                onChange={(event) => updateSettings("tab_title_format", event.target.value)}
                className="w-56 px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors duration-[var(--duration-short)]"
              />
            </SettingRow>
          </div>
        );
      case "security":
        return (
          <div>
            <SettingRow label={t("settings.vaultAutoLock")} description={t("settings.vaultAutoLockDescription")}>
              <NumberInput
                value={Math.round(settings.idle_lock_timeout_secs / 60)}
                onChange={(value) => updateSettings("idle_lock_timeout_secs", value * 60)}
                min={0}
                max={1440}
              />
            </SettingRow>
            <SettingRow label={t("settings.copyOnSelect") || "Copy on select"}>
              <Toggle value={settings.copy_on_select} onChange={(value) => updateSettings("copy_on_select", value)} />
            </SettingRow>
            <SettingRow label={t("settings.pasteConfirmation")} description={t("settings.pasteConfirmationDescription")}>
              <NumberInput
                value={settings.paste_warning_lines}
                onChange={(value) => updateSettings("paste_warning_lines", value)}
                min={0}
                max={1000}
              />
            </SettingRow>
          </div>
        );
    }
  };

  return (
    <div className="flex h-full bg-surface-primary">
      <nav className="w-48 border-r border-border-subtle shrink-0 py-3 px-2">
        <div className="text-[10px] uppercase tracking-wider text-text-disabled px-2 mb-2">
          {t("settings.title")}
        </div>
        {CATEGORIES.map((entry) => (
          <button
            key={entry.id}
            onClick={() => setCategory(entry.id)}
            className={clsx(
              "flex items-center gap-2.5 w-full px-2.5 py-2 rounded-lg text-xs transition-colors duration-[var(--duration-micro)]",
              category === entry.id
                ? "bg-interactive-default/15 text-text-primary"
                : "text-text-secondary hover:bg-surface-elevated hover:text-text-primary",
            )}
          >
            <span className="shrink-0">{entry.icon}</span>
            {t(entry.label)}
          </button>
        ))}
      </nav>

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
