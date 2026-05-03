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
  Link2,
  FolderSync,
  Keyboard,
  Bell,
  Cpu,
  RefreshCw,
} from "lucide-react";
import { useAppStore } from "@/stores/appStore";
import { ThemeVariant } from "@/types";
import type { BellStyle, CursorStyle, ThemeFile, ThemeTokens } from "@/types";
import FieldHelp from "@/components/Help/FieldHelp";
import SecuritySettings from "@/components/Settings/SecuritySettings";
import ProfileSync from "@/components/Settings/ProfileSync";

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

type Category =
  | "general"
  | "appearance"
  | "terminal"
  | "ssh"
  | "connections"
  | "file_transfer"
  | "keyboard"
  | "notifications"
  | "security"
  | "advanced";

interface BackendSettings {
  // Appearance
  theme: string;
  font_size: number;
  font_family: string;
  font_ligatures: boolean;
  line_height: number;
  letter_spacing: number;
  // Terminal
  cursor_style: string;
  cursor_blink: boolean;
  scrollback_lines: number;
  terminal_opacity: number;
  bell_style: string;
  terminal_encoding: string;
  scroll_on_output: boolean;
  scroll_on_keystroke: boolean;
  scrollbar_visible: boolean;
  word_separators: string;
  // SSH
  tab_title_format: string;
  default_shell: string | null;
  ssh_port: number;
  ssh_keepalive_interval: number;
  ssh_strict_host_check: boolean;
  ssh_compression: boolean;
  ssh_agent_forwarding: boolean;
  ssh_x11_forwarding: boolean;
  // Connections
  connection_timeout_secs: number;
  reconnect_on_disconnect: boolean;
  reconnect_delay_secs: number;
  reconnect_max_attempts: number;
  max_concurrent_connections: number;
  // File Transfer
  sftp_default_remote_dir: string;
  sftp_encoding: string;
  transfer_confirm_overwrite: boolean;
  transfer_preserve_timestamps: boolean;
  transfer_concurrent_jobs: number;
  // General / UI
  auto_update: boolean;
  gpu_acceleration: boolean;
  startup_behavior: string;
  confirm_close_tab: boolean;
  show_status_bar: boolean;
  window_always_on_top: boolean;
  // Keyboard
  backspace_sends_delete: boolean;
  alt_is_meta: boolean;
  ctrl_h_is_backspace: boolean;
  home_end_scroll_buffer: boolean;
  right_click_action: string;
  // Notifications
  notify_on_disconnect: boolean;
  notify_on_bell: boolean;
  notify_on_long_process: boolean;
  long_process_threshold_secs: number;
  flash_tab_on_bell: boolean;
  // Security
  copy_on_select: boolean;
  paste_warning_lines: number;
  idle_lock_timeout_secs: number;
  clipboard_history_size: number;
  // Advanced
  log_level: string;
  telemetry_enabled: boolean;
}

const DEFAULT_SETTINGS: BackendSettings = {
  theme: ThemeVariant.Dark,
  font_size: 14,
  font_family: "JetBrains Mono",
  font_ligatures: true,
  line_height: 1.2,
  letter_spacing: 0,
  cursor_style: "block",
  cursor_blink: true,
  scrollback_lines: 10000,
  terminal_opacity: 1,
  bell_style: "visual",
  terminal_encoding: "utf-8",
  scroll_on_output: true,
  scroll_on_keystroke: true,
  scrollbar_visible: true,
  word_separators: " ,;:\"'()[]{}",
  tab_title_format: "{name} - {host}",
  default_shell: null,
  ssh_port: 22,
  ssh_keepalive_interval: 60,
  ssh_strict_host_check: true,
  ssh_compression: false,
  ssh_agent_forwarding: false,
  ssh_x11_forwarding: false,
  connection_timeout_secs: 30,
  reconnect_on_disconnect: false,
  reconnect_delay_secs: 5,
  reconnect_max_attempts: 3,
  max_concurrent_connections: 20,
  sftp_default_remote_dir: "~",
  sftp_encoding: "utf-8",
  transfer_confirm_overwrite: true,
  transfer_preserve_timestamps: true,
  transfer_concurrent_jobs: 4,
  auto_update: true,
  gpu_acceleration: true,
  startup_behavior: "restore",
  confirm_close_tab: false,
  show_status_bar: true,
  window_always_on_top: false,
  backspace_sends_delete: false,
  alt_is_meta: true,
  ctrl_h_is_backspace: false,
  home_end_scroll_buffer: true,
  right_click_action: "context_menu",
  notify_on_disconnect: true,
  notify_on_bell: false,
  notify_on_long_process: true,
  long_process_threshold_secs: 10,
  flash_tab_on_bell: true,
  copy_on_select: false,
  paste_warning_lines: 5,
  idle_lock_timeout_secs: 900,
  clipboard_history_size: 50,
  log_level: "info",
  telemetry_enabled: false,
};

