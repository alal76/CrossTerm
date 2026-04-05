import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import { X, Save } from "lucide-react";
import { v4 as uuidv4 } from "uuid";
import { useSessionStore } from "@/stores/sessionStore";
import { SessionType } from "@/types";
import type { Session } from "@/types";
import FieldHelp from "@/components/Help/FieldHelp";

const SESSION_TYPE_OPTIONS = [
  { value: SessionType.SSH, label: "SSH" },
  { value: SessionType.SFTP, label: "SFTP" },
  { value: SessionType.SCP, label: "SCP" },
  { value: SessionType.RDP, label: "RDP" },
  { value: SessionType.VNC, label: "VNC" },
  { value: SessionType.Telnet, label: "Telnet" },
  { value: SessionType.Serial, label: "Serial" },
  { value: SessionType.LocalShell, label: "Local Shell" },
  { value: SessionType.WSL, label: "WSL" },
  { value: SessionType.CloudShell, label: "Cloud Shell" },
  { value: SessionType.KubernetesExec, label: "Kubernetes Exec" },
  { value: SessionType.DockerExec, label: "Docker Exec" },
  { value: SessionType.WebConsole, label: "Web Console" },
];

const DEFAULT_PORTS: Partial<Record<SessionType, number>> = {
  [SessionType.SSH]: 22,
  [SessionType.SFTP]: 22,
  [SessionType.SCP]: 22,
  [SessionType.RDP]: 3389,
  [SessionType.VNC]: 5900,
  [SessionType.Telnet]: 23,
};

interface SessionEditorProps {
  readonly session?: Session | null;
  readonly onClose: () => void;
}

interface FormErrors {
  name?: string;
  host?: string;
  port?: string;
}

