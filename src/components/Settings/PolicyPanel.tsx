import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import clsx from "clsx";
import { Plus, X, Download, Upload, Check } from "lucide-react";

// ── Types ────────────────────────────────────────────────────────────────────

interface RecordingPolicy {
  enabled: boolean;
  require_recording_for: Array<{ "0": string }>;
  storage_path: string | null;
  retention_days: number;
  encrypt_recordings: boolean;
  notify_user: boolean;
  allow_user_disable: boolean;
}

interface PolicyConfig {
  recording: RecordingPolicy;
  max_session_duration_minutes: number | null;
  require_mfa_for_privileged: boolean;
  allowed_protocols: string[];
  blocked_hosts: Array<{ "0": string }>;
  audit_all_commands: boolean;
}

const ALL_PROTOCOLS = ["ssh", "sftp", "rdp", "vnc", "telnet", "serial", "ftp"] as const;
type Protocol = (typeof ALL_PROTOCOLS)[number];

const PROTOCOL_LABELS: Record<Protocol, string> = {
  ssh: "SSH",
  sftp: "SFTP",
  rdp: "RDP",
  vnc: "VNC",
  telnet: "Telnet",
  serial: "Serial",
  ftp: "FTP",
};

const DEFAULT_POLICY: PolicyConfig = {
  recording: {
    enabled: false,
    require_recording_for: [],
    storage_path: null,
    retention_days: 90,
    encrypt_recordings: false,
    notify_user: true,
    allow_user_disable: false,
  },
  max_session_duration_minutes: null,
  require_mfa_for_privileged: false,
  allowed_protocols: [],
  blocked_hosts: [],
  audit_all_commands: false,
};

// ── Shared sub-components (mirror SettingsPanel style) ────────────────────────

function SectionHeading({ children }: Readonly<{ children: React.ReactNode }>) {
  return (
    <p className="text-[10px] font-semibold uppercase tracking-wider text-text-disabled mt-5 mb-1 first:mt-0">
      {children}
    </p>
  );
}

