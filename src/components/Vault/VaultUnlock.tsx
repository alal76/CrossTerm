import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import { Lock, Eye, EyeOff, ShieldCheck, Loader2 } from "lucide-react";
import { useVaultStore } from "@/stores/vaultStore";

export default function VaultUnlock() {
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [isNewVault, setIsNewVault] = useState<boolean | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [checkingVault, setCheckingVault] = useState(true);
  const inputRef = useRef<HTMLInputElement>(null);

  const unlockVault = useVaultStore((s) => s.unlockVault);

  // Detect if vault exists
  useEffect(() => {
    async function check() {
      try {
        await invoke<boolean>("vault_is_locked");
        // If the command succeeds, a vault exists
        setIsNewVault(false);
      } catch {
        // Vault doesn't exist yet
        setIsNewVault(true);
      } finally {
        setCheckingVault(false);
      }
    }
    check();
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
      setError("Password is required");
      return;
    }

    if (isNewVault) {
      if (password.length < 8) {
        setError("Password must be at least 8 characters");
        return;
      }
      if (password !== confirmPassword) {
        setError("Passwords do not match");
        return;
      }
      setLoading(true);
      try {
        await invoke("vault_create", { password });
        await unlockVault(password);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    } else {
      setLoading(true);
      try {
        await unlockVault(password);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    }
  }

  if (checkingVault) {
    return (
      <div className="flex items-center justify-center w-full h-full bg-surface-primary">
        <Loader2 size={24} className="animate-spin text-accent-primary" />
      </div>
    );
  }

  return (
    <div className="flex items-center justify-center w-full h-full bg-surface-primary">
      <div className="w-full max-w-sm px-6">
        <div className="flex flex-col items-center mb-8">
          <div className="w-16 h-16 rounded-2xl bg-accent-primary/10 flex items-center justify-center mb-4">
            {isNewVault ? (
              <ShieldCheck size={32} className="text-accent-primary" />
            ) : (
              <Lock size={32} className="text-accent-primary" />
            )}
          </div>
          <h1 className="text-lg font-semibold text-text-primary">
            {isNewVault ? "Create Vault" : "Unlock Vault"}
          </h1>
          <p className="text-xs text-text-secondary mt-1 text-center max-w-[260px]">
            {isNewVault
              ? "Set a master password to encrypt your credentials."
              : "Enter your master password to access saved credentials."}
          </p>
        </div>

        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          <div className="relative">
            <input
              ref={inputRef}
              type={showPassword ? "text" : "password"}
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={isNewVault ? "Master password" : "Password"}
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
              placeholder="Confirm password"
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
            disabled={loading || !password}
            className={clsx(
              "w-full py-2.5 rounded-lg text-sm font-medium",
              "transition-colors duration-[var(--duration-short)]",
              loading || !password
                ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                : "bg-interactive-default hover:bg-interactive-hover text-text-primary"
            )}
          >
            {loading && <Loader2 size={16} className="animate-spin mx-auto" />}
            {!loading && isNewVault && "Create Vault"}
            {!loading && !isNewVault && "Unlock"}
          </button>
        </form>
      </div>
    </div>
  );
}
