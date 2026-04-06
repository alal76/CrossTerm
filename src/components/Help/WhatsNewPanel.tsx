import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { X, Sparkles } from "lucide-react";

const APP_VERSION = "1.0.0";
const DISMISSED_VERSION_KEY = "crossterm:whats-new-dismissed";

const RELEASE_NOTES = `
## What's New in CrossTerm ${APP_VERSION}

### New Features
- **Credential Vault** — Store passwords, SSH keys, and API tokens securely with AES-256-GCM encryption.
- **Split Panes** — Divide your terminal into multiple panes horizontally or vertically.
- **Quick Connect** — Rapidly connect to hosts without saving a session first.
- **SFTP Browser** — Browse and transfer files over SFTP.

### Improvements
- Keyboard shortcut overlay (⌘/)
- Field-level help icons across forms
- Theme import/export support
- Broadcast mode for sending input to all panes

### Bug Fixes
- Fixed terminal resize on split pane changes
- Improved reconnection logic for SSH sessions
`;

export default function WhatsNewPanel() {
  const { t } = useTranslation();
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const dismissed = localStorage.getItem(DISMISSED_VERSION_KEY);
    if (dismissed !== APP_VERSION) {
      setVisible(true);
    }
  }, []);

  function handleDismiss() {
    setVisible(false);
  }

  function handleDontShowAgain() {
    localStorage.setItem(DISMISSED_VERSION_KEY, APP_VERSION);
    setVisible(false);
  }

  if (!visible) return null;

  return (
    <div className="fixed inset-0 z-[8500] flex items-center justify-center">
      <div
        className="absolute inset-0 bg-surface-overlay/60 backdrop-blur-sm"
        onClick={handleDismiss}
        onKeyDown={(e) => e.key === "Escape" && handleDismiss()}
        aria-hidden="true"
      />
      <div
        className="relative w-full max-w-lg max-h-[80vh] bg-surface-elevated border border-border-default rounded-xl shadow-[var(--shadow-3)] flex flex-col overflow-hidden"
        style={{ animation: "paletteIn var(--duration-medium) var(--ease-decelerate)" }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-3.5 border-b border-border-subtle shrink-0">
          <div className="flex items-center gap-2">
            <Sparkles size={16} className="text-accent-primary" />
            <h2 className="text-sm font-semibold text-text-primary">{t("whatsNew.title")}</h2>
          </div>
          <button
            onClick={handleDismiss}
            className="p-1 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
          >
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-5 py-4 text-xs text-text-secondary leading-relaxed">
          {RELEASE_NOTES.trim()
            .split("\n")
            .map((line, i) => {
              const key = `line-${String(i)}`;
              if (line.startsWith("## ")) {
                return (
                  <h3 key={key} className="text-sm font-semibold text-text-primary mt-3 mb-2">
                    {line.replace("## ", "")}
                  </h3>
                );
              }
              if (line.startsWith("### ")) {
                return (
                  <h4 key={key} className="text-xs font-medium text-text-primary mt-3 mb-1.5">
                    {line.replace("### ", "")}
                  </h4>
                );
              }
              if (line.startsWith("- ")) {
                const content = line.replace("- ", "");
                const boldMatch = /^\*\*(.+?)\*\*\s*—\s*(.+)$/.exec(content);
                if (boldMatch) {
                  return (
                    <p key={key} className="ml-3 mb-1">
                      <span className="text-text-primary font-medium">{boldMatch[1]}</span>
                      {" — "}
                      {boldMatch[2]}
                    </p>
                  );
                }
                return (
                  <p key={key} className="ml-3 mb-1">
                    • {content}
                  </p>
                );
              }
              if (line.trim() === "") return <div key={key} className="h-1" />;
              return (
                <p key={key} className="mb-1">
                  {line}
                </p>
              );
            })}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-border-subtle shrink-0">
          <button
            onClick={handleDontShowAgain}
            className="px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            {t("whatsNew.dontShowAgain")}
          </button>
          <button
            onClick={handleDismiss}
            className="px-3 py-1.5 text-xs rounded-lg bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            {t("whatsNew.dismiss")}
          </button>
        </div>
      </div>
    </div>
  );
}
