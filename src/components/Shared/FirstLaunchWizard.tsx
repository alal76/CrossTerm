import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Terminal,
  User,
  Lock,
  Palette,
  Sun,
  Moon,
  Monitor,
  Check,
  ChevronRight,
  ChevronDown,
  AlertCircle,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useAppStore } from "@/stores/appStore";
import { ThemeVariant } from "@/types";

const TOTAL_STEPS = 4;

function StepIndicator({ current, total }: { readonly current: number; readonly total: number }) {
  return (
    <div className="flex items-center gap-2">
      {Array.from({ length: total }, (_, i) => (
        <div
          key={i}
          className={clsx(
            "w-2 h-2 rounded-full transition-colors duration-[var(--duration-short)]",
            getStepDotClass(i, current)
          )}
        />
      ))}
    </div>
  );
}

function getStepDotClass(i: number, current: number): string {
  if (i <= current) return "bg-accent-primary";
  return "bg-border-default";
}

export default function FirstLaunchWizard() {
  const { t } = useTranslation();
  const setFirstLaunchComplete = useAppStore((s) => s.setFirstLaunchComplete);
  const setTheme = useAppStore((s) => s.setTheme);
  const addProfile = useAppStore((s) => s.addProfile);
  const setActiveProfile = useAppStore((s) => s.setActiveProfile);

  const [step, setStep] = useState(0);
  const [profileName, setProfileName] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [selectedTheme, setSelectedTheme] = useState(ThemeVariant.Dark);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const handleCreateProfile = useCallback(async (): Promise<boolean> => {
    const name = profileName.trim();
    if (!name) {
      setError(t("wizard.profileRequired"));
      return false;
    }
    setLoading(true);
    try {
      const profileId = await invoke<string>("profile_create", { name });
      addProfile({ id: profileId, name, authMethod: "password", createdAt: new Date().toISOString() });
      setActiveProfile(profileId);
    } catch {
      const localId = crypto.randomUUID();
      addProfile({ id: localId, name, authMethod: "password", createdAt: new Date().toISOString() });
      setActiveProfile(localId);
    } finally {
      setLoading(false);
    }
    return true;
  }, [profileName, t, addProfile, setActiveProfile]);

  const handleCreateVault = useCallback(async (): Promise<boolean> => {
    if (!password) { setError(t("wizard.passwordRequired")); return false; }
    if (password !== confirmPassword) { setError(t("wizard.passwordMismatch")); return false; }
    if (password.length < 8) { setError(t("wizard.passwordTooShort")); return false; }
    setLoading(true);
    try {
      await invoke("vault_create", { masterPassword: password });
    } catch {
      // Backend may not be ready; proceed anyway
    } finally {
      setLoading(false);
    }
    return true;
  }, [password, confirmPassword, t]);

  const handleFinish = useCallback(async () => {
    setTheme(selectedTheme);
    try {
      await invoke("settings_update", { settings: { appearance: { theme: selectedTheme } } });
    } catch {
      // Best-effort
    }
    setFirstLaunchComplete(true);
  }, [selectedTheme, setTheme, setFirstLaunchComplete]);

  const handleNext = useCallback(async () => {
    setError(null);
    if (step === 1 && !(await handleCreateProfile())) return;
    if (step === 2 && !(await handleCreateVault())) return;
    if (step === 3) { await handleFinish(); return; }
    setStep((s) => s + 1);
  }, [step, handleCreateProfile, handleCreateVault, handleFinish]);

  return (
    <div className="fixed inset-0 z-[9000] flex items-center justify-center bg-surface-primary">
      <div className="flex flex-col items-center w-full max-w-md px-8 py-10">
        {/* Stepper */}
        <StepIndicator current={step} total={TOTAL_STEPS} />

        <div className="w-full mt-8">
          {/* Step 0: Welcome */}
          {step === 0 && (
            <div className="flex flex-col items-center text-center gap-6 animate-fade-in">
              <div className="w-16 h-16 rounded-2xl bg-accent-primary/10 flex items-center justify-center">
                <Terminal size={32} className="text-accent-primary" />
              </div>
              <div>
                <h1 className="text-2xl font-semibold text-text-primary mb-2">
                  {t("app.name")}
                </h1>
                <p className="text-sm text-text-secondary">
                  {t("wizard.welcome")}
                </p>
              </div>
              <button
                onClick={handleNext}
                className="flex items-center gap-2 px-6 py-2.5 rounded-lg bg-interactive-default hover:bg-interactive-hover text-text-inverse text-sm font-medium transition-colors duration-[var(--duration-short)]"
              >
                {t("wizard.getStarted")}
                <ChevronRight size={16} />
              </button>
            </div>
          )}

          {/* Step 1: Create Profile */}
          {step === 1 && (
            <div className="flex flex-col gap-6 animate-fade-in">
              <div className="flex flex-col items-center text-center gap-2">
                <div className="w-12 h-12 rounded-xl bg-accent-primary/10 flex items-center justify-center">
                  <User size={24} className="text-accent-primary" />
                </div>
                <h2 className="text-lg font-semibold text-text-primary">
                  {t("wizard.createProfile")}
                </h2>
                <p className="text-xs text-text-secondary">
                  {t("wizard.profileDescription")}
                </p>
              </div>
              <LearnMore
                summary={t("wizard.learnMoreProfile")}
                content={t("wizard.learnMoreProfileContent")}
              />
              <div className="flex flex-col gap-2">
                <label className="text-xs text-text-secondary" htmlFor="profile-name">
                  {t("wizard.profileName")}
                </label>
                <input
                  id="profile-name"
                  type="text"
                  value={profileName}
                  onChange={(e) => setProfileName(e.target.value)}
                  placeholder="My Profile"
                  autoFocus
                  className="w-full px-3 py-2 rounded-lg bg-surface-secondary border border-border-default text-text-primary text-sm outline-none focus:border-border-focus transition-colors duration-[var(--duration-short)]"
                  onKeyDown={(e) => e.key === "Enter" && handleNext()}
                />
              </div>
              {error && <ErrorMessage message={error} />}
              <WizardNav
                loading={loading}
                onBack={() => setStep(0)}
                onNext={handleNext}
                nextLabel={t("actions.create")}
              />
            </div>
          )}

          {/* Step 2: Set Master Password */}
          {step === 2 && (
            <div className="flex flex-col gap-6 animate-fade-in">
              <div className="flex flex-col items-center text-center gap-2">
                <div className="w-12 h-12 rounded-xl bg-accent-primary/10 flex items-center justify-center">
                  <Lock size={24} className="text-accent-primary" />
                </div>
                <h2 className="text-lg font-semibold text-text-primary">
                  {t("wizard.setPassword")}
                </h2>
                <p className="text-xs text-text-secondary">
                  {t("wizard.passwordDescription")}
                </p>
              </div>
              <LearnMore
                summary={t("wizard.learnMoreVault")}
                content={t("wizard.learnMoreVaultContent")}
              />
              <div className="flex flex-col gap-4">
                <div className="flex flex-col gap-2">
                  <label className="text-xs text-text-secondary" htmlFor="master-password">
                    {t("wizard.masterPassword")}
                  </label>
                  <input
                    id="master-password"
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-surface-secondary border border-border-default text-text-primary text-sm outline-none focus:border-border-focus transition-colors duration-[var(--duration-short)]"
                  />
                </div>
                <div className="flex flex-col gap-2">
                  <label className="text-xs text-text-secondary" htmlFor="confirm-password">
                    {t("wizard.confirmPassword")}
                  </label>
                  <input
                    id="confirm-password"
                    type="password"
                    value={confirmPassword}
                    onChange={(e) => setConfirmPassword(e.target.value)}
                    className="w-full px-3 py-2 rounded-lg bg-surface-secondary border border-border-default text-text-primary text-sm outline-none focus:border-border-focus transition-colors duration-[var(--duration-short)]"
                    onKeyDown={(e) => e.key === "Enter" && handleNext()}
                  />
                </div>
              </div>
              {error && <ErrorMessage message={error} />}
              <WizardNav
                loading={loading}
                onBack={() => setStep(1)}
                onNext={handleNext}
                nextLabel={t("actions.create")}
              />
            </div>
          )}

          {/* Step 3: Choose Theme */}
          {step === 3 && (
            <div className="flex flex-col gap-6 animate-fade-in">
              <div className="flex flex-col items-center text-center gap-2">
                <div className="w-12 h-12 rounded-xl bg-accent-primary/10 flex items-center justify-center">
                  <Palette size={24} className="text-accent-primary" />
                </div>
                <h2 className="text-lg font-semibold text-text-primary">
                  {t("wizard.chooseTheme")}
                </h2>
                <p className="text-xs text-text-secondary">
                  {t("wizard.themeDescription")}
                </p>
              </div>
              <LearnMore
                summary={t("wizard.learnMoreTheme")}
                content={t("wizard.learnMoreThemeContent")}
              />
              <div className="flex gap-3 justify-center">
                {([
                  { value: ThemeVariant.Dark, icon: <Moon size={20} />, label: t("themes.dark") },
                  { value: ThemeVariant.Light, icon: <Sun size={20} />, label: t("themes.light") },
                  { value: ThemeVariant.System, icon: <Monitor size={20} />, label: t("themes.system") },
                ] as const).map((option) => (
                  <button
                    key={option.value}
                    onClick={() => setSelectedTheme(option.value)}
                    className={clsx(
                      "flex flex-col items-center gap-2 px-6 py-4 rounded-xl border transition-colors duration-[var(--duration-short)]",
                      selectedTheme === option.value
                        ? "border-accent-primary bg-accent-primary/10 text-text-primary"
                        : "border-border-default hover:bg-surface-secondary text-text-secondary"
                    )}
                  >
                    {option.icon}
                    <span className="text-xs font-medium">{option.label}</span>
                    {selectedTheme === option.value && (
                      <Check size={14} className="text-accent-primary" />
                    )}
                  </button>
                ))}
              </div>
              <WizardNav
                loading={loading}
                onBack={() => setStep(2)}
                onNext={handleNext}
                nextLabel={t("wizard.finish")}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function LearnMore({ summary, content }: { readonly summary: string; readonly content: string }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="rounded-lg border border-border-subtle overflow-hidden">
      <button
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-2 w-full px-3 py-2 text-xs text-text-secondary hover:text-text-primary hover:bg-surface-secondary transition-colors text-left"
      >
        <ChevronDown
          size={12}
          className={clsx(
            "shrink-0 transition-transform duration-[var(--duration-short)]",
            !open && "-rotate-90"
          )}
        />
        {summary}
      </button>
      {open && (
        <div className="px-3 pb-3 text-xs text-text-secondary leading-relaxed animate-fade-in">
          {content}
        </div>
      )}
    </div>
  );
}

function ErrorMessage({ message }: { readonly message: string }) {
  return (
    <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-status-disconnected/10 text-status-disconnected text-xs">
      <AlertCircle size={14} />
      {message}
    </div>
  );
}

function WizardNav({
  loading,
  onBack,
  onNext,
  nextLabel,
}: {
  readonly loading: boolean;
  readonly onBack: () => void;
  readonly onNext: () => void;
  readonly nextLabel: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex justify-between items-center pt-2">
      <button
        onClick={onBack}
        className="px-4 py-2 rounded-lg text-xs text-text-secondary hover:text-text-primary hover:bg-surface-secondary transition-colors duration-[var(--duration-short)]"
      >
        {t("wizard.back")}
      </button>
      <button
        onClick={onNext}
        disabled={loading}
        className={clsx(
          "flex items-center gap-2 px-5 py-2 rounded-lg text-sm font-medium transition-colors duration-[var(--duration-short)]",
          loading
            ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
            : "bg-interactive-default hover:bg-interactive-hover text-text-inverse"
        )}
      >
        {nextLabel}
        <ChevronRight size={14} />
      </button>
    </div>
  );
}
