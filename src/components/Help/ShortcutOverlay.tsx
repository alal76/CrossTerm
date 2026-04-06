import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import { X, Search, Keyboard, Printer } from "lucide-react";
import type { KeyboardShortcut } from "@/types";
import { useAppStore } from "@/stores/appStore";

const isMac = /mac/i.test(navigator.userAgent);

export const SHORTCUTS: KeyboardShortcut[] = [
  // General
  { id: "cmd-palette", keys: "Ctrl+Shift+P", macKeys: "⌘⇧P", label: "Command Palette", category: "general" },
  { id: "help", keys: "F1", macKeys: "F1", label: "Help Panel", category: "general" },
  { id: "shortcuts", keys: "Ctrl+/", macKeys: "⌘/", label: "Keyboard Shortcuts", category: "general" },
  { id: "settings", keys: "Ctrl+,", macKeys: "⌘,", label: "Settings", category: "general" },
  { id: "quick-connect", keys: "Ctrl+Shift+N", macKeys: "⌘⇧N", label: "Quick Connect", category: "general" },
  { id: "toggle-sidebar", keys: "Ctrl+B", macKeys: "⌘B", label: "Toggle Sidebar", category: "general" },
  { id: "toggle-bottom", keys: "Ctrl+J", macKeys: "⌘J", label: "Toggle Bottom Panel", category: "general" },

  // Tabs
  { id: "new-tab", keys: "Ctrl+T", macKeys: "⌘T", label: "New Local Shell", category: "tabs" },
  { id: "close-tab", keys: "Ctrl+W", macKeys: "⌘W", label: "Close Tab", category: "tabs" },
  { id: "next-tab", keys: "Ctrl+Tab", macKeys: "⌃Tab", label: "Next Tab", category: "tabs" },
  { id: "prev-tab", keys: "Ctrl+Shift+Tab", macKeys: "⌃⇧Tab", label: "Previous Tab", category: "tabs" },
  { id: "tab-1-9", keys: "Ctrl+1–9", macKeys: "⌘1–9", label: "Go to Tab 1–9", category: "tabs" },

  // Terminal
  { id: "copy", keys: "Ctrl+Shift+C", macKeys: "⌘C", label: "Copy", category: "terminal" },
  { id: "paste", keys: "Ctrl+Shift+V", macKeys: "⌘V", label: "Paste", category: "terminal" },
  { id: "clear", keys: "Ctrl+Shift+K", macKeys: "⌘K", label: "Clear Terminal", category: "terminal" },
  { id: "search", keys: "Ctrl+Shift+F", macKeys: "⌘F", label: "Search in Terminal", category: "terminal" },
  { id: "zoom-in", keys: "Ctrl+=", macKeys: "⌘=", label: "Zoom In", category: "terminal" },
  { id: "zoom-out", keys: "Ctrl+-", macKeys: "⌘-", label: "Zoom Out", category: "terminal" },
  { id: "zoom-reset", keys: "Ctrl+0", macKeys: "⌘0", label: "Reset Zoom", category: "terminal" },

  // Split Panes
  { id: "split-right", keys: "Ctrl+Shift+D", macKeys: "⌘⇧D", label: "Split Right", category: "splitPanes" },
  { id: "split-down", keys: "Ctrl+Shift+E", macKeys: "⌘⇧E", label: "Split Down", category: "splitPanes" },
  { id: "focus-left", keys: "Alt+Left", macKeys: "⌥←", label: "Focus Left Pane", category: "splitPanes" },
  { id: "focus-right", keys: "Alt+Right", macKeys: "⌥→", label: "Focus Right Pane", category: "splitPanes" },
  { id: "focus-up", keys: "Alt+Up", macKeys: "⌥↑", label: "Focus Up Pane", category: "splitPanes" },
  { id: "focus-down", keys: "Alt+Down", macKeys: "⌥↓", label: "Focus Down Pane", category: "splitPanes" },

  // Navigation
  { id: "focus-sidebar", keys: "Ctrl+Shift+S", macKeys: "⌘⇧S", label: "Focus Sidebar", category: "navigation" },
  { id: "focus-terminal", keys: "Ctrl+Shift+T", macKeys: "⌘⇧T", label: "Focus Terminal", category: "navigation" },
  { id: "focus-bottom", keys: "Ctrl+Shift+J", macKeys: "⌘⇧J", label: "Focus Bottom Panel", category: "navigation" },
  { id: "cycle-focus", keys: "F6", macKeys: "F6", label: "Cycle Focus Region", category: "navigation" },
];

const CATEGORY_ORDER = ["general", "tabs", "terminal", "splitPanes", "navigation"];

interface ShortcutOverlayProps {
  readonly open: boolean;
  readonly onClose: () => void;
}

