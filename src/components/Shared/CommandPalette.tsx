import { useState, useEffect, useRef, useCallback, useMemo } from "react";
import clsx from "clsx";
import {
  Terminal,
  Globe,
  PanelLeftClose,
  PanelBottomClose,
  Settings,
  Lock,
  Palette,
  Command,
} from "lucide-react";
import { useAppStore } from "@/stores/appStore";
import { ThemeVariant } from "@/types";

interface PaletteAction {
  id: string;
  label: string;
  icon: React.ReactNode;
  shortcut?: string;
  handler: () => void;
}

interface CommandPaletteProps {
  onNewLocalShell?: () => void;
  onNewSSHSession?: () => void;
  onOpenSettings?: () => void;
  onLockVault?: () => void;
}

export default function CommandPalette({
  onNewLocalShell,
  onNewSSHSession,
  onOpenSettings,
  onLockVault,
}: CommandPaletteProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const toggleBottomPanel = useAppStore((s) => s.toggleBottomPanel);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);

  const close = useCallback(() => {
    setOpen(false);
    setQuery("");
    setSelectedIndex(0);
  }, []);

  const actions: PaletteAction[] = useMemo(
    () => [
      {
        id: "new-local-shell",
        label: "New Local Shell",
        icon: <Terminal size={16} />,
        shortcut: "⌘T",
        handler: () => {
          close();
          onNewLocalShell?.();
        },
      },
      {
        id: "new-ssh-session",
        label: "New SSH Session",
        icon: <Globe size={16} />,
        handler: () => {
          close();
          onNewSSHSession?.();
        },
      },
      {
        id: "toggle-sidebar",
        label: "Toggle Sidebar",
        icon: <PanelLeftClose size={16} />,
        shortcut: "⌘B",
        handler: () => {
          close();
          toggleSidebar();
        },
      },
      {
        id: "toggle-bottom-panel",
        label: "Toggle Bottom Panel",
        icon: <PanelBottomClose size={16} />,
        shortcut: "⌘J",
        handler: () => {
          close();
          toggleBottomPanel();
        },
      },
      {
        id: "open-settings",
        label: "Open Settings",
        icon: <Settings size={16} />,
        shortcut: "⌘,",
        handler: () => {
          close();
          onOpenSettings?.();
        },
      },
      {
        id: "lock-vault",
        label: "Lock Vault",
        icon: <Lock size={16} />,
        handler: () => {
          close();
          onLockVault?.();
        },
      },
      {
        id: "switch-theme",
        label: `Switch to ${theme === ThemeVariant.Dark ? "Light" : "Dark"} Theme`,
        icon: <Palette size={16} />,
        handler: () => {
          close();
          setTheme(theme === ThemeVariant.Dark ? ThemeVariant.Light : ThemeVariant.Dark);
        },
      },
    ],
    [close, onNewLocalShell, onNewSSHSession, onOpenSettings, onLockVault, toggleSidebar, toggleBottomPanel, theme, setTheme]
  );

  const filtered = useMemo(() => {
    if (!query) return actions;
    const lower = query.toLowerCase();
    return actions.filter((a) => {
      // Simple fuzzy: check if all chars appear in order
      let ai = 0;
      for (const char of lower) {
        ai = a.label.toLowerCase().indexOf(char, ai);
        if (ai === -1) return false;
        ai++;
      }
      return true;
    });
  }, [query, actions]);

  // Global shortcut listener
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === "p") {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
    }
    globalThis.addEventListener("keydown", onKeyDown);
    return () => globalThis.removeEventListener("keydown", onKeyDown);
  }, []);

  // Focus input when opened
  useEffect(() => {
    if (open) {
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  // Keep selected index in bounds
  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  // Scroll selected item into view
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const item = list.children[selectedIndex] as HTMLElement | undefined;
    item?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  function handleKeyDown(e: React.KeyboardEvent) {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, filtered.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
        break;
      case "Enter":
        e.preventDefault();
        filtered[selectedIndex]?.handler();
        break;
      case "Escape":
        e.preventDefault();
        close();
        break;
    }
  }

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-[9000] flex justify-center pt-[10vh]"
      onClick={close}
      onKeyDown={(e) => e.key === "Escape" && close()}
      role="presentation"
    >
      <div
        className="absolute inset-0 bg-surface-overlay/60 backdrop-blur-sm"
        style={{ transition: `opacity var(--duration-short) var(--ease-default)` }}
      />
      <div
        className="relative w-full max-w-lg bg-surface-elevated rounded-xl border border-border-default shadow-[var(--shadow-3)] overflow-hidden"
        style={{
          maxHeight: "400px",
          animation: "paletteIn var(--duration-medium) var(--ease-decelerate)",
        }}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.key === "Escape" && close()}
        role="dialog"
      >
        <div className="flex items-center gap-2 px-4 py-3 border-b border-border-subtle">
          <Command size={16} className="text-text-secondary shrink-0" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a command…"
            className="flex-1 bg-transparent text-sm text-text-primary placeholder:text-text-disabled outline-none"
            spellCheck={false}
          />
        </div>
        <div ref={listRef} className="overflow-y-auto" style={{ maxHeight: "340px" }}>
          {filtered.length === 0 ? (
            <div className="px-4 py-6 text-center text-xs text-text-secondary">
              No matching commands
            </div>
          ) : (
            filtered.map((action, idx) => (
              <button
                key={action.id}
                className={clsx(
                  "flex items-center gap-3 w-full px-4 py-2.5 text-left text-sm",
                  "transition-colors duration-[var(--duration-micro)]",
                  idx === selectedIndex
                    ? "bg-interactive-default/20 text-text-primary"
                    : "text-text-secondary hover:bg-surface-secondary hover:text-text-primary"
                )}
                onClick={action.handler}
                onMouseEnter={() => setSelectedIndex(idx)}
              >
                <span className="shrink-0 text-text-secondary">{action.icon}</span>
                <span className="flex-1">{action.label}</span>
                {action.shortcut && (
                  <kbd className="text-xs text-text-disabled bg-surface-secondary px-1.5 py-0.5 rounded">
                    {action.shortcut}
                  </kbd>
                )}
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
