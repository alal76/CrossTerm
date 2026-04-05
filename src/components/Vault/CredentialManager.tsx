import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import {
  Plus,
  Key,
  Lock,
  Cloud,
  FileKey2,
  Pencil,
  Trash2,
  X,
  Save,
  Eye,
  EyeOff,
  Loader2,
  Shield,
} from "lucide-react";
import { useVaultStore } from "@/stores/vaultStore";
import { CredentialType } from "@/types";
import FieldHelp from "@/components/Help/FieldHelp";

// ── Constants ──

const CREDENTIAL_TYPE_OPTIONS = [
  { value: CredentialType.Password, labelKey: "credentialTypes.password", icon: <Lock size={14} /> },
  { value: CredentialType.SSHKey, labelKey: "credentialTypes.ssh_key", icon: <FileKey2 size={14} /> },
  { value: CredentialType.APIToken, labelKey: "credentialTypes.api_token", icon: <Key size={14} /> },
  { value: CredentialType.CloudCredential, labelKey: "credentialTypes.cloud_credential", icon: <Cloud size={14} /> },
];

const TYPE_ICONS: Record<CredentialType, React.ReactNode> = {
  [CredentialType.Password]: <Lock size={14} className="text-accent-secondary" />,
  [CredentialType.SSHKey]: <FileKey2 size={14} className="text-accent-primary" />,
  [CredentialType.APIToken]: <Key size={14} className="text-status-connecting" />,
  [CredentialType.Certificate]: <Shield size={14} className="text-status-connected" />,
  [CredentialType.CloudCredential]: <Cloud size={14} className="text-text-link" />,
  [CredentialType.TOTPSeed]: <Key size={14} className="text-accent-primary" />,
};

// ── Credential Form Modal ──

