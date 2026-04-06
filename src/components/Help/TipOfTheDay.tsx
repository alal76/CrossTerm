import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Lightbulb, X, ChevronRight } from "lucide-react";

const TIPS = [
  "Press Ctrl+Shift+P (⌘⇧P on macOS) to open the Command Palette and quickly access any action.",
  "Use Ctrl+Shift+N (⌘⇧N) to open Quick Connect for a fast one-time SSH connection.",
  "Split your terminal into multiple panes with Ctrl+Shift+D (⌘⇧D) for side-by-side sessions.",
  "Store your SSH keys and passwords in the Credential Vault — they're encrypted with AES-256-GCM.",
  "Drag and drop files in the SFTP browser to upload or download between local and remote machines.",
  "Press Ctrl+/ (⌘/) to view all keyboard shortcuts at a glance.",
  "Switch between open tabs with Ctrl+Tab, or jump directly with Ctrl+1 through Ctrl+9.",
  "Use Ctrl+Shift+F (⌘F) to search within your terminal scrollback buffer.",
  "Set up SSH jump hosts (ProxyJump) to connect through an intermediate server when direct access isn't available.",
  "Import custom color themes via JSON to personalize your terminal experience.",
  "The vault auto-locks after 15 minutes of inactivity. Adjust this in Settings → Security.",
  "Right-click a tab to duplicate, split, pin, or close groups of sessions.",
];

const STORAGE_KEY_INDEX = "crossterm-tip-index";
const STORAGE_KEY_OPTOUT = "crossterm-tip-optout";

export default function TipOfTheDay() {
  const { t } = useTranslation();
  const [visible, setVisible] = useState(false);
  const [tipIndex, setTipIndex] = useState(0);

  useEffect(() => {
    const optedOut = localStorage.getItem(STORAGE_KEY_OPTOUT) === "true";
    if (optedOut) return;

    const stored = localStorage.getItem(STORAGE_KEY_INDEX);
    const idx = stored ? (Number.parseInt(stored, 10) + 1) % TIPS.length : 0;
    setTipIndex(idx);
    localStorage.setItem(STORAGE_KEY_INDEX, String(idx));

    // Show after a short delay so it doesn't flash during startup
    const timer = setTimeout(() => setVisible(true), 1500);
    return () => clearTimeout(timer);
  }, []);

  const handleDismiss = useCallback(() => {
    setVisible(false);
  }, []);

  const handleNextTip = useCallback(() => {
    const next = (tipIndex + 1) % TIPS.length;
    setTipIndex(next);
    localStorage.setItem(STORAGE_KEY_INDEX, String(next));
  }, [tipIndex]);

  const handleOptOut = useCallback(() => {
    localStorage.setItem(STORAGE_KEY_OPTOUT, "true");
    setVisible(false);
  }, []);

  if (!visible) return null;

  return (
    <aside
      className="fixed bottom-12 right-4 z-[7000] w-80 bg-surface-elevated border border-border-default rounded-xl shadow-[var(--shadow-3)] overflow-hidden"
      style={{ animation: "paletteIn var(--duration-medium) var(--ease-decelerate)" }}
      aria-label={t("tipOfDay.title")}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-border-subtle">
        <div className="flex items-center gap-2">
          <Lightbulb size={14} className="text-accent-primary" />
          <span className="text-xs font-semibold text-text-primary">
            {t("tipOfDay.title")}
          </span>
        </div>
        <button
          onClick={handleDismiss}
          className="p-0.5 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors"
        >
          <X size={14} />
        </button>
      </div>

      {/* Tip content */}
      <div className="px-3 py-3">
        <p className="text-xs text-text-secondary leading-relaxed">
          {TIPS[tipIndex]}
        </p>
      </div>

      {/* Actions */}
      <div className="flex items-center justify-between px-3 py-2 border-t border-border-subtle">
        <button
          onClick={handleOptOut}
          className="text-[10px] text-text-disabled hover:text-text-secondary transition-colors"
        >
          {t("tipOfDay.dontShowAgain")}
        </button>
        <button
          onClick={handleNextTip}
          className="flex items-center gap-1 text-xs text-accent-primary hover:text-accent-secondary transition-colors font-medium"
        >
          {t("tipOfDay.nextTip")}
          <ChevronRight size={12} />
        </button>
      </div>
    </aside>
  );
}
