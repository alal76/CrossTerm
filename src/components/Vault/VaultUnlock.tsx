import { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import { Lock, Eye, EyeOff, ShieldCheck, Loader2, Fingerprint, KeyRound, Vault as VaultIcon } from "lucide-react";
import { useVaultStore } from "@/stores/vaultStore";
import type { VaultInfo } from "@/types";

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

function VaultHeader({ isNewVault, t }: { readonly isNewVault: boolean; readonly t: (key: string) => string }) {
  return (
    <div className="flex flex-col items-center mb-8">
      <div className="w-16 h-16 rounded-2xl bg-accent-primary/10 flex items-center justify-center mb-4">
        {isNewVault ? (
          <ShieldCheck size={32} className="text-accent-primary" />
        ) : (
          <Lock size={32} className="text-accent-primary" />
        )}
      </div>
      <h1 className="text-lg font-semibold text-text-primary">
        {isNewVault ? t("vault.createVault") : t("vault.unlock")}
      </h1>
      <p className="text-xs text-text-secondary mt-1 text-center max-w-[260px]">
        {isNewVault
          ? t("vault.createDescription")
          : t("vault.unlockDescription")}
      </p>
    </div>
  );
}

function VaultListItem({
  vault,
  onSelect,
  selected,
}: {
  readonly vault: VaultInfo;
  readonly onSelect: (v: VaultInfo) => void;
  readonly selected: boolean;
}) {
  return (
    <button
      type="button"
      onClick={() => onSelect(vault)}
      className={clsx(
        "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left text-sm",
        "transition-colors duration-[var(--duration-short)]",
        "border",
        selected
          ? "border-border-focus bg-surface-elevated"
          : "border-border-default bg-surface-secondary hover:bg-surface-elevated"
      )}
    >
      <VaultIcon size={16} className="text-text-secondary shrink-0" />
      <div className="flex-1 min-w-0">
        <span className="text-text-primary truncate block">{vault.name}</span>
        {vault.is_default && (
          <span className="text-[10px] text-accent-primary">{vault.is_default ? "Default" : "Shared"}</span>
        )}
      </div>
      <Lock size={14} className="text-text-disabled shrink-0" />
    </button>
  );
}

export default function VaultUnlock() {
  const { t } = useTranslation();
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [checkingVault, setCheckingVault] = useState(true);
  const [biometricAvailable, setBiometricAvailable] = useState(false);
  const [fido2Available, setFido2Available] = useState(false);
  const [isMac, setIsMac] = useState(false);
  const [selectedVault, setSelectedVault] = useState<VaultInfo | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const vaults = useVaultStore((s) => s.vaults);
  const vaultLocked = useVaultStore((s) => s.vaultLocked);
  const vaultLockStates = useVaultStore((s) => s.vaultLockStates);
  const unlockVault = useVaultStore((s) => s.unlockVault);
  const createVault = useVaultStore((s) => s.createVault);
  const listVaults = useVaultStore((s) => s.listVaults);

  const isNewVault = !checkingVault && vaults.length === 0;
  const lockedVaults = vaults.filter((v) => vaultLockStates[v.id]);

  // Load vaults for the current profile
  useEffect(() => {
    async function check() {
      await listVaults();
      setCheckingVault(false);
    }
    check();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Auto-select the default vault when vaults load
  useEffect(() => {
    if (!selectedVault && lockedVaults.length > 0) {
      const defaultVault = lockedVaults.find((v) => v.is_default) ?? lockedVaults[0];
      setSelectedVault(defaultVault);
    }
  }, [lockedVaults, selectedVault]);

  // Detect biometric and FIDO2 availability
  useEffect(() => {
    detectAuthMethods(setBiometricAvailable, setFido2Available, setIsMac);
  }, []);

  useEffect(() => {
    if (!checkingVault) {
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [checkingVault]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);

    if (!password) {
      setError(t("vault.passwordRequired"));
      return;
    }

    if (isNewVault) {
      const validationError = validateNewVaultPassword(password, confirmPassword, t);
      if (validationError) {
        setError(validationError);
        return;
      }
    }

    setLoading(true);
    try {
      if (isNewVault) {
        await createVault(password);
      } else if (selectedVault) {
        await unlockVault(selectedVault.id, password);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function handleBiometricUnlock() {
    setError(null);
    setLoading(true);
    try {
      await invoke("vault_unlock_biometric");
      if (selectedVault) {
        await unlockVault(selectedVault.id, "");
      }
    } catch (e) {
      setError(String(e) || t("vault.biometricUnavailable"));
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
      <div className="flex items-center justify-center w-full h-full bg-surface-primary">
        <Loader2 size={24} className="animate-spin text-accent-primary" />
      </div>
    );
  }

  if (!vaultLocked) {
    return null;
  }

  return (
    <div className="flex items-center justify-center w-full h-full bg-surface-primary">
      <div className="w-full max-w-sm px-6">
        <VaultHeader isNewVault={isNewVault} t={t} />

        {/* Vault selector (when multiple locked vaults exist) */}
        {!isNewVault && lockedVaults.length > 1 && (
          <div className="flex flex-col gap-1.5 mb-4">
            <label className="text-xs text-text-secondary px-1">
              {t("vault.selectVault")}
            </label>
            {lockedVaults.map((v) => (
              <VaultListItem
                key={v.id}
                vault={v}
                onSelect={setSelectedVault}
                selected={selectedVault?.id === v.id}
              />
            ))}
          </div>
        )}

        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          <div className="relative">
            <input
              ref={inputRef}
              type={showPassword ? "text" : "password"}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={isNewVault ? t("vault.masterPasswordPlaceholder") : t("vault.passwordPlaceholder")}
              className={clsx(
                "w-full px-3 py-2.5 pr-10 rounded-lg text-sm bg-surface-secondary border outline-none",
                "text-text-primary placeholder:text-text-disabled",
                "transition-colors duration-[var(--duration-short)]",
                "focus:border-border-focus",
                error ? "border-status-disconnected" : "border-border-default"
              )}
              autoComplete="current-password"
            />
            <button
              type="button"
              onClick={() => setShowPassword(!showPassword)}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
              tabIndex={-1}
            >
              {showPassword ? <EyeOff size={16} /> : <Eye size={16} />}
            </button>
          </div>

          {isNewVault && (
            <input
              type={showPassword ? "text" : "password"}
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              placeholder={t("vault.confirmPasswordPlaceholder")}
              className={clsx(
                "w-full px-3 py-2.5 rounded-lg text-sm bg-surface-secondary border outline-none",
                "text-text-primary placeholder:text-text-disabled",
                "transition-colors duration-[var(--duration-short)]",
                "focus:border-border-focus",
                error && password !== confirmPassword
                  ? "border-status-disconnected"
                  : "border-border-default"
              )}
              autoComplete="new-password"
            />
          )}

          {error && (
            <p className="text-xs text-status-disconnected px-1">{error}</p>
          )}

          <button
            type="submit"
            disabled={loading || !password || (!isNewVault && !selectedVault)}
            className={clsx(
              "w-full py-2.5 rounded-lg text-sm font-medium",
              "transition-colors duration-[var(--duration-short)]",
              loading || !password
                ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                : "bg-interactive-default hover:bg-interactive-hover text-text-primary"
            )}
          >
            {loading && <Loader2 size={16} className="animate-spin mx-auto" />}
            {!loading && isNewVault && t("vault.createVault")}
            {!loading && !isNewVault && t("vault.unlock")}
          </button>

          {!isNewVault && (biometricAvailable || fido2Available) && (
            <div className="flex flex-col gap-2 mt-1">
              {biometricAvailable && (
                <button
                  type="button"
                  disabled={loading}
                  onClick={handleBiometricUnlock}
                  className={clsx(
                    "w-full py-2 rounded-lg text-sm font-medium flex items-center justify-center gap-2",
                    "transition-colors duration-[var(--duration-short)]",
                    "border border-border-default",
                    loading
                      ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                      : "bg-surface-secondary hover:bg-surface-tertiary text-text-primary"
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
                    "transition-colors duration-[var(--duration-short)]",
                    "border border-border-default",
                    loading
                      ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                      : "bg-surface-secondary hover:bg-surface-tertiary text-text-primary"
                  )}
                >
                  <KeyRound size={16} />
                  {t("vault.useSecurityKey")}
                </button>
              )}
            </div>
          )}
        </form>
      </div>
    </div>
  );
}
