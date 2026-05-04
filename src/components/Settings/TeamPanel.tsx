import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import { UserPlus, Shield, Trash2, Users, Settings2 } from "lucide-react";
import { useToast } from "@/components/Shared/Toast";

// ── Types ─────────────────────────────────────────────────────────────────────

type RoleType =
  | "admin"
  | "power_user"
  | "read_only"
  | "auditor"
  | { custom: string[] };

interface TeamMember {
  id: string;
  display_name: string;
  email: string | null;
  role: RoleType;
  public_key: string | null;
  added_at: string;
  last_active: string | null;
}

interface TeamConfig {
  members: TeamMember[];
  require_mfa: boolean;
  session_timeout_minutes: number;
  allowed_ips: string[];
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const ROLE_OPTIONS: Array<{ value: string; label: string }> = [
  { value: "admin",      label: "Admin" },
  { value: "power_user", label: "Power User" },
  { value: "read_only",  label: "Read Only" },
  { value: "auditor",    label: "Auditor" },
];

function roleLabel(role: RoleType): string {
  if (role === "admin")      return "🛡 Admin";
  if (role === "power_user") return "⚡ Power User";
  if (role === "read_only")  return "👁 Read Only";
  if (role === "auditor")    return "📋 Auditor";
  if (typeof role === "object" && "custom" in role) return "🎛 Custom";
  return String(role);
}

function roleToString(role: RoleType): string {
  if (typeof role === "object" && "custom" in role) return "custom";
  return role;
}

function stringToRole(value: string): RoleType {
  if (value === "admin")      return "admin";
  if (value === "power_user") return "power_user";
  if (value === "read_only")  return "read_only";
  if (value === "auditor")    return "auditor";
  return "read_only";
}

function formatDate(iso: string | null): string {
  if (!iso) return "—";
  try {
    return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(iso));
  } catch {
    return iso;
  }
}

// ── Invite Modal ──────────────────────────────────────────────────────────────

interface InviteModalProps {
  onClose: () => void;
  onInvited: (member: TeamMember) => void;
}

