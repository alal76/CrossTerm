import { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Lock, Eye, EyeOff, ShieldCheck, Loader2, Fingerprint, KeyRound,
  Vault as VaultIcon, Plus, Trash2, ArrowLeft,
} from "lucide-react";
import { useVaultStore } from "@/stores/vaultStore";
import type { VaultInfo } from "@/types";

type Mode = "unlock" | "create" | "delete";

function validateNewVaultPassword(
  password: string,
  confirmPassword: string,
  t: (key: string) => string,
): string | null {
  if (password.length < 8) return t("vault.passwordTooShort");
  if (password !== confirmPassword) return t("vault.passwordMismatch");
  return null;
}

async function detectAuthMethods(
  setBiometricAvailable: (v: boolean) => void,
  setFido2Available: (v: boolean) => void,
  setIsMac: (v: boolean) => void,
) {
  try {
    const bio = await invoke<boolean>("vault_biometric_available");
    setBiometricAvailable(bio);
  } catch {
    setBiometricAvailable(false);
  }
  try {
    const fido = await invoke<boolean>("vault_fido2_available");
    setFido2Available(fido);
  } catch {
    setFido2Available(false);
  }
  setIsMac(navigator.userAgent.toUpperCase().includes("MAC"));
}

function VaultHeader({
  mode,
  vaultName,
  t,
}: {
  readonly mode: Mode;
  readonly vaultName?: string;
  readonly t: (key: string) => string;
}) {
  let icon: React.ReactNode = <Lock size={32} className="text-accent-primary" />;
  if (mode === "create") icon = <ShieldCheck size={32} className="text-accent-primary" />;
  if (mode === "delete") icon = <Trash2 size={32} className="text-status-disconnected" />;

  let title = t("vault.unlock");
  if (mode === "create") title = t("vault.createVault");
  if (mode === "delete") title = t("vault.deleteVaultTitle");

  let description = t("vault.unlockDescription");
  if (mode === "create") description = t("vault.createDescription");
  if (mode === "delete") {
    const hint = t("vault.deleteVaultPasswordHint");
    description = vaultName ? `${hint}: "${vaultName}"` : hint;
  }

  return (
    <div className="flex flex-col items-center mb-6">
      <div className={clsx(
        "w-16 h-16 rounded-2xl flex items-center justify-center mb-4",
        mode === "delete" ? "bg-status-disconnected/10" : "bg-accent-primary/10",
      )}>
        {icon}
      </div>
      <h1 className="text-lg font-semibold text-text-primary">{title}</h1>
      <p className="text-xs text-text-secondary mt-1 text-center max-w-[280px]">{description}</p>
    </div>
  );
}

function VaultListItem({
  vault,
  onSelect,
  selected,
  onDelete,
}: {
  readonly vault: VaultInfo;
  readonly onSelect: (v: VaultInfo) => void;
  readonly selected: boolean;
  readonly onDelete: (v: VaultInfo) => void;
}) {
  return (
    <div className={clsx(
      "w-full flex items-center gap-2 px-3 py-2.5 rounded-lg",
      "border transition-colors duration-[var(--duration-short)]",
      selected ? "border-border-focus bg-surface-elevated" : "border-border-default bg-surface-secondary",
    )}>
      <button
        type="button"
        onClick={() => onSelect(vault)}
        className="flex items-center gap-3 flex-1 min-w-0 text-left"
      >
        <VaultIcon size={16} className="text-text-secondary shrink-0" />
        <div className="flex-1 min-w-0">
          <span className="text-sm text-text-primary truncate block">{vault.name}</span>
          {vault.is_default && (
            <span className="text-[10px] text-accent-primary">Default</span>
          )}
        </div>
        <Lock size={14} className="text-text-disabled shrink-0" />
      </button>
      <button
        type="button"
        onClick={() => onDelete(vault)}
        title="Delete vault"
        className="shrink-0 p-1 rounded text-text-disabled hover:text-status-disconnected hover:bg-status-disconnected/10 transition-colors"
      >
        <Trash2 size={14} />
      </button>
    </div>
  );
}