// ── Category definitions ────────────────────────────────────────────────────

const CATEGORIES: { id: Category; labelKey: string; icon: React.ReactNode }[] = [
  { id: "general",       labelKey: "settings.general",           icon: <Settings size={15} /> },
  { id: "appearance",    labelKey: "settings.appearance",        icon: <Palette size={15} /> },
  { id: "terminal",      labelKey: "settings.terminalCategory",  icon: <Terminal size={15} /> },
  { id: "ssh",           labelKey: "settings.ssh",               icon: <Globe size={15} /> },
  { id: "connections",   labelKey: "settings.connections",       icon: <Link2 size={15} /> },
  { id: "file_transfer", labelKey: "settings.fileTransfer",      icon: <FolderSync size={15} /> },
  { id: "keyboard",      labelKey: "settings.keyboard",          icon: <Keyboard size={15} /> },
  { id: "notifications", labelKey: "settings.notifications",     icon: <Bell size={15} /> },
  { id: "security",      labelKey: "settings.security",         icon: <Shield size={15} /> },
  { id: "advanced",      labelKey: "settings.advancedCategory",  icon: <Cpu size={15} /> },
];

const DESCRIPTION_KEYS: Record<Category, string> = {
  general:       "settings.generalDescription",
  appearance:    "settings.appearanceDescription",
  terminal:      "settings.terminalDescription",
  ssh:           "settings.sshDescription",
  connections:   "settings.connectionsDescription",
  file_transfer: "settings.fileTransferDescription",
  keyboard:      "settings.keyboardDescription",
  notifications: "settings.notificationsDescription",
  security:      "settings.securityDescription",
  advanced:      "settings.advancedDescription",
};

const HEADING_KEYS: Record<Category, string> = {
  general:       "settings.general",
  appearance:    "settings.appearance",
  terminal:      "settings.terminalCategory",
  ssh:           "settings.ssh",
  connections:   "settings.connections",
  file_transfer: "settings.fileTransfer",
  keyboard:      "settings.keyboard",
  notifications: "settings.notifications",
  security:      "settings.security",
  advanced:      "settings.advancedCategory",
};

// ── Shared UI components ────────────────────────────────────────────────────

function SettingRow({
  label,
  description,
  help,
  children,
}: {
  readonly label: string;
  readonly description?: string;
  readonly help?: React.ReactNode;
  readonly children: React.ReactNode;
}) {
  return (
    <div className="flex items-start justify-between gap-4 py-3 border-b border-border-subtle last:border-0">
      <div className="flex-1 min-w-0">
        <p className="flex items-center text-xs text-text-primary">
          {label}
          {help}
        </p>
        {description ? <p className="text-[10px] text-text-secondary mt-0.5">{description}</p> : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function SectionHeading({ children }: { readonly children: React.ReactNode }) {
  return (
    <p className="text-[10px] font-semibold uppercase tracking-wider text-text-disabled mt-5 mb-1 first:mt-0">
      {children}
    </p>
  );
}

function Toggle({ value, onChange }: { readonly value: boolean; readonly onChange: (v: boolean) => void }) {
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
  readonly onChange: (v: string) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors"
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>{o.label}</option>
      ))}
    </select>
  );
}

function NumberInput({
  value, onChange, min, max, step,
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
      onChange={(e) => { const n = Number.parseFloat(e.target.value); if (!Number.isNaN(n)) onChange(n); }}
      min={min} max={max} step={step}
      className="w-24 px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus text-right transition-colors"
    />
  );
}

function TextInput({
  value, onChange, placeholder, wide,
}: {
  readonly value: string;
  readonly onChange: (v: string) => void;
  readonly placeholder?: string;
  readonly wide?: boolean;
}) {
  return (
    <input
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      className={clsx(
        "px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors",
        wide ? "w-64" : "w-44",
      )}
    />
  );
}

// ── Main component ──────────────────────────────────────────────────────────

