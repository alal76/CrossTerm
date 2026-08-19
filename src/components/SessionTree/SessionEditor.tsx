import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import { X, Save, ChevronRight, ChevronDown } from "lucide-react";
import { v4 as uuidv4 } from "uuid";
import { useSessionStore } from "@/stores/sessionStore";
import { SessionType } from "@/types";
import type { Session, ProxmoxResourceType } from "@/types";
import FieldHelp from "@/components/Help/FieldHelp";

export const SESSION_TYPE_OPTIONS = [
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
  { value: SessionType.Mosh, label: "Mosh" },
  { value: SessionType.WinRM, label: "WinRM / PowerShell" },
  { value: SessionType.WebSocketTerminal, label: "WebSocket Terminal" },
  { value: SessionType.TN3270, label: "TN3270 (IBM Mainframe)" },
  { value: SessionType.TN5250, label: "TN5250 (IBM AS/400)" },
  { value: SessionType.IpmiSol, label: "IPMI Serial-over-LAN" },
  { value: SessionType.Redfish, label: "Redfish (BMC REST)" },
  { value: SessionType.NetConf, label: "NETCONF / YANG" },
  { value: SessionType.Snmp, label: "SNMP" },
  { value: SessionType.Smb, label: "SMB / CIFS" },
  { value: SessionType.WebDav, label: "WebDAV" },
  { value: SessionType.GrpcExplorer, label: "gRPC Explorer" },
  { value: SessionType.MqttClient, label: "MQTT Client" },
  { value: SessionType.KubernetesPortForward, label: "Kubernetes Port-Forward" },
  { value: SessionType.DockerLogs, label: "Docker Logs" },
  { value: SessionType.SpiceConsole, label: "SPICE Console" },
  { value: SessionType.ProxmoxConsole, label: "Proxmox Console" },
  { value: SessionType.Rlogin, label: "Rlogin" },
  { value: SessionType.X11Forward, label: "X11 Forwarding" },
  { value: SessionType.NfsExplorer, label: "NFS Explorer" },
];

const DEFAULT_PORTS: Partial<Record<SessionType, number>> = {
  [SessionType.SSH]: 22,
  [SessionType.SFTP]: 22,
  [SessionType.SCP]: 22,
  [SessionType.RDP]: 3389,
  [SessionType.VNC]: 5900,
  [SessionType.Telnet]: 23,
  [SessionType.Mosh]: 22,
  [SessionType.WinRM]: 5985,
  [SessionType.WebSocketTerminal]: 7681,
  [SessionType.TN3270]: 23,
  [SessionType.TN5250]: 23,
  [SessionType.IpmiSol]: 623,
  [SessionType.Redfish]: 443,
  [SessionType.NetConf]: 830,
  [SessionType.Snmp]: 161,
  [SessionType.Smb]: 445,
  [SessionType.WebDav]: 443,
  [SessionType.GrpcExplorer]: 50051,
  [SessionType.MqttClient]: 1883,
  [SessionType.KubernetesPortForward]: 6443,
  [SessionType.DockerLogs]: 2375,
  [SessionType.SpiceConsole]: 5900,
  [SessionType.ProxmoxConsole]: 8006,
  [SessionType.Rlogin]: 513,
  [SessionType.X11Forward]: 6000,
  [SessionType.NfsExplorer]: 2049,
};

interface SessionEditorProps {
  readonly session?: Session | null;
  readonly defaultType?: SessionType;
  readonly onClose: () => void;
}

interface FormErrors {
  name?: string;
  host?: string;
  port?: string;
  proxmoxNode?: string;
  proxmoxVmid?: string;
  containerId?: string;
  podName?: string;
}