export default function ShortcutOverlay({ open, onClose }: ShortcutOverlayProps) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const customShortcuts = useAppStore((s) => s.customShortcuts);

  const shortcuts = useMemo(() => {
    return SHORTCUTS.map((s) => {
      const custom = customShortcuts[s.id];
      if (!custom) return { ...s, isCustom: false };
      return {
        ...s,
        keys: custom.keys ?? s.keys,
        macKeys: custom.macKeys ?? s.macKeys,
        isCustom: true,
      };
    });
  }, [customShortcuts]);

  const filtered = useMemo(() => {
    if (!query.trim()) return shortcuts;
    const lower = query.toLowerCase();
    return shortcuts.filter(
      (s) =>
        s.label.toLowerCase().includes(lower) ||
        s.keys.toLowerCase().includes(lower) ||
        s.macKeys.toLowerCase().includes(lower),
    );
  }, [query, shortcuts]);

  const grouped = useMemo(() => {
    const groups: Record<string, KeyboardShortcut[]> = {};
    for (const shortcut of filtered) {
      if (!groups[shortcut.category]) {
        groups[shortcut.category] = [];
      }
      groups[shortcut.category].push(shortcut);
    }
    return groups;
  }, [filtered]);

  const sortedCategories = useMemo(
    () =>
      Object.keys(grouped).sort(
        (a, b) => CATEGORY_ORDER.indexOf(a) - CATEGORY_ORDER.indexOf(b),
      ),
    [grouped],
  );

  // Focus search on open
  useEffect(() => {
    if (open) {
      requestAnimationFrame(() => searchRef.current?.focus());
      setQuery("");
    }
  }, [open]);

  // Close on ESC
  useEffect(() => {
    if (!open) return;
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    globalThis.addEventListener("keydown", handleKeyDown);
    return () => globalThis.removeEventListener("keydown", handleKeyDown);
  }, [open, onClose]);

  const handleExport = useCallback(() => {
    const lines: string[] = [t("help.shortcutOverlay.title"), ""];
    for (const cat of sortedCategories) {
      lines.push(t(`help.shortcutOverlay.categories.${cat}`, cat).toUpperCase());
      lines.push("-".repeat(40));
      for (const s of grouped[cat]) {
        const keys = isMac ? s.macKeys : s.keys;
        const custom = "isCustom" in s && s.isCustom ? ` ${t("help.shortcutOverlay.customLabel")}` : "";
        lines.push(`  ${s.label.padEnd(24)} ${keys}${custom}`);
      }
      lines.push("");
    }
    const blob = new Blob([lines.join("\n")], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "crossterm-shortcuts.txt";
    a.click();
    URL.revokeObjectURL(url);
  }, [sortedCategories, grouped, t]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[9000] flex items-center justify-center"
      onClick={onClose}
      onKeyDown={(e) => { if (e.key === "Escape") onClose(); }}
      role="dialog"
      aria-modal="true"
      aria-label={t("help.shortcutOverlay.title")}
    >
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/40" />

      {/* Modal */}
      <div
        className="shortcut-overlay relative w-full max-w-[560px] max-h-[70vh] bg-surface-primary rounded-xl border border-border-default shadow-[var(--shadow-3)] flex flex-col overflow-hidden"
        style={{ animation: "paletteIn var(--duration-short) var(--ease-decelerate)" }}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => { if (e.key === "Escape") onClose(); }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-subtle">
          <div className="flex items-center gap-2">
            <Keyboard size={16} className="text-accent-primary" />
            <span className="text-sm font-semibold text-text-primary">
              {t("help.shortcutOverlay.title")}
            </span>
          </div>
          <div className="flex items-center gap-1">
            <button
              onClick={handleExport}
              className="p-1 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
              title={t("help.shortcutOverlay.print")}
            >
              <Printer size={16} />
            </button>
            <button
              onClick={onClose}
              className="p-1 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
            >
              <X size={16} />
            </button>
          </div>
        </div>

        {/* Search */}
        <div className="px-4 py-2 border-b border-border-subtle">
          <div className="flex items-center gap-1.5 px-2 py-1 rounded bg-surface-sunken border border-border-subtle">
            <Search size={12} className="text-text-disabled shrink-0" />
            <input
              ref={searchRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("help.shortcutOverlay.search")}
              className="flex-1 bg-transparent text-xs text-text-primary placeholder:text-text-disabled outline-none"
            />
          </div>
        </div>

        {/* Shortcut list */}
        <div className="flex-1 overflow-y-auto px-4 py-3">
          {sortedCategories.length === 0 && (
            <p className="text-xs text-text-disabled text-center py-4">
              {t("help.shortcutOverlay.noResults")}
            </p>
          )}
          {sortedCategories.map((category) => (
            <div key={category} className="mb-4 last:mb-0">
              <h3 className="text-[10px] font-semibold uppercase tracking-wider text-text-secondary mb-2">
                {t(`help.shortcutOverlay.categories.${category}`, category)}
              </h3>
              <div className="space-y-0.5">
                {grouped[category].map((shortcut) => (
                  <div
                    key={shortcut.id}
                    className="flex items-center justify-between px-2 py-1.5 rounded hover:bg-surface-elevated transition-colors"
                  >
                    <span className="text-xs text-text-secondary">
                      {shortcut.label}
                      {(shortcut as KeyboardShortcut & { isCustom?: boolean }).isCustom && (
                        <span className="ml-1 text-[10px] text-accent-primary">
                          {t("help.shortcutOverlay.customLabel")}
                        </span>
                      )}
                    </span>
                    <kbd className={clsx(
                      "inline-flex items-center px-1.5 py-0.5 rounded text-[11px] font-mono",
                      "bg-surface-sunken border border-border-subtle text-text-primary",
                    )}>
                      {isMac ? shortcut.macKeys : shortcut.keys}
                    </kbd>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