export default function SettingsPanel() {
  const { t } = useTranslation();
  const [category, setCategory] = useState<Category>("general");
  const [settings, setSettings] = useState<BackendSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);

  const appTheme      = useAppStore((s) => s.theme);
  const setAppTheme   = useAppStore((s) => s.setTheme);
  const profiles      = useAppStore((s) => s.profiles);
  const customThemeName = useAppStore((s) => s.customThemeName);
  const setCustomTheme  = useAppStore((s) => s.setCustomTheme);
  const bellStyle     = useAppStore((s) => s.bellStyle);
  const setBellStyle  = useAppStore((s) => s.setBellStyle);
  const cursorStyle   = useAppStore((s) => s.cursorStyle);
  const setCursorStyle = useAppStore((s) => s.setCursorStyle);
  const cursorBlink   = useAppStore((s) => s.cursorBlink);
  const setCursorBlink = useAppStore((s) => s.setCursorBlink);

  useEffect(() => {
    let mounted = true;
    async function load() {
      try {
        const loaded = await invoke<BackendSettings>("settings_get");
        if (!mounted) return;
        const next = { ...DEFAULT_SETTINGS, ...loaded };
        setSettings(next);
        setAppTheme((next.theme as ThemeVariant) ?? ThemeVariant.Dark);
        setBellStyle((next.bell_style as BellStyle) ?? "visual");
        setCursorStyle((next.cursor_style as CursorStyle) ?? "block");
        setCursorBlink(next.cursor_blink);
      } catch {
        if (mounted) setSettings(DEFAULT_SETTINGS);
      } finally {
        if (mounted) setLoading(false);
      }
    }
    load();
    return () => { mounted = false; };
  }, [setAppTheme, setBellStyle, setCursorBlink, setCursorStyle]);

  const persist = useCallback(async (next: BackendSettings) => {
    setSettings(next);
    try { await invoke("settings_update", { settings: next }); } catch { /* keep optimistic */ }
  }, []);

  const update = useCallback(
    <K extends keyof BackendSettings>(key: K, value: BackendSettings[K]) => {
      void persist({ ...settings, [key]: value });
    },
    [persist, settings],
  );

  const applyThemeTokens = useCallback((tokens: Partial<ThemeTokens>) => {
    const root = document.documentElement;
    for (const [k, v] of Object.entries(tokens)) {
      if (typeof v === "string") root.style.setProperty(`--${k}`, v);
    }
  }, []);

  const handleSelectBuiltInTheme = useCallback((tf: ThemeFile) => {
    applyThemeTokens(tf.tokens);
    setCustomTheme(tf.name, tf.tokens);
    setAppTheme(tf.variant);
    update("theme", tf.variant);
  }, [applyThemeTokens, setAppTheme, setCustomTheme, update]);

  const handleImportTheme = useCallback(async () => {
    try {
      const selected = await open({ title: t("themeImport.selectFile"), filters: [{ name: "JSON", extensions: ["json"] }], multiple: false });
      if (!selected || Array.isArray(selected)) return;
      const content = await readTextFile(selected);
      const parsed = JSON.parse(content) as ThemeFile;
      applyThemeTokens(parsed.tokens);
      setCustomTheme(parsed.name ?? t("themeImport.customTheme"), parsed.tokens);
      setAppTheme(parsed.variant ?? ThemeVariant.Dark);
    } catch { /* ignore */ }
  }, [applyThemeTokens, setAppTheme, setCustomTheme, t]);

  // ── Tab renderers ─────────────────────────────────────────────────────────

  function renderGeneral() {
    return (
      <div>
        <SectionHeading>Application</SectionHeading>
        <SettingRow label={t("settings.defaultProfile")} description={t("settings.profileOnStartup")}>
          <SelectInput
            value={profiles[0]?.id ?? "default"}
            options={profiles.map((p) => ({ value: p.id, label: p.name }))}
            onChange={() => {}}
          />
        </SettingRow>
        <SettingRow label={t("settings.startupBehavior")} description={t("settings.startupBehaviorDescription")}>
          <SelectInput
            value={settings.startup_behavior}
            options={[
              { value: "restore",   label: "Restore last session" },
              { value: "new_tab",   label: "Open new tab" },
              { value: "dashboard", label: "Show session browser" },
            ]}
            onChange={(v) => update("startup_behavior", v)}
          />
        </SettingRow>
        <SettingRow label={t("settings.confirmCloseTab")} description={t("settings.confirmCloseTabDescription")}>
          <Toggle value={settings.confirm_close_tab} onChange={(v) => update("confirm_close_tab", v)} />
        </SettingRow>
        <SettingRow label={t("settings.showStatusBar")} description={t("settings.showStatusBarDescription")}>
          <Toggle value={settings.show_status_bar} onChange={(v) => update("show_status_bar", v)} />
        </SettingRow>
        <SettingRow label={t("settings.windowAlwaysOnTop")} description={t("settings.windowAlwaysOnTopDescription")}>
          <Toggle value={settings.window_always_on_top} onChange={(v) => update("window_always_on_top", v)} />
        </SettingRow>
        <SectionHeading>Updates &amp; Performance</SectionHeading>
        <SettingRow label={t("settings.autoUpdate")} description={t("settings.autoUpdateDescription")}>
          <Toggle value={settings.auto_update} onChange={(v) => update("auto_update", v)} />
        </SettingRow>
        <SettingRow label={t("settings.gpuAcceleration")} description={t("settings.gpuAccelerationDescription")}>
          <Toggle value={settings.gpu_acceleration} onChange={(v) => update("gpu_acceleration", v)} />
        </SettingRow>
        <SectionHeading>Profile Sync</SectionHeading>
        <div className="py-2">
          <ProfileSync />
        </div>
      </div>
    );
  }

  function renderAppearance() {
    return (
      <div>
        <SectionHeading>Color Theme</SectionHeading>
        <SettingRow label={t("settings.theme")} description={t("settings.themeDescription")} help={<FieldHelp description={t("fieldHelp.theme")} />}>
          <div className="flex flex-wrap gap-2 justify-end">
            {[
              { value: ThemeVariant.Dark,   label: t("themes.dark"),   icon: <Moon size={12} /> },
              { value: ThemeVariant.Light,  label: t("themes.light"),  icon: <Sun size={12} /> },
              { value: ThemeVariant.System, label: t("themes.system"), icon: <Monitor size={12} /> },
            ].map((opt) => (
              <button
                key={opt.value}
                onClick={() => { setAppTheme(opt.value); setCustomTheme(null, null); update("theme", opt.value); }}
                className={clsx(
                  "flex items-center gap-1.5 px-3 py-1.5 rounded-lg border text-xs transition-colors",
                  appTheme === opt.value && !customThemeName
                    ? "border-border-focus bg-interactive-default/10 text-text-primary"
                    : "border-border-default hover:bg-surface-secondary text-text-secondary",
                )}
              >
                {opt.icon} {opt.label}
                {appTheme === opt.value && !customThemeName ? <Check size={12} className="text-accent-primary" /> : null}
              </button>
            ))}
          </div>
        </SettingRow>
        <SettingRow label={t("themeImport.customTheme")} description={t("settings.themeDescription")}>
          <div className="flex flex-col gap-2">
            <div className="grid grid-cols-2 gap-1.5">
              {BUILT_IN_THEMES.map((tf) => (
                <button
                  key={tf.name}
                  onClick={() => handleSelectBuiltInTheme(tf)}
                  className={clsx(
                    "px-2.5 py-1.5 rounded-lg border text-xs text-left transition-colors",
                    customThemeName === tf.name
                      ? "border-border-focus bg-interactive-default/10 text-text-primary"
                      : "border-border-default hover:bg-surface-secondary text-text-secondary",
                  )}
                >
                  {tf.name}
                  {customThemeName === tf.name ? <Check size={10} className="inline ml-1 text-accent-primary" /> : null}
                </button>
              ))}
            </div>
            <button
              onClick={() => { void handleImportTheme(); }}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary transition-colors"
            >
              <Upload size={13} /> {t("themeImport.importTheme")}
            </button>
          </div>
        </SettingRow>

        <SectionHeading>Font</SectionHeading>
        <SettingRow label={t("settings.fontFamily")}>
          <SelectInput
            value={settings.font_family}
            options={[
              { value: "JetBrains Mono", label: "JetBrains Mono" },
              { value: "Fira Code",      label: "Fira Code" },
              { value: "Cascadia Code",  label: "Cascadia Code" },
              { value: "SF Mono",        label: "SF Mono" },
              { value: "Menlo",          label: "Menlo" },
              { value: "Consolas",       label: "Consolas" },
              { value: "monospace",      label: "System Monospace" },
            ]}
            onChange={(v) => update("font_family", v)}
          />
        </SettingRow>
        <SettingRow label={t("settings.fontSize")} description={t("settings.fontSizeDescription")}>
          <NumberInput value={settings.font_size} onChange={(v) => update("font_size", v)} min={8} max={32} />
        </SettingRow>
        <SettingRow label={t("settings.lineHeight")} description={t("settings.lineHeightDescription")}>
          <NumberInput value={settings.line_height} onChange={(v) => update("line_height", v)} min={1} max={2} step={0.05} />
        </SettingRow>
        <SettingRow label={t("settings.letterSpacing")} description={t("settings.letterSpacingDescription")}>
          <NumberInput value={settings.letter_spacing} onChange={(v) => update("letter_spacing", v)} min={-2} max={10} step={0.5} />
        </SettingRow>
        <SettingRow label={t("settings.fontLigatures")} description={t("settings.fontLigaturesDescription")}>
          <Toggle value={settings.font_ligatures} onChange={(v) => update("font_ligatures", v)} />
        </SettingRow>
      </div>
    );
  }

  function renderTerminal() {
    return (
      <div>
        <SectionHeading>Cursor</SectionHeading>
        <SettingRow label={t("settings.cursorStyle")}>
          <SelectInput
            value={cursorStyle}
            options={[
              { value: "block",     label: t("cursorStyles.block") },
              { value: "underline", label: t("cursorStyles.underline") },
              { value: "bar",       label: t("cursorStyles.bar") },
            ]}
            onChange={(v) => { setCursorStyle(v as CursorStyle); update("cursor_style", v); }}
          />
        </SettingRow>
        <SettingRow label={t("settings.cursorBlink")}>
          <Toggle value={cursorBlink} onChange={(v) => { setCursorBlink(v); update("cursor_blink", v); }} />
        </SettingRow>

        <SectionHeading>Scrolling</SectionHeading>
        <SettingRow label={t("settings.scrollbackLines")} description={t("settings.scrollbackDescription")} help={<FieldHelp description={t("fieldHelp.scrollback")} />}>
          <NumberInput value={settings.scrollback_lines} onChange={(v) => update("scrollback_lines", v)} min={1000} max={100000} step={1000} />
        </SettingRow>
        <SettingRow label={t("settings.scrollbarVisible")} description={t("settings.scrollbarVisibleDescription")}>
          <Toggle value={settings.scrollbar_visible} onChange={(v) => update("scrollbar_visible", v)} />
        </SettingRow>
        <SettingRow label={t("settings.scrollOnOutput")} description={t("settings.scrollOnOutputDescription")}>
          <Toggle value={settings.scroll_on_output} onChange={(v) => update("scroll_on_output", v)} />
        </SettingRow>
        <SettingRow label={t("settings.scrollOnKeystroke")} description={t("settings.scrollOnKeystrokeDescription")}>
          <Toggle value={settings.scroll_on_keystroke} onChange={(v) => update("scroll_on_keystroke", v)} />
        </SettingRow>

        <SectionHeading>Display</SectionHeading>
        <SettingRow label={t("settings.opacity")} description={t("settings.opacityDescription")}>
          <NumberInput value={settings.terminal_opacity} onChange={(v) => update("terminal_opacity", v)} min={0.1} max={1} step={0.05} />
        </SettingRow>
        <SettingRow label={t("settings.bellMode")} description={t("settings.bellDescription")}>
          <SelectInput
            value={bellStyle}
            options={[
              { value: "none",   label: t("bellStyles.none") },
              { value: "visual", label: t("bellStyles.visual") },
              { value: "audio",  label: t("bellStyles.audio") },
            ]}
            onChange={(v) => { setBellStyle(v as BellStyle); update("bell_style", v); }}
          />
        </SettingRow>
        <SettingRow label={t("settings.terminalEncoding")} description={t("settings.terminalEncodingDescription")}>
          <SelectInput
            value={settings.terminal_encoding}
            options={[
              { value: "utf-8",        label: "UTF-8" },
              { value: "utf-16",       label: "UTF-16" },
              { value: "iso-8859-1",   label: "ISO-8859-1 (Latin-1)" },
              { value: "windows-1252", label: "Windows-1252" },
              { value: "shift-jis",    label: "Shift-JIS" },
              { value: "gb18030",      label: "GB18030 (Chinese)" },
            ]}
            onChange={(v) => update("terminal_encoding", v)}
          />
        </SettingRow>
        <SettingRow label={t("settings.wordSeparators")} description={t("settings.wordSeparatorsDescription")}>
          <TextInput value={settings.word_separators} onChange={(v) => update("word_separators", v)} />
        </SettingRow>
      </div>
    );
  }

  function renderSSH() {
    return (
      <div>
        <SectionHeading>Connection Defaults</SectionHeading>
        <SettingRow label={t("settings.defaultShell")} description={t("settings.sshDescription")}>
          <TextInput value={settings.default_shell ?? ""} onChange={(v) => update("default_shell", v || null)} placeholder="/bin/zsh" wide />
        </SettingRow>
        <SettingRow label={t("settings.sshPort")} description={t("settings.sshPortDescription")}>
          <NumberInput value={settings.ssh_port} onChange={(v) => update("ssh_port", v)} min={1} max={65535} />
        </SettingRow>
        <SettingRow label={t("settings.tabTitleFormat")} description={t("settings.tabTitleFormatDescription")}>
          <TextInput value={settings.tab_title_format} onChange={(v) => update("tab_title_format", v)} placeholder="{name} - {host}" wide />
        </SettingRow>

        <SectionHeading>Keep-Alive &amp; Security</SectionHeading>
        <SettingRow label={t("settings.sshKeepalive")} description={t("settings.sshKeepaliveDescription")}>
          <NumberInput value={settings.ssh_keepalive_interval} onChange={(v) => update("ssh_keepalive_interval", v)} min={0} max={600} />
        </SettingRow>
        <SettingRow label={t("settings.sshStrictHostCheck")} description={t("settings.sshStrictHostCheckDescription")}>
          <Toggle value={settings.ssh_strict_host_check} onChange={(v) => update("ssh_strict_host_check", v)} />
        </SettingRow>

        <SectionHeading>Forwarding &amp; Tunnelling</SectionHeading>
        <SettingRow label={t("settings.sshAgentForwarding")} description={t("settings.sshAgentForwardingDescription")}>
          <Toggle value={settings.ssh_agent_forwarding} onChange={(v) => update("ssh_agent_forwarding", v)} />
        </SettingRow>
        <SettingRow label={t("settings.sshX11Forwarding")} description={t("settings.sshX11ForwardingDescription")}>
          <Toggle value={settings.ssh_x11_forwarding} onChange={(v) => update("ssh_x11_forwarding", v)} />
        </SettingRow>

        <SectionHeading>Network</SectionHeading>
        <SettingRow label={t("settings.sshCompression")} description={t("settings.sshCompressionDescription")}>
          <Toggle value={settings.ssh_compression} onChange={(v) => update("ssh_compression", v)} />
        </SettingRow>
      </div>
    );
  }

  function renderConnections() {
    return (
      <div>
        <SectionHeading>Timeouts</SectionHeading>
        <SettingRow label={t("settings.connectionTimeout")} description={t("settings.connectionTimeoutDescription")}>
          <NumberInput value={settings.connection_timeout_secs} onChange={(v) => update("connection_timeout_secs", v)} min={5} max={120} />
        </SettingRow>
        <SettingRow label={t("settings.maxConcurrentConnections")} description={t("settings.maxConcurrentConnectionsDescription")}>
          <NumberInput value={settings.max_concurrent_connections} onChange={(v) => update("max_concurrent_connections", v)} min={1} max={100} />
        </SettingRow>

        <SectionHeading>Auto-Reconnect</SectionHeading>
        <SettingRow label={t("settings.reconnectOnDisconnect")} description={t("settings.reconnectOnDisconnectDescription")}>
          <Toggle value={settings.reconnect_on_disconnect} onChange={(v) => update("reconnect_on_disconnect", v)} />
        </SettingRow>
        <SettingRow label={t("settings.reconnectDelay")} description={t("settings.reconnectDelayDescription")}>
          <NumberInput value={settings.reconnect_delay_secs} onChange={(v) => update("reconnect_delay_secs", v)} min={1} max={60} />
        </SettingRow>
        <SettingRow label={t("settings.reconnectMaxAttempts")} description={t("settings.reconnectMaxAttemptsDescription")}>
          <NumberInput value={settings.reconnect_max_attempts} onChange={(v) => update("reconnect_max_attempts", v)} min={0} max={20} />
        </SettingRow>
      </div>
    );
  }

  function renderFileTransfer() {
    return (
      <div>
        <SectionHeading>SFTP / SCP Defaults</SectionHeading>
        <SettingRow label={t("settings.sftpDefaultDir")} description={t("settings.sftpDefaultDirDescription")}>
          <TextInput value={settings.sftp_default_remote_dir} onChange={(v) => update("sftp_default_remote_dir", v)} placeholder="~" wide />
        </SettingRow>
        <SettingRow label={t("settings.sftpEncoding")} description={t("settings.sftpEncodingDescription")}>
          <SelectInput
            value={settings.sftp_encoding}
            options={[
              { value: "utf-8",        label: "UTF-8" },
              { value: "iso-8859-1",   label: "ISO-8859-1" },
              { value: "windows-1252", label: "Windows-1252" },
              { value: "shift-jis",    label: "Shift-JIS" },
            ]}
            onChange={(v) => update("sftp_encoding", v)}
          />
        </SettingRow>

        <SectionHeading>Transfer Behaviour</SectionHeading>
        <SettingRow label={t("settings.transferConfirmOverwrite")} description={t("settings.transferConfirmOverwriteDescription")}>
          <Toggle value={settings.transfer_confirm_overwrite} onChange={(v) => update("transfer_confirm_overwrite", v)} />
        </SettingRow>
        <SettingRow label={t("settings.transferPreserveTimestamps")} description={t("settings.transferPreserveTimestampsDescription")}>
          <Toggle value={settings.transfer_preserve_timestamps} onChange={(v) => update("transfer_preserve_timestamps", v)} />
        </SettingRow>
        <SettingRow label={t("settings.transferConcurrentJobs")} description={t("settings.transferConcurrentJobsDescription")}>
          <NumberInput value={settings.transfer_concurrent_jobs} onChange={(v) => update("transfer_concurrent_jobs", v)} min={1} max={16} />
        </SettingRow>
      </div>
    );
  }

  function renderKeyboard() {
    return (
      <div>
        <SectionHeading>Key Behaviour</SectionHeading>
        <SettingRow label={t("settings.backspaceSendsDelete")} description={t("settings.backspaceSendsDeleteDescription")}>
          <Toggle value={settings.backspace_sends_delete} onChange={(v) => update("backspace_sends_delete", v)} />
        </SettingRow>
        <SettingRow label={t("settings.altIsMeta")} description={t("settings.altIsMetaDescription")}>
          <Toggle value={settings.alt_is_meta} onChange={(v) => update("alt_is_meta", v)} />
        </SettingRow>
        <SettingRow label={t("settings.ctrlHIsBackspace")} description={t("settings.ctrlHIsBackspaceDescription")}>
          <Toggle value={settings.ctrl_h_is_backspace} onChange={(v) => update("ctrl_h_is_backspace", v)} />
        </SettingRow>
        <SettingRow label={t("settings.homeEndScrollBuffer")} description={t("settings.homeEndScrollBufferDescription")}>
          <Toggle value={settings.home_end_scroll_buffer} onChange={(v) => update("home_end_scroll_buffer", v)} />
        </SettingRow>

        <SectionHeading>Mouse</SectionHeading>
        <SettingRow label={t("settings.rightClickAction")} description={t("settings.rightClickActionDescription")}>
          <SelectInput
            value={settings.right_click_action}
            options={[
              { value: "context_menu", label: "Context menu" },
              { value: "paste",        label: "Paste clipboard" },
              { value: "select",       label: "Extend selection" },
            ]}
            onChange={(v) => update("right_click_action", v)}
          />
        </SettingRow>
        <SettingRow label={t("settings.copyOnSelect")} description={t("settings.copyOnSelectDescription")}>
          <Toggle value={settings.copy_on_select} onChange={(v) => update("copy_on_select", v)} />
        </SettingRow>
      </div>
    );
  }

  function renderNotifications() {
    return (
      <div>
        <SectionHeading>Desktop Notifications</SectionHeading>
        <SettingRow label={t("settings.notifyOnDisconnect")} description={t("settings.notifyOnDisconnectDescription")}>
          <Toggle value={settings.notify_on_disconnect} onChange={(v) => update("notify_on_disconnect", v)} />
        </SettingRow>
        <SettingRow label={t("settings.notifyOnBell")} description={t("settings.notifyOnBellDescription")}>
          <Toggle value={settings.notify_on_bell} onChange={(v) => update("notify_on_bell", v)} />
        </SettingRow>
        <SettingRow label={t("settings.notifyOnLongProcess")} description={t("settings.notifyOnLongProcessDescription")}>
          <Toggle value={settings.notify_on_long_process} onChange={(v) => update("notify_on_long_process", v)} />
        </SettingRow>
        <SettingRow label={t("settings.longProcessThreshold")} description={t("settings.longProcessThresholdDescription")}>
          <NumberInput value={settings.long_process_threshold_secs} onChange={(v) => update("long_process_threshold_secs", v)} min={5} max={300} />
        </SettingRow>

        <SectionHeading>In-App Alerts</SectionHeading>
        <SettingRow label={t("settings.flashTabOnBell")} description={t("settings.flashTabOnBellDescription")}>
          <Toggle value={settings.flash_tab_on_bell} onChange={(v) => update("flash_tab_on_bell", v)} />
        </SettingRow>
      </div>
    );
  }

  function renderSecurity() {
    return (
      <div>
        <SectionHeading>Vault</SectionHeading>
        <SettingRow label={t("settings.vaultAutoLock")} description={t("settings.vaultAutoLockDescription")}>
          <NumberInput
            value={Math.round(settings.idle_lock_timeout_secs / 60)}
            onChange={(v) => update("idle_lock_timeout_secs", v * 60)}
            min={0} max={1440}
          />
        </SettingRow>

        <SectionHeading>Clipboard</SectionHeading>
        <SettingRow label={t("settings.copyOnSelect")} description={t("settings.copyOnSelectDescription")}>
          <Toggle value={settings.copy_on_select} onChange={(v) => update("copy_on_select", v)} />
        </SettingRow>
        <SettingRow label={t("settings.pasteConfirmation")} description={t("settings.pasteConfirmationDescription")}>
          <NumberInput value={settings.paste_warning_lines} onChange={(v) => update("paste_warning_lines", v)} min={0} max={1000} />
        </SettingRow>
        <SettingRow label={t("settings.clipboardHistorySize")} description={t("settings.clipboardHistorySizeDescription")}>
          <NumberInput value={settings.clipboard_history_size} onChange={(v) => update("clipboard_history_size", v)} min={0} max={500} />
        </SettingRow>

        <SectionHeading>Advanced Security</SectionHeading>
        <div className="py-2">
          <SecuritySettings />
        </div>
      </div>
    );
  }

  function renderAdvanced() {
    return (
      <div>
        <SectionHeading>Logging</SectionHeading>
        <SettingRow label={t("settings.logLevel")} description={t("settings.logLevelDescription")}>
          <SelectInput
            value={settings.log_level}
            options={[
              { value: "error", label: "Error" },
              { value: "warn",  label: "Warning" },
              { value: "info",  label: "Info" },
              { value: "debug", label: "Debug" },
              { value: "trace", label: "Trace" },
            ]}
            onChange={(v) => update("log_level", v)}
          />
        </SettingRow>

        <SectionHeading>Privacy</SectionHeading>
        <SettingRow label={t("settings.telemetryEnabled")} description={t("settings.telemetryEnabledDescription")}>
          <Toggle value={settings.telemetry_enabled} onChange={(v) => update("telemetry_enabled", v)} />
        </SettingRow>

        <SectionHeading>Debug Info</SectionHeading>
        <div className="py-2 flex flex-col gap-1.5 text-[11px] text-text-secondary">
          {(() => {
            let platform = "—";
            if (typeof navigator !== "undefined") {
              if (navigator.userAgent.includes("Mac")) platform = "macOS";
              else if (navigator.userAgent.includes("Win")) platform = "Windows";
              else platform = "Linux";
            }
            return [
              { label: "Version", value: "0.2.3" },
              { label: "Platform", value: platform },
              { label: "Renderer", value: settings.gpu_acceleration ? "GPU (WebGL)" : "CPU (Canvas)" },
            ].map(({ label, value }) => (
              <div key={label} className="flex justify-between">
                <span className="text-text-disabled">{label}</span>
                <span className="font-mono">{value}</span>
              </div>
            ));
          })()}
        </div>
        <div className="mt-3">
          <button
            onClick={() => { void invoke("audit_log_export_csv").catch(() => {}); }}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary transition-colors"
          >
            <RefreshCw size={12} /> Export Audit Log
          </button>
        </div>
      </div>
    );
  }

  const renderCategory = () => {
    switch (category) {
      case "general":       return renderGeneral();
      case "appearance":    return renderAppearance();
      case "terminal":      return renderTerminal();
      case "ssh":           return renderSSH();
      case "connections":   return renderConnections();
      case "file_transfer": return renderFileTransfer();
      case "keyboard":      return renderKeyboard();
      case "notifications": return renderNotifications();
      case "security":      return renderSecurity();
      case "advanced":      return renderAdvanced();
    }
  };

  // ── Layout ────────────────────────────────────────────────────────────────

  return (
    <div className="flex h-full bg-surface-primary" data-help-article="getting-started">
      {/* Sidebar nav */}
      <nav className="w-48 border-r border-border-subtle shrink-0 py-3 px-2 overflow-y-auto">
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
            {t(entry.labelKey)}
          </button>
        ))}
      </nav>

      {/* Content pane */}
      <div className="flex-1 overflow-y-auto px-6 py-4">
        <h2 className="text-sm font-semibold text-text-primary mb-1">
          {t(HEADING_KEYS[category])}
        </h2>
        <p className="text-[11px] text-text-secondary mb-4">
          {t(DESCRIPTION_KEYS[category])}
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