function PasswordInput({
  value,
  onChange,
  placeholder,
  show,
  onToggleShow,
  hasError,
  inputRef,
  autoComplete = "current-password",
}: {
  readonly value: string;
  readonly onChange: (v: string) => void;
  readonly placeholder: string;
  readonly show: boolean;
  readonly onToggleShow: () => void;
  readonly hasError: boolean;
  readonly inputRef?: React.RefObject<HTMLInputElement>;
  readonly autoComplete?: string;
}) {
  return (
    <div className="relative">
      <input
        ref={inputRef}
        type={show ? "text" : "password"}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        autoComplete={autoComplete}
        className={clsx(
          "w-full px-3 py-2.5 pr-10 rounded-lg text-sm bg-surface-secondary border outline-none",
          "text-text-primary placeholder:text-text-disabled",
          "transition-colors duration-[var(--duration-short)] focus:border-border-focus",
          hasError ? "border-status-disconnected" : "border-border-default",
        )}
      />
      <button
        type="button"
        tabIndex={-1}
        onClick={onToggleShow}
        className="absolute right-2.5 top-1/2 -translate-y-1/2 text-text-secondary hover:text-text-primary transition-colors"
      >
        {show ? <EyeOff size={16} /> : <Eye size={16} />}
      </button>
    </div>
  );
}

function resolveHeaderMode(showDelete: boolean, showCreate: boolean): Mode {
  if (showDelete) return "delete";
  if (showCreate) return "create";
  return "unlock";
}