export default function SessionEditor({ session, onClose }: SessionEditorProps) {
  const { t } = useTranslation();
  const addSession = useSessionStore((s) => s.addSession);
  const updateSession = useSessionStore((s) => s.updateSession);
  const sessionFolders = useSessionStore((s) => s.sessionFolders);

  const isEdit = !!session;

  const [name, setName] = useState(session?.name ?? "");
  const [type, setType] = useState<SessionType>(session?.type ?? SessionType.SSH);
  const [host, setHost] = useState(session?.connection.host ?? "");
  const [port, setPort] = useState(String(session?.connection.port ?? DEFAULT_PORTS[SessionType.SSH] ?? 22));
  const [group, setGroup] = useState(session?.group ?? "");
  const [tags, setTags] = useState(session?.tags.join(", ") ?? "");
  const [credentialRef, setCredentialRef] = useState(session?.credentialRef ?? "");
  const [startupScript, setStartupScript] = useState(session?.startupScript ?? "");
  const [notes, setNotes] = useState(session?.notes ?? "");
  const [errors, setErrors] = useState<FormErrors>({});

  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    requestAnimationFrame(() => nameRef.current?.focus());
  }, []);

  // Update default port when type changes
  useEffect(() => {
    if (!isEdit) {
      const defaultPort = DEFAULT_PORTS[type];
      if (defaultPort) setPort(String(defaultPort));
    }
  }, [type, isEdit]);

  function validate(): boolean {
    const errs: FormErrors = {};

    if (!name.trim()) errs.name = "Name is required";

    const needsHost = type !== SessionType.LocalShell && type !== SessionType.WSL;
    if (needsHost && !host.trim()) errs.host = "Host is required";

    const portNum = Number.parseInt(port, 10);
    if (needsHost && (Number.isNaN(portNum) || portNum < 1 || portNum > 65535)) {
      errs.port = "Port must be between 1 and 65535";
    }

    setErrors(errs);
    return Object.keys(errs).length === 0;
  }

  function handleSave() {
    if (!validate()) return;

    const now = new Date().toISOString();
    const portNum = Number.parseInt(port, 10) || 22;
    const parsedTags = tags
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean);

    if (isEdit && session) {
      updateSession(session.id, {
        name: name.trim(),
        type,
        group: group.trim(),
        tags: parsedTags,
        credentialRef: credentialRef.trim() || undefined,
        connection: { host: host.trim(), port: portNum },
        startupScript: startupScript.trim() || undefined,
        notes: notes.trim() || undefined,
      });
    } else {
      const newSession: Session = {
        id: uuidv4(),
        name: name.trim(),
        type,
        group: group.trim(),
        tags: parsedTags,
        credentialRef: credentialRef.trim() || undefined,
        connection: { host: host.trim(), port: portNum },
        startupScript: startupScript.trim() || undefined,
        notes: notes.trim() || undefined,
        createdAt: now,
        updatedAt: now,
        autoReconnect: false,
        keepAliveIntervalSeconds: 60,
      };
      addSession(newSession);
    }

    onClose();
  }

  const needsHost = type !== SessionType.LocalShell && type !== SessionType.WSL;

  return (
    <div
      className="fixed inset-0 z-[8000] flex items-center justify-center"
      role="dialog"
      aria-modal="true"
      aria-label={isEdit ? "Edit Session" : "New Session"}
    >
      <div
        className="absolute inset-0 bg-surface-overlay/60 backdrop-blur-sm"
        onClick={onClose}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
        role="presentation"
      />
      <div
        className="relative w-full max-w-md max-h-[90vh] bg-surface-elevated border border-border-default rounded-xl shadow-[var(--shadow-3)] flex flex-col overflow-hidden"
        style={{ animation: "paletteIn var(--duration-medium) var(--ease-decelerate)" }}
        data-help-article="ssh-connections"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-border-subtle shrink-0">
          <h2 className="text-sm font-semibold text-text-primary">
            {isEdit ? "Edit Session" : "New Session"}
          </h2>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
          >
            <X size={16} />
          </button>
        </div>

        {/* Form */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-3.5">
          {/* Name */}
          <Field label="Name" error={errors.name}>
            <input
              ref={nameRef}
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My Server"
              className={inputClass(!!errors.name)}
            />
          </Field>

          {/* Type */}
          <Field label="Type" help={<FieldHelp description={t("fieldHelp.sessionType")} articleSlug="getting-started" />}>
            <select
              value={type}
              onChange={(e) => setType(e.target.value as SessionType)}
              className={inputClass(false)}
            >
              {SESSION_TYPE_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </Field>

          {/* Host + Port */}
          {needsHost && (
            <div className="grid grid-cols-[1fr_80px] gap-2">
              <Field label="Host" error={errors.host} help={<FieldHelp description={t("fieldHelp.hostname")} articleSlug="ssh-connections" />}>
                <input
                  value={host}
                  onChange={(e) => setHost(e.target.value)}
                  placeholder="192.168.1.100"
                  className={inputClass(!!errors.host)}
                />
              </Field>
              <Field label="Port" error={errors.port} help={<FieldHelp description={t("fieldHelp.port")} articleSlug="ssh-connections" />}>
                <input
                  value={port}
                  onChange={(e) => setPort(e.target.value)}
                  placeholder="22"
                  className={inputClass(!!errors.port)}
                />
              </Field>
            </div>
          )}

          {/* Group */}
          <Field label="Group / Folder">
            <input
              value={group}
              onChange={(e) => setGroup(e.target.value)}
              placeholder="Production"
              list="session-folders"
              className={inputClass(false)}
            />
            <datalist id="session-folders">
              {sessionFolders.map((f) => (
                <option key={f} value={f} />
              ))}
            </datalist>
          </Field>

          {/* Tags */}
          <Field label="Tags (comma-separated)">
            <input
              value={tags}
              onChange={(e) => setTags(e.target.value)}
              placeholder="web, staging"
              className={inputClass(false)}
            />
          </Field>

          {/* Credential Reference */}
          <Field label="Credential">
            <input
              value={credentialRef}
              onChange={(e) => setCredentialRef(e.target.value)}
              placeholder="Credential name or ID"
              className={inputClass(false)}
            />
          </Field>

          {/* Startup Script */}
          <Field label="Startup Script">
            <textarea
              value={startupScript}
              onChange={(e) => setStartupScript(e.target.value)}
              placeholder="Commands to run after connection..."
              rows={2}
              className={clsx(inputClass(false), "resize-none font-mono text-xs")}
            />
          </Field>

          {/* Notes */}
          <Field label="Notes">
            <textarea
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              placeholder="Optional notes…"
              rows={2}
              className={clsx(inputClass(false), "resize-none")}
            />
          </Field>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border-subtle shrink-0">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            <Save size={13} />
            {isEdit ? "Save" : "Create"}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Helpers ──

function Field({
  label,
  error,
  help,
  children,
}: {
  readonly label: string;
  readonly error?: string;
  readonly help?: React.ReactNode;
  readonly children: React.ReactNode;
}) {
  return (
    <div>
      <label className="flex items-center text-[11px] text-text-secondary mb-1">
        {label}
        {help}
      </label>
      {children}
      {error && <p className="text-[10px] text-status-disconnected mt-0.5">{error}</p>}
    </div>
  );
}

function inputClass(hasError: boolean) {
  return clsx(
    "w-full px-2.5 py-2 rounded-lg text-xs bg-surface-secondary border outline-none",
    "text-text-primary placeholder:text-text-disabled",
    "transition-colors duration-[var(--duration-short)]",
    "focus:border-border-focus",
    hasError ? "border-status-disconnected" : "border-border-default"
  );
}