function InviteModal({ onClose, onInvited }: Readonly<InviteModalProps>) {
  const { toast } = useToast();
  const [displayName, setDisplayName] = useState("");
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<string>("read_only");
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!displayName.trim()) {
      toast("warning", "Display name is required.");
      return;
    }
    setSubmitting(true);
    try {
      const member = await invoke<TeamMember>("rbac_add_member", {
        displayName: displayName.trim(),
        email: email.trim() || null,
        role: stringToRole(role),
      });
      toast("success", `${member.display_name} has been added to the team.`);
      onInvited(member);
      onClose();
    } catch (err) {
      toast("error", `Failed to add member: ${String(err)}`);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div
        className="bg-surface-elevated border border-border-default rounded-xl shadow-[var(--shadow-3)] w-full max-w-md mx-4"
        role="dialog"
        aria-modal="true"
        aria-labelledby="invite-modal-title"
      >
        {/* Header */}
        <div className="flex items-center gap-2.5 px-5 py-4 border-b border-border-subtle">
          <UserPlus size={16} className="text-accent-primary shrink-0" />
          <h2 id="invite-modal-title" className="text-sm font-semibold text-text-primary">
            Invite Team Member
          </h2>
        </div>

        {/* Form */}
        <form onSubmit={(e) => { void handleSubmit(e); }} className="px-5 py-4 flex flex-col gap-4">
          <div className="flex flex-col gap-1">
            <label className="text-xs text-text-secondary" htmlFor="invite-display-name">
              Display Name <span className="text-status-disconnected">*</span>
            </label>
            <input
              id="invite-display-name"
              type="text"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              placeholder="Jane Smith"
              required
              className="px-2.5 py-1.5 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors"
            />
          </div>

          <div className="flex flex-col gap-1">
            <label className="text-xs text-text-secondary" htmlFor="invite-email">
              Email (optional)
            </label>
            <input
              id="invite-email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="jane@example.com"
              className="px-2.5 py-1.5 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors"
            />
          </div>

          <div className="flex flex-col gap-1">
            <label className="text-xs text-text-secondary" htmlFor="invite-role">
              Role
            </label>
            <select
              id="invite-role"
              value={role}
              onChange={(e) => setRole(e.target.value)}
              className="px-2.5 py-1.5 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors"
            >
              {ROLE_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          </div>

          {/* Actions */}
          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="px-3 py-1.5 rounded-lg text-xs border border-border-default bg-surface-secondary hover:bg-surface-elevated text-text-secondary transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={submitting}
              className={clsx(
                "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors",
                submitting
                  ? "bg-interactive-default/50 text-text-disabled cursor-not-allowed"
                  : "bg-interactive-default hover:bg-interactive-hover text-white",
              )}
            >
              <UserPlus size={12} />
              {submitting ? "Adding…" : "Add Member"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Main component ────────────────────────────────────────────────────────────

export default function TeamPanel() {
  const { toast } = useToast();

  const [members, setMembers] = useState<TeamMember[]>([]);
  const [config, setConfig] = useState<TeamConfig>({
    members: [],
    require_mfa: false,
    session_timeout_minutes: 60,
    allowed_ips: [],
  });
  const [loading, setLoading] = useState(true);
  const [showInvite, setShowInvite] = useState(false);
  const [confirmRemoveId, setConfirmRemoveId] = useState<string | null>(null);
  const [savingConfig, setSavingConfig] = useState(false);

  // Allowed IPs as a single comma-separated string for the text input
  const [allowedIpsText, setAllowedIpsText] = useState("");

  // ── Load ──────────────────────────────────────────────────────────────────

  useEffect(() => {
    let mounted = true;
    async function load() {
      try {
        const [memberList, teamConfig] = await Promise.all([
          invoke<TeamMember[]>("rbac_list_members"),
          invoke<TeamConfig>("rbac_get_team_config"),
        ]);
        if (!mounted) return;
        setMembers(memberList);
        setConfig(teamConfig);
        setAllowedIpsText(teamConfig.allowed_ips.join(", "));
      } catch (err) {
        if (mounted) toast("error", `Failed to load team data: ${String(err)}`);
      } finally {
        if (mounted) setLoading(false);
      }
    }
    void load();
    return () => { mounted = false; };
  }, [toast]);

  // ── Handlers ──────────────────────────────────────────────────────────────

  const handleMemberInvited = useCallback((member: TeamMember) => {
    setMembers((prev) => [...prev, member]);
  }, []);

  const handleRoleChange = useCallback(async (memberId: string, roleStr: string) => {
    const role = stringToRole(roleStr);
    try {
      const updated = await invoke<TeamMember>("rbac_update_member_role", {
        memberId,
        role,
      });
      setMembers((prev) => prev.map((m) => (m.id === memberId ? updated : m)));
      toast("success", `Role updated to ${roleLabel(role)}.`);
    } catch (err) {
      toast("error", `Failed to update role: ${String(err)}`);
    }
  }, [toast]);

  const handleRemove = useCallback(async (memberId: string) => {
    try {
      await invoke("rbac_remove_member", { memberId });
      setMembers((prev) => prev.filter((m) => m.id !== memberId));
      toast("success", "Team member removed.");
    } catch (err) {
      toast("error", `Failed to remove member: ${String(err)}`);
    } finally {
      setConfirmRemoveId(null);
    }
  }, [toast]);

  const handleSaveConfig = useCallback(async () => {
    const ips = allowedIpsText
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean);
    const next: TeamConfig = { ...config, members, allowed_ips: ips };
    setSavingConfig(true);
    try {
      await invoke("rbac_update_team_config", { config: next });
      setConfig(next);
      toast("success", "Team settings saved.");
    } catch (err) {
      toast("error", `Failed to save settings: ${String(err)}`);
    } finally {
      setSavingConfig(false);
    }
  }, [config, members, allowedIpsText, toast]);

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div className="flex flex-col h-full bg-surface-primary">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-4 border-b border-border-subtle shrink-0">
        <div className="flex items-center gap-2">
          <Users size={16} className="text-accent-primary shrink-0" />
          <div>
            <h2 className="text-sm font-semibold text-text-primary">Team</h2>
            <p className="text-[11px] text-text-secondary mt-0.5">
              Manage members, roles, and access policies.
            </p>
          </div>
        </div>
        <button
          onClick={() => setShowInvite(true)}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-interactive-default hover:bg-interactive-hover text-white transition-colors"
        >
          <UserPlus size={13} />
          Invite Member
        </button>
      </div>

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto px-6 py-4 flex flex-col gap-6">
        {loading ? (
          <div className="text-xs text-text-secondary py-8 text-center">Loading team data…</div>
        ) : (
          <>
            {/* Members table */}
            <section>
              <p className="text-[10px] font-semibold uppercase tracking-wider text-text-disabled mb-2">
                Members
              </p>

              {members.length === 0 ? (
                <div className="flex flex-col items-center justify-center gap-2 py-10 border border-dashed border-border-default rounded-xl text-center">
                  <Users size={28} className="text-text-disabled" />
                  <p className="text-sm text-text-secondary">No team members yet.</p>
                  <p className="text-xs text-text-disabled">
                    Add your first team member to get started.
                  </p>
                  <button
                    onClick={() => setShowInvite(true)}
                    className="mt-1 flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs border border-border-default hover:bg-surface-secondary text-text-secondary transition-colors"
                  >
                    <UserPlus size={12} /> Invite Member
                  </button>
                </div>
              ) : (
                <div className="rounded-xl border border-border-default overflow-hidden">
                  <table className="w-full border-collapse">
                    <thead>
                      <tr className="bg-surface-secondary border-b border-border-default">
                        <th className="px-3 py-2 text-left text-[10px] font-semibold uppercase tracking-wider text-text-disabled">
                          Name
                        </th>
                        <th className="px-3 py-2 text-left text-[10px] font-semibold uppercase tracking-wider text-text-disabled">
                          Email
                        </th>
                        <th className="px-3 py-2 text-left text-[10px] font-semibold uppercase tracking-wider text-text-disabled">
                          Role
                        </th>
                        <th className="px-3 py-2 text-left text-[10px] font-semibold uppercase tracking-wider text-text-disabled">
                          Last Active
                        </th>
                        <th className="px-3 py-2 text-right text-[10px] font-semibold uppercase tracking-wider text-text-disabled">
                          Actions
                        </th>
                      </tr>
                    </thead>
                    <tbody>
                      {members.map((member, idx) => (
                        <tr
                          key={member.id}
                          className={clsx(
                            "border-b border-border-subtle last:border-0",
                            idx % 2 === 0 ? "bg-surface-primary" : "bg-surface-secondary/40",
                          )}
                        >
                          {/* Name */}
                          <td className="px-3 py-2.5">
                            <div className="flex items-center gap-2">
                              <Shield size={13} className="text-text-disabled shrink-0" />
                              <span className="text-xs text-text-primary font-medium">
                                {member.display_name}
                              </span>
                            </div>
                          </td>

                          {/* Email */}
                          <td className="px-3 py-2.5">
                            <span className="text-xs text-text-secondary">
                              {member.email ?? "—"}
                            </span>
                          </td>

                          {/* Role — inline change */}
                          <td className="px-3 py-2.5">
                            {typeof member.role === "object" && "custom" in member.role ? (
                              <span className="text-xs text-text-secondary">{roleLabel(member.role)}</span>
                            ) : (
                              <select
                                value={roleToString(member.role)}
                                onChange={(e) => { void handleRoleChange(member.id, e.target.value); }}
                                className="px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors"
                              >
                                {ROLE_OPTIONS.map((opt) => (
                                  <option key={opt.value} value={opt.value}>
                                    {opt.label}
                                  </option>
                                ))}
                              </select>
                            )}
                          </td>

                          {/* Last active */}
                          <td className="px-3 py-2.5">
                            <span className="text-xs text-text-secondary">
                              {formatDate(member.last_active)}
                            </span>
                          </td>

                          {/* Actions */}
                          <td className="px-3 py-2.5 text-right">
                            {confirmRemoveId === member.id ? (
                              <div className="flex items-center justify-end gap-1.5">
                                <span className="text-[11px] text-text-secondary">Remove?</span>
                                <button
                                  onClick={() => { void handleRemove(member.id); }}
                                  className="px-2 py-1 rounded text-[11px] bg-status-disconnected/20 hover:bg-status-disconnected/30 text-status-disconnected transition-colors"
                                >
                                  Yes
                                </button>
                                <button
                                  onClick={() => setConfirmRemoveId(null)}
                                  className="px-2 py-1 rounded text-[11px] border border-border-default hover:bg-surface-secondary text-text-secondary transition-colors"
                                >
                                  No
                                </button>
                              </div>
                            ) : (
                              <button
                                onClick={() => setConfirmRemoveId(member.id)}
                                className="inline-flex items-center gap-1 px-2 py-1 rounded-lg text-xs border border-border-default hover:bg-surface-secondary text-text-secondary hover:text-status-disconnected transition-colors"
                                title="Remove member"
                              >
                                <Trash2 size={12} />
                                Remove
                              </button>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </section>

            {/* Team Settings section */}
            <section>
              <div className="flex items-center gap-1.5 mb-2">
                <Settings2 size={13} className="text-text-disabled shrink-0" />
                <p className="text-[10px] font-semibold uppercase tracking-wider text-text-disabled">
                  Team Settings
                </p>
              </div>

              <div className="rounded-xl border border-border-default bg-surface-secondary/30 divide-y divide-border-subtle">
                {/* Require MFA */}
                <div className="flex items-start justify-between gap-4 px-4 py-3">
                  <div>
                    <p className="text-xs text-text-primary">Require MFA</p>
                    <p className="text-[10px] text-text-secondary mt-0.5">
                      Enforce multi-factor authentication for all team members.
                    </p>
                  </div>
                  <button
                    onClick={() => setConfig((c) => ({ ...c, require_mfa: !c.require_mfa }))}
                    className={clsx(
                      "relative shrink-0 w-9 h-5 rounded-full transition-colors duration-[var(--duration-short)]",
                      config.require_mfa
                        ? "bg-accent-primary"
                        : "bg-surface-secondary border border-border-default",
                    )}
                  >
                    <span
                      className={clsx(
                        "absolute top-0.5 w-4 h-4 rounded-full bg-white shadow-sm transition-transform duration-[var(--duration-short)]",
                        config.require_mfa ? "translate-x-4" : "translate-x-0.5",
                      )}
                    />
                  </button>
                </div>

                {/* Session timeout */}
                <div className="flex items-start justify-between gap-4 px-4 py-3">
                  <div>
                    <p className="text-xs text-text-primary">Session Timeout</p>
                    <p className="text-[10px] text-text-secondary mt-0.5">
                      Automatically disconnect inactive sessions (minutes). Set to 0 to disable.
                    </p>
                  </div>
                  <input
                    type="number"
                    value={config.session_timeout_minutes}
                    onChange={(e) => {
                      const n = Number.parseInt(e.target.value, 10);
                      if (!Number.isNaN(n) && n >= 0) {
                        setConfig((c) => ({ ...c, session_timeout_minutes: n }));
                      }
                    }}
                    min={0}
                    max={10080}
                    className="w-24 px-2 py-1 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus text-right transition-colors"
                  />
                </div>

                {/* Allowed IPs */}
                <div className="flex items-start justify-between gap-4 px-4 py-3">
                  <div className="flex-1 min-w-0">
                    <p className="text-xs text-text-primary">Allowed IP Ranges</p>
                    <p className="text-[10px] text-text-secondary mt-0.5">
                      Comma-separated CIDR ranges or IPs. Leave blank to allow all.
                    </p>
                    <input
                      type="text"
                      value={allowedIpsText}
                      onChange={(e) => setAllowedIpsText(e.target.value)}
                      placeholder="192.168.1.0/24, 10.0.0.1"
                      className="mt-1.5 w-full px-2 py-1.5 rounded-lg text-xs bg-surface-secondary border border-border-default text-text-primary outline-none focus:border-border-focus transition-colors"
                    />
                  </div>
                </div>
              </div>

              <div className="mt-3 flex justify-end">
                <button
                  onClick={() => { void handleSaveConfig(); }}
                  disabled={savingConfig}
                  className={clsx(
                    "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors",
                    savingConfig
                      ? "bg-interactive-default/50 text-text-disabled cursor-not-allowed"
                      : "bg-interactive-default hover:bg-interactive-hover text-white",
                  )}
                >
                  <Settings2 size={12} />
                  {savingConfig ? "Saving…" : "Save Settings"}
                </button>
              </div>
            </section>
          </>
        )}
      </div>

      {/* Invite modal */}
      {showInvite && (
        <InviteModal
          onClose={() => setShowInvite(false)}
          onInvited={handleMemberInvited}
        />
      )}
    </div>
  );
}