export default function VaultUnlock() {
  const { t } = useTranslation();

  // Shared state
  const [mode, setMode] = useState<Mode>("unlock");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [checkingVault, setCheckingVault] = useState(true);
  const [biometricAvailable, setBiometricAvailable] = useState(false);
  const [fido2Available, setFido2Available] = useState(false);
  const [isMac, setIsMac] = useState(false);
  const [selectedVault, setSelectedVault] = useState<VaultInfo | null>(null);
  const [showPassword, setShowPassword] = useState(false);

  // Unlock mode
  const [password, setPassword] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  // Create mode
  const [newVaultName, setNewVaultName] = useState("");
  const [createPassword, setCreatePassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [showCreatePassword, setShowCreatePassword] = useState(false);
  const createInputRef = useRef<HTMLInputElement>(null);

  // Delete mode
  const [deleteTarget, setDeleteTarget] = useState<VaultInfo | null>(null);
  const [deletePassword, setDeletePassword] = useState("");
  const [showDeletePassword, setShowDeletePassword] = useState(false);
  const deleteInputRef = useRef<HTMLInputElement>(null);

  const vaults = useVaultStore((s) => s.vaults);
  const vaultLocked = useVaultStore((s) => s.vaultLocked);
  const vaultLockStates = useVaultStore((s) => s.vaultLockStates);
  const unlockVault = useVaultStore((s) => s.unlockVault);
  const createVault = useVaultStore((s) => s.createVault);
  const deleteVault = useVaultStore((s) => s.deleteVault);
  const listVaults = useVaultStore((s) => s.listVaults);

  const isFirstVault = !checkingVault && vaults.length === 0;
  const lockedVaults = vaults.filter((v) => vaultLockStates[v.id]);

  useEffect(() => {
    async function check() {
      await listVaults();
      setCheckingVault(false);
    }
    check();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!selectedVault && lockedVaults.length > 0) {
      const def = lockedVaults.find((v) => v.is_default) ?? lockedVaults[0];
      setSelectedVault(def);
    }
  }, [lockedVaults, selectedVault]);

  useEffect(() => {
    detectAuthMethods(setBiometricAvailable, setFido2Available, setIsMac);
  }, []);

  // Auto-focus the relevant input when mode changes
  useEffect(() => {
    if (checkingVault) return;
    let target = inputRef;
    if (mode === "create") target = createInputRef;
    if (mode === "delete") target = deleteInputRef;
    requestAnimationFrame(() => target.current?.focus());
  }, [checkingVault, mode]);

  function resetAndBack() {
    setMode("unlock");
    setError(null);
    setNewVaultName("");
    setCreatePassword("");
    setConfirmPassword("");
    setDeletePassword("");
    setDeleteTarget(null);
  }

  function handleStartDelete(vault: VaultInfo) {
    setDeleteTarget(vault);
    setDeletePassword("");
    setError(null);
    setMode("delete");
  }

  async function handleUnlock(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (!password) { setError(t("vault.passwordRequired")); return; }
    if (!selectedVault) return;
    setLoading(true);
    try {
      await unlockVault(selectedVault.id, password);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const validationError = validateNewVaultPassword(createPassword, confirmPassword, t);
    if (validationError) { setError(validationError); return; }
    setLoading(true);
    try {
      const name = newVaultName.trim() || "My Vault";
      await createVault(createPassword, name, isFirstVault);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function handleDelete(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (!deletePassword) { setError(t("vault.passwordRequired")); return; }
    if (!deleteTarget) return;
    setLoading(true);
    try {
      await deleteVault(deleteTarget.id, deletePassword);
      await listVaults();
      resetAndBack();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  async function handleBiometricUnlock() {
    setError(null);
    setLoading(true);
    try {
      await invoke("vault_unlock_biometric");
      if (selectedVault) await unlockVault(selectedVault.id, "");
    } catch {
      setError(t("vault.biometricUnavailable"));
    } finally {
      setLoading(false);
    }
  }

  async function handleFido2Unlock() {
    setError(null);
    setLoading(true);
    try {
      await invoke("vault_fido2_auth_begin", { vaultId: selectedVault?.id ?? "" });
      setError(t("vault.fido2Unavailable"));
    } catch {
      setError(t("vault.fido2Unavailable"));
    } finally {
      setLoading(false);
    }
  }

  if (checkingVault) {
    return (
      <div className="fixed inset-0 flex items-center justify-center bg-surface-primary z-50">
        <Loader2 size={24} className="animate-spin text-accent-primary" />
      </div>
    );
  }

  if (!vaultLocked && !isFirstVault) return null;

  const showUnlockMode = mode === "unlock" && !isFirstVault;
  const showCreateMode = mode === "create" || isFirstVault;
  const showDeleteMode = mode === "delete";
  const activeMode = resolveHeaderMode(showDeleteMode, showCreateMode);

  return (
    <div className="fixed inset-0 flex items-center justify-center bg-surface-primary z-50">
      <div className="w-full max-w-sm px-6">

        {/* Back button (create/delete modes when there are existing vaults) */}
        {(mode === "create" || mode === "delete") && !isFirstVault && (
          <button
            type="button"
            onClick={resetAndBack}
            className="flex items-center gap-1.5 text-xs text-text-secondary hover:text-text-primary mb-5 transition-colors"
          >
            <ArrowLeft size={14} />
            {t("vault.backToUnlock")}
          </button>
        )}

        <VaultHeader
          mode={activeMode}
          vaultName={deleteTarget?.name}
          t={t}
        />

        {/* ── UNLOCK MODE ── */}
        {showUnlockMode && (
          <>
            {lockedVaults.length > 1 && (
              <div className="flex flex-col gap-1.5 mb-4">
                <label className="text-xs text-text-secondary px-1">{t("vault.selectVault")}</label>
                {lockedVaults.map((v) => (
                  <VaultListItem
                    key={v.id}
                    vault={v}
                    onSelect={setSelectedVault}
                    selected={selectedVault?.id === v.id}
                    onDelete={handleStartDelete}
                  />
                ))}
              </div>
            )}

            {lockedVaults.length === 1 && (
              <div className="flex items-center justify-between px-1 mb-3">
                <span className="text-xs text-text-secondary truncate">{lockedVaults[0].name}</span>
                <button
                  type="button"
                  onClick={() => handleStartDelete(lockedVaults[0])}
                  title={t("vault.deleteVault")}
                  className="p-1 rounded text-text-disabled hover:text-status-disconnected hover:bg-status-disconnected/10 transition-colors"
                >
                  <Trash2 size={13} />
                </button>
              </div>
            )}

            <form onSubmit={handleUnlock} className="flex flex-col gap-3">
              <PasswordInput
                inputRef={inputRef}
                value={password}
                onChange={setPassword}
                placeholder={t("vault.passwordPlaceholder")}
                show={showPassword}
                onToggleShow={() => setShowPassword(!showPassword)}
                hasError={!!error}
              />
              {error && <p className="text-xs text-status-disconnected px-1">{error}</p>}
              <button
                type="submit"
                disabled={loading || !password || !selectedVault}
                className={clsx(
                  "w-full py-2.5 rounded-lg text-sm font-medium transition-colors duration-[var(--duration-short)]",
                  loading || !password
                    ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                    : "bg-interactive-default hover:bg-interactive-hover text-text-primary",
                )}
              >
                {loading ? <Loader2 size={16} className="animate-spin mx-auto" /> : t("vault.unlock")}
              </button>

              {(biometricAvailable || fido2Available) && (
                <div className="flex flex-col gap-2 mt-1">
                  {biometricAvailable && (
                    <button
                      type="button"
                      disabled={loading}
                      onClick={handleBiometricUnlock}
                      className={clsx(
                        "w-full py-2 rounded-lg text-sm font-medium flex items-center justify-center gap-2",
                        "border border-border-default transition-colors duration-[var(--duration-short)]",
                        loading ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                                : "bg-surface-secondary hover:bg-surface-tertiary text-text-primary",
                      )}
                    >
                      <Fingerprint size={16} />
                      {isMac ? t("vault.useBiometric") : t("vault.useWindowsHello")}
                    </button>
                  )}
                  {fido2Available && (
                    <button
                      type="button"
                      disabled={loading}
                      onClick={handleFido2Unlock}
                      className={clsx(
                        "w-full py-2 rounded-lg text-sm font-medium flex items-center justify-center gap-2",
                        "border border-border-default transition-colors duration-[var(--duration-short)]",
                        loading ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                                : "bg-surface-secondary hover:bg-surface-tertiary text-text-primary",
                      )}
                    >
                      <KeyRound size={16} />
                      {t("vault.useSecurityKey")}
                    </button>
                  )}
                </div>
              )}
            </form>

            {/* Add new vault link */}
            <div className="mt-5 flex justify-center">
              <button
                type="button"
                onClick={() => { setMode("create"); setError(null); }}
                className="flex items-center gap-1.5 text-xs text-text-secondary hover:text-accent-primary transition-colors"
              >
                <Plus size={13} />
                {t("vault.addNewVault")}
              </button>
            </div>
          </>
        )}

        {/* ── CREATE MODE ── */}
        {showCreateMode && (
          <form onSubmit={handleCreate} className="flex flex-col gap-3">
            <input
              ref={createInputRef}
              type="text"
              value={newVaultName}
              onChange={(e) => setNewVaultName(e.target.value)}
              placeholder={t("vault.newVaultNamePlaceholder")}
              maxLength={40}
              className="w-full px-3 py-2.5 rounded-lg text-sm bg-surface-secondary border border-border-default outline-none text-text-primary placeholder:text-text-disabled focus:border-border-focus transition-colors"
            />
            <PasswordInput
              value={createPassword}
              onChange={setCreatePassword}
              placeholder={t("vault.masterPasswordPlaceholder")}
              show={showCreatePassword}
              onToggleShow={() => setShowCreatePassword(!showCreatePassword)}
              hasError={!!error}
              autoComplete="new-password"
            />
            <PasswordInput
              value={confirmPassword}
              onChange={setConfirmPassword}
              placeholder={t("vault.confirmPasswordPlaceholder")}
              show={showCreatePassword}
              onToggleShow={() => setShowCreatePassword(!showCreatePassword)}
              hasError={!!error && createPassword !== confirmPassword}
              autoComplete="new-password"
            />
            {error && <p className="text-xs text-status-disconnected px-1">{error}</p>}
            <button
              type="submit"
              disabled={loading || !createPassword}
              className={clsx(
                "w-full py-2.5 rounded-lg text-sm font-medium transition-colors duration-[var(--duration-short)]",
                loading || !createPassword
                  ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                  : "bg-interactive-default hover:bg-interactive-hover text-text-primary",
              )}
            >
              {loading ? <Loader2 size={16} className="animate-spin mx-auto" /> : t("vault.createVault")}
            </button>
          </form>
        )}

        {/* ── DELETE MODE ── */}
        {showDeleteMode && (
          <form onSubmit={handleDelete} className="flex flex-col gap-3">
            <PasswordInput
              inputRef={deleteInputRef}
              value={deletePassword}
              onChange={setDeletePassword}
              placeholder={t("vault.passwordPlaceholder")}
              show={showDeletePassword}
              onToggleShow={() => setShowDeletePassword(!showDeletePassword)}
              hasError={!!error}
            />
            {error && <p className="text-xs text-status-disconnected px-1">{error}</p>}
            <button
              type="submit"
              disabled={loading || !deletePassword}
              className={clsx(
                "w-full py-2.5 rounded-lg text-sm font-medium transition-colors duration-[var(--duration-short)]",
                loading || !deletePassword
                  ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                  : "bg-status-disconnected hover:bg-status-disconnected/80 text-white",
              )}
            >
              {loading ? <Loader2 size={16} className="animate-spin mx-auto" /> : t("vault.deleteVault")}
            </button>
            <button
              type="button"
              onClick={resetAndBack}
              className="w-full py-2 text-xs text-text-secondary hover:text-text-primary transition-colors"
            >
              {t("vault.cancelAction")}
            </button>
          </form>
        )}
      </div>
    </div>
  );
}