function CredentialForm({
  credential,
  onClose,
}: {
  readonly credential?: { id: string; name: string; credential_type: string; username: string | null } | null;
  readonly onClose: () => void;
}) {
  const { t } = useTranslation();
  const addCredential = useVaultStore((s) => s.addCredential);
  const updateCredential = useVaultStore((s) => s.updateCredential);
  const loading = useVaultStore((s) => s.loading);

  const isEdit = !!credential;
  const [type, setType] = useState<CredentialType>(
    (credential?.credential_type as CredentialType) ?? CredentialType.Password
  );
  const [name, setName] = useState(credential?.name ?? "");
  const [username, setUsername] = useState(credential?.username ?? "");
  const [password, setPassword] = useState("");
  const [privateKey, setPrivateKey] = useState("");
  const [passphrase, setPassphrase] = useState("");
  const [token, setToken] = useState("");
  const [provider, setProvider] = useState("");
  const [accessKey, setAccessKey] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [region, setRegion] = useState("");
  const [showSecret, setShowSecret] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const nameRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    requestAnimationFrame(() => nameRef.current?.focus());
  }, []);

  async function handleSave() {
    if (!name.trim()) {
      setError(t("credentialForm.nameRequired"));
      return;
    }
    setError(null);

    let data: unknown;

    switch (type) {
      case CredentialType.Password:
        data = { password };
        break;
      case CredentialType.SSHKey:
        data = { private_key: privateKey, passphrase: passphrase || undefined };
        break;
      case CredentialType.APIToken:
        data = { provider, token };
        break;
      case CredentialType.CloudCredential:
        data = {
          provider: provider as "aws" | "azure" | "gcp",
          access_key: accessKey,
          secret_key: secretKey,
          region: region || undefined,
        };
        break;
      default:
        return;
    }

    try {
      if (isEdit && credential) {
        await updateCredential(credential.id, {
          name: name.trim(),
          username: username || undefined,
          data,
        });
      } else {
        await addCredential({
          name: name.trim(),
          credential_type: type,
          username: username || undefined,
          data,
        });
      }
      onClose();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="fixed inset-0 z-[8000] flex items-center justify-center" role="dialog" aria-modal="true">
      <div
        className="absolute inset-0 bg-surface-overlay/60 backdrop-blur-sm"
        onClick={onClose}
        onKeyDown={(e) => e.key === "Escape" && onClose()}
        role="presentation"
      />
      <div
        className="relative w-full max-w-md max-h-[85vh] bg-surface-elevated border border-border-default rounded-xl shadow-[var(--shadow-3)] flex flex-col overflow-hidden"
        style={{ animation: "paletteIn var(--duration-medium) var(--ease-decelerate)" }}
      >
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-border-subtle shrink-0">
          <h2 className="text-sm font-semibold text-text-primary">
            {isEdit ? t("credentialForm.editTitle") : t("credentialForm.newTitle")}
          </h2>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
          >
            <X size={16} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-3.5">
          {/* Type */}
          {!isEdit && (
            <div>
              <label className="flex items-center text-[11px] text-text-secondary mb-1.5">
                {t("credentialForm.typeLabel")}
                <FieldHelp description={t("fieldHelp.credentialType")} articleSlug="credential-vault" />
              </label>
              <div className="grid grid-cols-2 gap-1.5">
                {CREDENTIAL_TYPE_OPTIONS.map((opt) => (
                  <button
                    key={opt.value}
                    onClick={() => setType(opt.value)}
                    className={clsx(
                      "flex items-center gap-2 px-3 py-2 rounded-lg border text-xs",
                      "transition-colors duration-[var(--duration-micro)]",
                      type === opt.value
                        ? "border-border-focus bg-interactive-default/10 text-text-primary"
                        : "border-border-default hover:bg-surface-secondary text-text-secondary"
                    )}
                  >
                    {opt.icon}
                    {t(opt.labelKey)}
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Name */}
          <div>
            <label className="block text-[11px] text-text-secondary mb-1">{t("credentialForm.nameLabel")}</label>
            <input
              ref={nameRef}
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("credentialForm.namePlaceholder")}
              className={fieldClass}
            />
          </div>

          {/* Type-specific fields */}
          {type === CredentialType.Password && (
            <>
              <div>
                <label className="block text-[11px] text-text-secondary mb-1">{t("credentialForm.usernameLabel")}</label>
                <input
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder={t("credentialForm.usernamePlaceholder")}
                  className={fieldClass}
                />
              </div>
              <div>
                <label className="block text-[11px] text-text-secondary mb-1">{t("vault.password")}</label>
                <div className="relative">
                  <input
                    type={showSecret ? "text" : "password"}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    className={clsx(fieldClass, "pr-8")}
                  />
                  <button
                    type="button"
                    onClick={() => setShowSecret(!showSecret)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-text-secondary hover:text-text-primary"
                    tabIndex={-1}
                  >
                    {showSecret ? <EyeOff size={14} /> : <Eye size={14} />}
                  </button>
                </div>
              </div>
            </>
          )}

          {type === CredentialType.SSHKey && (
            <>
              <div>
                <label className="block text-[11px] text-text-secondary mb-1">{t("credentialForm.privateKeyLabel")}</label>
                <textarea
                  value={privateKey}
                  onChange={(e) => setPrivateKey(e.target.value)}
                  placeholder={t("credentialForm.privateKeyPlaceholder")}
                  rows={4}
                  className={clsx(fieldClass, "resize-none font-mono text-[10px]")}
                />
              </div>
              <div>
                <label className="block text-[11px] text-text-secondary mb-1">
                  {t("credentialForm.passphraseLabel")} <span className="text-text-disabled">({t("credentialForm.optional")})</span>
                </label>
                <input
                  type={showSecret ? "text" : "password"}
                  value={passphrase}
                  onChange={(e) => setPassphrase(e.target.value)}
                  className={fieldClass}
                />
              </div>
            </>
          )}

          {type === CredentialType.APIToken && (
            <>
              <div>
                <label className="block text-[11px] text-text-secondary mb-1">{t("credentialForm.providerLabel")}</label>
                <input
                  value={provider}
                  onChange={(e) => setProvider(e.target.value)}
                  placeholder={t("credentialForm.providerPlaceholder")}
                  className={fieldClass}
                />
              </div>
              <div>
                <label className="block text-[11px] text-text-secondary mb-1">{t("credentialForm.tokenLabel")}</label>
                <div className="relative">
                  <input
                    type={showSecret ? "text" : "password"}
                    value={token}
                    onChange={(e) => setToken(e.target.value)}
                    className={clsx(fieldClass, "pr-8 font-mono")}
                  />
                  <button
                    type="button"
                    onClick={() => setShowSecret(!showSecret)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-text-secondary hover:text-text-primary"
                    tabIndex={-1}
                  >
                    {showSecret ? <EyeOff size={14} /> : <Eye size={14} />}
                  </button>
                </div>
              </div>
            </>
          )}

          {type === CredentialType.CloudCredential && (
            <>
              <div>
                <label className="block text-[11px] text-text-secondary mb-1">{t("credentialForm.cloudProviderLabel")}</label>
                <select
                  value={provider}
                  onChange={(e) => setProvider(e.target.value)}
                  className={fieldClass}
                >
                  <option value="">{t("credentialForm.selectProvider")}</option>
                  <option value="aws">AWS</option>
                  <option value="azure">Azure</option>
                  <option value="gcp">GCP</option>
                </select>
              </div>
              <div>
                <label className="block text-[11px] text-text-secondary mb-1">{t("credentialForm.accessKeyLabel")}</label>
                <input
                  value={accessKey}
                  onChange={(e) => setAccessKey(e.target.value)}
                  className={clsx(fieldClass, "font-mono")}
                />
              </div>
              <div>
                <label className="block text-[11px] text-text-secondary mb-1">{t("credentialForm.secretKeyLabel")}</label>
                <div className="relative">
                  <input
                    type={showSecret ? "text" : "password"}
                    value={secretKey}
                    onChange={(e) => setSecretKey(e.target.value)}
                    className={clsx(fieldClass, "pr-8 font-mono")}
                  />
                  <button
                    type="button"
                    onClick={() => setShowSecret(!showSecret)}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-text-secondary hover:text-text-primary"
                    tabIndex={-1}
                  >
                    {showSecret ? <EyeOff size={14} /> : <Eye size={14} />}
                  </button>
                </div>
              </div>
              <div>
                <label className="block text-[11px] text-text-secondary mb-1">
                  {t("credentialForm.regionLabel")} <span className="text-text-disabled">({t("credentialForm.optional")})</span>
                </label>
                <input
                  value={region}
                  onChange={(e) => setRegion(e.target.value)}
                  placeholder={t("credentialForm.regionPlaceholder")}
                  className={fieldClass}
                />
              </div>
            </>
          )}

          {error && <p className="text-xs text-status-disconnected">{error}</p>}
        </div>

        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border-subtle shrink-0">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            {t("actions.cancel")}
          </button>
          <button
            onClick={handleSave}
            disabled={loading}
            className={clsx(
              "flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg transition-colors duration-[var(--duration-short)]",
              loading
                ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                : "bg-interactive-default hover:bg-interactive-hover text-text-primary"
            )}
          >
            {loading ? <Loader2 size={13} className="animate-spin" /> : <Save size={13} />}
            {isEdit ? t("actions.save") : t("actions.create")}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Main Component ──

export default function CredentialManager() {
  const { t } = useTranslation();
  const credentials = useVaultStore((s) => s.credentials);
  const deleteCredential = useVaultStore((s) => s.deleteCredential);
  const loading = useVaultStore((s) => s.loading);
  const fetchCredentials = useVaultStore((s) => s.fetchCredentials);

  const [formOpen, setFormOpen] = useState(false);
  const [editingCredential, setEditingCredential] = useState<{
    id: string;
    name: string;
    credential_type: string;
    username: string | null;
  } | null>(null);

  useEffect(() => {
    fetchCredentials();
  }, [fetchCredentials]);

  function getSubtitle(cred: { credential_type: string; username: string | null }): string {
    if (cred.username) return cred.username;
    switch (cred.credential_type) {
      case CredentialType.Password:
        return t("credentialTypes.password");
      case CredentialType.SSHKey:
        return t("credentialTypes.ssh_key");
      case CredentialType.APIToken:
        return t("credentialTypes.api_token");
      case CredentialType.CloudCredential:
        return t("credentialTypes.cloud_credential");
      case CredentialType.Certificate:
        return t("credentialTypes.certificate");
      case CredentialType.TOTPSeed:
        return t("credentialTypes.totp_seed");
      default:
        return cred.credential_type;
    }
  }

  return (
    <div className="flex flex-col h-full" data-help-article="credential-vault">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-border-subtle shrink-0">
        <h2 className="text-sm font-semibold text-text-primary">{t("vault.credentials")}</h2>
        {credentials.length > 0 && (
          <span className="text-[10px] text-text-disabled">
            {t("counts.credentials", { count: credentials.length })}
          </span>
        )}
        <div className="flex-1" />
        <button
          onClick={() => {
            setEditingCredential(null);
            setFormOpen(true);
          }}
          className="flex items-center gap-1 px-2.5 py-1 text-xs rounded-lg bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors duration-[var(--duration-short)]"
        >
          <Plus size={13} />
          {t("vault.add")}
        </button>
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto">
        {loading && credentials.length === 0 ? (
          <div className="flex items-center justify-center h-32">
            <Loader2 size={20} className="animate-spin text-accent-primary" />
          </div>
        ) : credentials.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-center px-6">
            <div className="w-14 h-14 rounded-2xl bg-surface-elevated flex items-center justify-center mb-3">
              <Key size={24} className="text-text-disabled" />
            </div>
            <p className="text-sm text-text-primary mb-1">{t("vault.noCredentials")}</p>
            <p className="text-xs text-text-secondary mb-4">
              {t("vault.noCredentialsHint")}
            </p>
            <button
              onClick={() => setFormOpen(true)}
              className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors duration-[var(--duration-short)]"
            >
              <Plus size={13} />
              {t("vault.addCredential")}
            </button>
          </div>
        ) : (
          <div className="divide-y divide-border-subtle">
            {credentials.map((cred) => (
              <div
                key={cred.id}
                className="flex items-center gap-3 px-4 py-2.5 hover:bg-surface-elevated/50 transition-colors duration-[var(--duration-micro)] group"
              >
                <div className="shrink-0 w-8 h-8 rounded-lg bg-surface-secondary flex items-center justify-center">
                  {TYPE_ICONS[cred.credential_type as CredentialType]}
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-xs text-text-primary truncate">{cred.name}</p>
                  <p className="text-[10px] text-text-secondary truncate">{getSubtitle(cred)}</p>
                </div>
                <div className="flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity duration-[var(--duration-micro)]">
                  <button
                    onClick={() => {
                      setEditingCredential(cred);
                      setFormOpen(true);
                    }}
                    className="p-1 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
                    title={t("sessions.edit")}
                  >
                    <Pencil size={13} />
                  </button>
                  <button
                    onClick={() => deleteCredential(cred.id)}
                    className="p-1 rounded hover:bg-status-disconnected/10 text-text-secondary hover:text-status-disconnected transition-colors duration-[var(--duration-micro)]"
                    title={t("sessions.delete")}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Modal */}
      {formOpen && (
        <CredentialForm
          credential={editingCredential}
          onClose={() => {
            setFormOpen(false);
            setEditingCredential(null);
          }}
        />
      )}
    </div>
  );
}

// ── Shared ──

const fieldClass = clsx(
  "w-full px-2.5 py-2 rounded-lg text-xs bg-surface-secondary border border-border-default outline-none",
  "text-text-primary placeholder:text-text-disabled",
  "transition-colors duration-[var(--duration-short)]",
  "focus:border-border-focus"
);