export default function SessionEditor({ session, defaultType, onClose }: SessionEditorProps) {
  const { t } = useTranslation();
  const addSession = useSessionStore((s) => s.addSession);
  const updateSession = useSessionStore((s) => s.updateSession);
  const sessionFolders = useSessionStore((s) => s.sessionFolders);

  const isEdit = !!session;

  const [name, setName] = useState(session?.name ?? "");
  const [type, setType] = useState<SessionType>(session?.type ?? defaultType ?? SessionType.SSH);
  const [host, setHost] = useState(session?.connection.host ?? "");
  const [port, setPort] = useState(String(session?.connection.port ?? DEFAULT_PORTS[SessionType.SSH] ?? 22));
  const [username, setUsername] = useState((session?.connection.protocolOptions?.["username"] as string) ?? "");
  // Shared across every protocol whose config builder (sessionConfig.ts)
  // reads a "password"/"domain" key — VNC, RDP, Redfish, WinRM, IPMI, SMB,
  // NetConf, X11 Forward, and Proxmox Console. One state field each is
  // simpler than N near-identical ones and matches how `username` above is
  // already shared; switching session type mid-edit just carries the
  // typed value along, which is harmless since nothing is saved until Save
  // is clicked.
  const [password, setPassword] = useState((session?.connection.protocolOptions?.["password"] as string) ?? "");
  const [domain, setDomain] = useState((session?.connection.protocolOptions?.["domain"] as string) ?? "");
  // NetConf's builder reads "private_key"; X11 Forward's reads "key_data".
  // Different key names for the same "paste your SSH private key" concept
  // — mapped to the right one at save time based on `type`.
  const [privateKeyContent, setPrivateKeyContent] = useState(
    (session?.connection.protocolOptions?.["private_key"] as string) ??
    (session?.connection.protocolOptions?.["key_data"] as string) ??
    ""
  );
  // Docker Logs / Kubernetes Port-Forward: a Docker host runs many
  // containers and a Kubernetes cluster runs many pods — same class of gap
  // as Proxmox's node/vmid, just for one field instead of two.
  const [containerId, setContainerId] = useState((session?.connection.protocolOptions?.["container_id"] as string) ?? "");
  const [podName, setPodName] = useState((session?.connection.protocolOptions?.["pod_name"] as string) ?? "");
  const [k8sNamespace, setK8sNamespace] = useState((session?.connection.protocolOptions?.["namespace"] as string) ?? "default");
  // Proxmox Console needs a specific node + VM/container ID to know which
  // console to open (a Proxmox host manages many VMs/containers — there's
  // no such thing as "the" console for a bare host/username pair the way
  // there is for SSH). Without these, the connect request goes out with an
  // empty node/vmid and fails; the viewer still looks like a normal VNC
  // connect attempt on screen since Proxmox's console genuinely is VNC
  // tunneled inside a WebSocket, which made this look like a VNC bug
  // rather than missing required fields.
  const [proxmoxNode, setProxmoxNode] = useState((session?.connection.protocolOptions?.["node"] as string) ?? "");
  const [proxmoxVmid, setProxmoxVmid] = useState((session?.connection.protocolOptions?.["vmid"] as string) ?? "");
  const [proxmoxResourceType, setProxmoxResourceType] = useState<ProxmoxResourceType>(
    (session?.connection.protocolOptions?.["resource_type"] as ProxmoxResourceType) ?? "qemu"
  );
  const [proxmoxRealm, setProxmoxRealm] = useState((session?.connection.protocolOptions?.["realm"] as string) ?? "pam");
  const [group, setGroup] = useState(session?.group ?? "");
  const [tags, setTags] = useState(session?.tags.join(", ") ?? "");
  const [credentialRef, setCredentialRef] = useState(session?.credentialRef ?? "");
  const [startupScript, setStartupScript] = useState(session?.startupScript ?? "");
  const [notes, setNotes] = useState(session?.notes ?? "");
  const [errors, setErrors] = useState<FormErrors>({});
  const [advancedOpen, setAdvancedOpen] = useState(
    !!(session?.group || session?.tags.length || session?.credentialRef || session?.startupScript || session?.notes)
  );

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

    // A Proxmox host manages many VMs/containers — without a node + VM ID
    // there's no way to know which console to open, and the connect
    // request goes out malformed. Required, not optional/advanced.
    if (type === SessionType.ProxmoxConsole) {
      if (!proxmoxNode.trim()) errs.proxmoxNode = "Node is required";
      if (!proxmoxVmid.trim()) errs.proxmoxVmid = "VM/CT ID is required";
    }
    if (type === SessionType.DockerLogs && !containerId.trim()) {
      errs.containerId = "Container ID or name is required";
    }
    if (type === SessionType.KubernetesPortForward && !podName.trim()) {
      errs.podName = "Pod name is required";
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

    const usernameVal = username.trim();
    const protocolOptions: Record<string, unknown> = {};
    if (usernameVal) protocolOptions.username = usernameVal;
    if (needsPassword && password) protocolOptions.password = password;
    if (needsDomain && domain.trim()) protocolOptions.domain = domain.trim();
    if (needsPrivateKey && privateKeyContent.trim()) {
      // Different builders read different key names for the same concept
      // (see the privateKeyContent state comment above).
      protocolOptions[type === SessionType.NetConf ? "private_key" : "key_data"] = privateKeyContent;
    }
    if (isDockerLogs && containerId.trim()) protocolOptions.container_id = containerId.trim();
    if (isK8sPortForward) {
      if (podName.trim()) protocolOptions.pod_name = podName.trim();
      if (k8sNamespace.trim()) protocolOptions.namespace = k8sNamespace.trim();
    }
    if (isProxmoxConsole) {
      protocolOptions.node = proxmoxNode.trim();
      protocolOptions.vmid = proxmoxVmid.trim();
      protocolOptions.resource_type = proxmoxResourceType;
      protocolOptions.realm = proxmoxRealm.trim() || "pam";
    }
    const hasProtocolOptions = Object.keys(protocolOptions).length > 0;

    if (isEdit && session) {
      updateSession(session.id, {
        name: name.trim(),
        type,
        group: group.trim(),
        tags: parsedTags,
        credentialRef: credentialRef.trim() || undefined,
        connection: { host: host.trim(), port: portNum, protocolOptions: hasProtocolOptions ? protocolOptions : undefined },
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
        connection: { host: host.trim(), port: portNum, protocolOptions: hasProtocolOptions ? protocolOptions : undefined },
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
  const isProxmoxConsole = type === SessionType.ProxmoxConsole;
  const isDockerLogs = type === SessionType.DockerLogs;
  const isK8sPortForward = type === SessionType.KubernetesPortForward;
  // These protocols' config builders (sessionConfig.ts) read a "password"
  // key — VNC/RDP/Redfish/WinRM/IPMI/SMB always need it in real
  // deployments; NetConf/X11 Forward accept it as one of two valid auth
  // methods (see needsPrivateKey below). Not validated as required here:
  // unlike a resource identifier, an empty password can be genuinely
  // correct (e.g. a no-auth VNC/dev SMB share), so this stays optional
  // rather than blocking Save.
  const needsPassword = [
    SessionType.VNC, SessionType.RDP, SessionType.Redfish, SessionType.WinRM,
    SessionType.IpmiSol, SessionType.Smb, SessionType.NetConf, SessionType.X11Forward,
  ].includes(type) || isProxmoxConsole;
  const needsDomain = type === SessionType.RDP || type === SessionType.Smb;
  const needsPrivateKey = type === SessionType.NetConf || type === SessionType.X11Forward;

  return (
    <dialog
      open
      className="fixed inset-0 z-[8000] flex items-center justify-center"
      aria-modal="true"
      aria-label={isEdit ? "Edit Session" : "New Session"}
    >
      <div
        className="absolute inset-0 bg-surface-overlay/60 backdrop-blur-sm"
        onClick={onClose}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
        aria-hidden="true"
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

          {/* Username */}
          {needsHost && (
            <Field label="Username">
              <input
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="e.g. admin, ubuntu, root"
                autoComplete="username"
                className={inputClass(false)}
              />
            </Field>
          )}

          {/* Password — shared across every protocol whose builder reads a
              "password" key. Left optional (no validation error) since an
              empty password is sometimes genuinely correct (e.g. a no-auth
              VNC server or an anonymous SMB share), unlike the resource
              identifiers below. */}
          {needsPassword && (
            <Field label="Password">
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={isProxmoxConsole ? "Proxmox API password" : "Password"}
                autoComplete="current-password"
                className={inputClass(false)}
              />
            </Field>
          )}

          {/* Domain — RDP (AD domain) and SMB (workgroup/domain). */}
          {needsDomain && (
            <Field label="Domain">
              <input
                value={domain}
                onChange={(e) => setDomain(e.target.value)}
                placeholder="e.g. CORP, WORKGROUP"
                className={inputClass(false)}
              />
            </Field>
          )}

          {/* Private key — NetConf and X11 Forward accept this as an
              alternative to the password field above; either one alone is
              enough to authenticate. */}
          {needsPrivateKey && (
            <Field label="Private Key (alternative to password)">
              <textarea
                value={privateKeyContent}
                onChange={(e) => setPrivateKeyContent(e.target.value)}
                placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
                rows={3}
                className={clsx(inputClass(false), "font-mono text-[11px]")}
              />
            </Field>
          )}

          {/* Docker Logs: a Docker host runs many containers — there's no
              "the" log stream for a bare host the way there is for SSH. */}
          {isDockerLogs && (
            <Field label="Container ID or Name" error={errors.containerId}>
              <input
                value={containerId}
                onChange={(e) => setContainerId(e.target.value)}
                placeholder="e.g. web-app or a1b2c3d4e5f6"
                className={inputClass(!!errors.containerId)}
              />
            </Field>
          )}

          {/* Kubernetes Port-Forward: a cluster runs many pods across many
              namespaces — same class of gap as Docker Logs' container ID. */}
          {isK8sPortForward && (
            <div className="grid grid-cols-2 gap-2">
              <Field label="Pod Name" error={errors.podName}>
                <input
                  value={podName}
                  onChange={(e) => setPodName(e.target.value)}
                  placeholder="e.g. web-app-7d8f9c-x2k4p"
                  className={inputClass(!!errors.podName)}
                />
              </Field>
              <Field label="Namespace">
                <input
                  value={k8sNamespace}
                  onChange={(e) => setK8sNamespace(e.target.value)}
                  placeholder="default"
                  className={inputClass(false)}
                />
              </Field>
            </div>
          )}

          {/* Proxmox Console: node + VM/CT ID are required to know which
              console to open — a Proxmox host manages many VMs/containers,
              so a bare host/username pair isn't enough, unlike SSH. */}
          {isProxmoxConsole && (
            <>
              <div className="grid grid-cols-2 gap-2">
                <Field label="Node" error={errors.proxmoxNode}>
                  <input
                    value={proxmoxNode}
                    onChange={(e) => setProxmoxNode(e.target.value)}
                    placeholder="pve1"
                    className={inputClass(!!errors.proxmoxNode)}
                  />
                </Field>
                <Field label="VM/CT ID" error={errors.proxmoxVmid}>
                  <input
                    value={proxmoxVmid}
                    onChange={(e) => setProxmoxVmid(e.target.value)}
                    placeholder="100"
                    className={inputClass(!!errors.proxmoxVmid)}
                  />
                </Field>
              </div>
              <div className="grid grid-cols-2 gap-2">
                <Field label="Resource Type">
                  <select
                    value={proxmoxResourceType}
                    onChange={(e) => setProxmoxResourceType(e.target.value as ProxmoxResourceType)}
                    className={inputClass(false)}
                  >
                    <option value="qemu">QEMU/KVM VM</option>
                    <option value="lxc">LXC Container</option>
                  </select>
                </Field>
                <Field label="Realm">
                  <input
                    value={proxmoxRealm}
                    onChange={(e) => setProxmoxRealm(e.target.value)}
                    placeholder="pam"
                    className={inputClass(false)}
                  />
                </Field>
              </div>
            </>
          )}

          {/* Advanced toggle */}
          <button
            type="button"
            onClick={() => setAdvancedOpen((v) => !v)}
            className="flex items-center gap-1 text-[11px] text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
            aria-expanded={advancedOpen}
          >
            {advancedOpen ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
            Advanced
          </button>

          {advancedOpen && (
            <div className="space-y-3.5">
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
          )}
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
    </dialog>
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