function SettingRow({
  label,
  description,
  children,
}: Readonly<{
  label: string;
  description?: string;
  children: React.ReactNode;
}>) {
  return (
    <div className="flex items-start justify-between gap-4 py-3 border-b border-border-subtle last:border-0">
      <div className="flex-1 min-w-0">
        <p className="text-xs text-text-primary">{label}</p>
        {description ? (
          <p className="text-[10px] text-text-secondary mt-0.5">{description}</p>
        ) : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Toggle({
  value,
  onChange,
}: Readonly<{ value: boolean; onChange: (v: boolean) => void }>) {
  return (
    <button
      type="button"
      onClick={() => onChange(!value)}
      className={clsx(
        "relative w-9 h-5 rounded-full transition-colors duration-[var(--duration-short)]",
        value
          ? "bg-accent-primary"
          : "bg-surface-secondary border border-border-default",
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

function NumberInput({
  value,
  placeholder,
  onChange,
  min,
  max,
}: Readonly<{
  value: number | null;
  placeholder?: string;
  onChange: (v: number | null) => void;
  min?: number;
  max?: number;
}>) {
  return (
    <input
      type="number"
      value={value ?? ""}
      placeholder={placeholder}
      onChange={(e) => {
        const raw = e.target.value;
        if (raw === "") { onChange(null); return; }
        const n = Number.parseInt(raw, 10);
        if (!Number.isNaN(n)) onChange(n);
      }}
      min={min}
      max={max}
      className="w-24 px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus text-right transition-colors"
    />
  );
}

// ── Pattern list (add / remove chips) ─────────────────────────────────────────

function PatternList({
  patterns,
  placeholder,
  onChange,
}: Readonly<{
  patterns: string[];
  placeholder: string;
  onChange: (v: string[]) => void;
}>) {
  const [draft, setDraft] = useState("");

  const add = () => {
    const trimmed = draft.trim();
    if (!trimmed || patterns.includes(trimmed)) return;
    onChange([...patterns, trimmed]);
    setDraft("");
  };

  const remove = (idx: number) =>
    onChange(patterns.filter((_, i) => i !== idx));

  return (
    <div className="flex flex-col gap-1.5 w-56">
      {/* Input + add button */}
      <div className="flex gap-1">
        <input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") { e.preventDefault(); add(); }
          }}
          placeholder={placeholder}
          className="flex-1 min-w-0 px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors"
        />
        <button
          type="button"
          onClick={add}
          disabled={!draft.trim()}
          className="flex items-center justify-center w-6 h-6 rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors disabled:opacity-40"
        >
          <Plus size={11} />
        </button>
      </div>

      {/* Existing patterns */}
      {patterns.length > 0 ? (
        <ul className="flex flex-col gap-1">
          {patterns.map((pattern, idx) => (
            <li
              key={`${pattern}-${idx}`}
              className="flex items-center justify-between gap-1 px-2 py-0.5 rounded bg-surface-secondary border border-border-subtle"
            >
              <span className="text-[11px] font-mono text-text-primary truncate">
                {pattern}
              </span>
              <button
                type="button"
                onClick={() => remove(idx)}
                className="shrink-0 text-text-secondary hover:text-status-disconnected transition-colors"
                aria-label={`Remove ${pattern}`}
              >
                <X size={10} />
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="text-[10px] text-text-disabled italic">No patterns added</p>
      )}
    </div>
  );
}

// ── Protocol checkboxes ────────────────────────────────────────────────────────

function ProtocolCheckboxes({
  selected,
  onChange,
}: Readonly<{
  selected: string[];
  onChange: (v: string[]) => void;
}>) {
  const toggle = (proto: string) => {
    if (selected.includes(proto)) {
      onChange(selected.filter((p) => p !== proto));
    } else {
      onChange([...selected, proto]);
    }
  };

  return (
    <div className="flex flex-wrap gap-1.5 justify-end">
      {ALL_PROTOCOLS.map((proto) => {
        const active = selected.includes(proto);
        return (
          <button
            key={proto}
            type="button"
            onClick={() => toggle(proto)}
            className={clsx(
              "flex items-center gap-1 px-2.5 py-1 rounded-lg border text-[11px] transition-colors",
              active
                ? "border-border-focus bg-interactive-default/10 text-text-primary"
                : "border-border-default hover:bg-surface-secondary text-text-secondary",
            )}
          >
            {active ? <Check size={10} className="text-accent-primary" /> : null}
            {PROTOCOL_LABELS[proto]}
          </button>
        );
      })}
      {selected.length > 0 ? (
        <p className="w-full text-[10px] text-text-disabled mt-0.5">
          All other protocols will be blocked.
        </p>
      ) : (
        <p className="w-full text-[10px] text-text-disabled mt-0.5">
          No restrictions — all protocols are allowed.
        </p>
      )}
    </div>
  );
}

// ── Saved indicator ────────────────────────────────────────────────────────────

function SavedIndicator({ visible }: Readonly<{ visible: boolean }>) {
  return (
    <span
      className={clsx(
        "inline-flex items-center gap-1 text-[11px] text-status-connected transition-opacity duration-[var(--duration-medium)]",
        visible ? "opacity-100" : "opacity-0 pointer-events-none",
      )}
    >
      <Check size={12} /> Saved
    </span>
  );
}

// ── Helper: convert backend HostPattern tuple-struct to plain string ───────────

function patternToString(p: { "0": string } | string): string {
  if (typeof p === "string") return p;
  return p["0"] ?? "";
}

function stringsToPatterns(strs: string[]): Array<{ "0": string }> {
  return strs.map((s) => ({ "0": s }));
}

// ── Main component ─────────────────────────────────────────────────────────────

export default function PolicyPanel() {
  const [policy, setPolicy] = useState<PolicyConfig>(DEFAULT_POLICY);
  const [loading, setLoading] = useState(true);
  const [savedVisible, setSavedVisible] = useState(false);
  const savedTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ── Load on mount ──────────────────────────────────────────────────────────

  useEffect(() => {
    let mounted = true;
    async function load() {
      try {
        const loaded = await invoke<PolicyConfig>("policy_get");
        if (mounted) setPolicy({ ...DEFAULT_POLICY, ...loaded });
      } catch {
        if (mounted) setPolicy(DEFAULT_POLICY);
      } finally {
        if (mounted) setLoading(false);
      }
    }
    void load();
    return () => { mounted = false; };
  }, []);

  // ── Save ───────────────────────────────────────────────────────────────────

  const persist = useCallback(async (next: PolicyConfig) => {
    setPolicy(next);
    try {
      await invoke("policy_update", { config: next });
      // Show the "Saved" toast for 2.5 s
      if (savedTimerRef.current) clearTimeout(savedTimerRef.current);
      setSavedVisible(true);
      savedTimerRef.current = setTimeout(() => setSavedVisible(false), 2500);
    } catch {
      /* keep optimistic state; in a real app surface an error toast */
    }
  }, []);

  const updateRecording = useCallback(
    <K extends keyof RecordingPolicy>(key: K, value: RecordingPolicy[K]) => {
      void persist({ ...policy, recording: { ...policy.recording, [key]: value } });
    },
    [persist, policy],
  );

  const updateRoot = useCallback(
    <K extends keyof PolicyConfig>(key: K, value: PolicyConfig[K]) => {
      void persist({ ...policy, [key]: value });
    },
    [persist, policy],
  );

  // ── Export / Import ────────────────────────────────────────────────────────

  const handleExport = useCallback(async () => {
    try {
      const dest = await save({
        title: "Export Policy JSON",
        defaultPath: "crossterm-policy.json",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!dest) return;
      await writeTextFile(dest, JSON.stringify(policy, null, 2));
    } catch {
      /* ignore user cancellations */
    }
  }, [policy]);

  const handleImport = useCallback(async () => {
    try {
      const selected = await open({
        title: "Import Policy JSON",
        filters: [{ name: "JSON", extensions: ["json"] }],
        multiple: false,
      });
      if (!selected || Array.isArray(selected)) return;
      const content = await readTextFile(selected);
      const parsed = JSON.parse(content) as PolicyConfig;
      await persist({ ...DEFAULT_POLICY, ...parsed });
    } catch {
      /* ignore parse / user cancellation errors */
    }
  }, [persist]);

  // ── Derived helpers ────────────────────────────────────────────────────────

  const recordingPatterns = (policy.recording.require_recording_for ?? []).map(
    patternToString,
  );
  const blockedPatterns = (policy.blocked_hosts ?? []).map(patternToString);

  // ── Render ─────────────────────────────────────────────────────────────────

  if (loading) {
    return (
      <div className="text-xs text-text-secondary py-8 text-center">Loading policy…</div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto px-6 py-4">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <h2 className="text-sm font-semibold text-text-primary">Policy</h2>
          <p className="text-[11px] text-text-secondary mt-0.5">
            Compliance and access-control rules enforced across all sessions.
          </p>
        </div>
        <SavedIndicator visible={savedVisible} />
      </div>

      {/* ── 1. Recording Policy ────────────────────────────────────────── */}
      <SectionHeading>Recording Policy</SectionHeading>

      <SettingRow
        label="Enable session recording"
        description="Record terminal sessions to disk according to the rules below."
      >
        <Toggle
          value={policy.recording.enabled}
          onChange={(v) => updateRecording("enabled", v)}
        />
      </SettingRow>

      <SettingRow
        label="Record sessions to these hosts"
        description="Glob patterns (e.g. *.prod.example.com). Leave empty to record all sessions."
      >
        <PatternList
          patterns={recordingPatterns}
          placeholder="*.prod.example.com"
          onChange={(strs) =>
            updateRecording(
              "require_recording_for",
              stringsToPatterns(strs),
            )
          }
        />
      </SettingRow>

      <SettingRow
        label="Recording storage path"
        description="Override the default storage directory. Leave blank to use the app default."
      >
        <input
          value={policy.recording.storage_path ?? ""}
          onChange={(e) =>
            updateRecording("storage_path", e.target.value || null)
          }
          placeholder="/var/log/crossterm/recordings"
          className="w-56 px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors"
        />
      </SettingRow>

      <SettingRow
        label="Retention (days)"
        description="Automatically delete recordings older than this many days. 0 = keep forever."
      >
        <NumberInput
          value={policy.recording.retention_days}
          onChange={(v) => updateRecording("retention_days", v ?? 0)}
          min={0}
          max={3650}
        />
      </SettingRow>

      <SettingRow
        label="Encrypt recordings"
        description="Store recording files in an encrypted format."
      >
        <Toggle
          value={policy.recording.encrypt_recordings}
          onChange={(v) => updateRecording("encrypt_recordings", v)}
        />
      </SettingRow>

      <SettingRow
        label="Notify user"
        description="Show a compliance banner to the user during recorded sessions."
      >
        <Toggle
          value={policy.recording.notify_user}
          onChange={(v) => updateRecording("notify_user", v)}
        />
      </SettingRow>

      <SettingRow
        label="Allow user to disable recording"
        description="Permit users to opt out of recording from the compliance banner."
      >
        <Toggle
          value={policy.recording.allow_user_disable}
          onChange={(v) => updateRecording("allow_user_disable", v)}
        />
      </SettingRow>

      {/* ── 2. Connection Policy ───────────────────────────────────────── */}
      <SectionHeading>Connection Policy</SectionHeading>

      <SettingRow
        label="Blocked hosts"
        description="Deny-list of hostnames (exact or glob). Connections to these hosts will be refused."
      >
        <PatternList
          patterns={blockedPatterns}
          placeholder="*.darknet.example"
          onChange={(strs) =>
            updateRoot("blocked_hosts", stringsToPatterns(strs))
          }
        />
      </SettingRow>

      <SettingRow
        label="Allowed protocols"
        description="Select the protocols permitted for new connections. Leaving all unchecked allows everything."
      >
        <ProtocolCheckboxes
          selected={policy.allowed_protocols}
          onChange={(v) => updateRoot("allowed_protocols", v)}
        />
      </SettingRow>

      <SettingRow
        label="Max session duration (minutes)"
        description="Forcibly close sessions exceeding this length. Leave blank for no limit."
      >
        <NumberInput
          value={policy.max_session_duration_minutes}
          placeholder="Unlimited"
          onChange={(v) => updateRoot("max_session_duration_minutes", v)}
          min={1}
          max={14400}
        />
      </SettingRow>

      {/* ── 3. Compliance ─────────────────────────────────────────────── */}
      <SectionHeading>Compliance</SectionHeading>

      <SettingRow
        label="Require MFA for privileged sessions"
        description="Prompt for multi-factor authentication before opening root or admin sessions."
      >
        <Toggle
          value={policy.require_mfa_for_privileged}
          onChange={(v) => updateRoot("require_mfa_for_privileged", v)}
        />
      </SettingRow>

      <SettingRow
        label="Audit all commands"
        description="Capture every command entered across all sessions in the audit log."
      >
        <Toggle
          value={policy.audit_all_commands}
          onChange={(v) => updateRoot("audit_all_commands", v)}
        />
      </SettingRow>

      {/* ── 4. Import / Export ────────────────────────────────────────── */}
      <SectionHeading>Import / Export</SectionHeading>

      <div className="py-2 flex flex-col gap-2 text-[11px] text-text-secondary">
        <p className="text-[10px] text-text-disabled">
          Export the current policy as JSON for MDM deployment, or import a
          policy file distributed by your IT team.
        </p>

        <div className="flex gap-2 flex-wrap">
          {/* Export */}
          <button
            type="button"
            onClick={() => { void handleExport(); }}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors"
          >
            <Download size={12} />
            Export Policy JSON
          </button>

          {/* Import */}
          <button
            type="button"
            onClick={() => { void handleImport(); }}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors"
          >
            <Upload size={12} />
            Import Policy JSON
          </button>
        </div>
      </div>
    </div>
  );
}
