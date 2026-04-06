import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  HelpCircle,
  BookOpen,
  Keyboard,
  AlertTriangle,
  ExternalLink,
  Info,
  Compass,
} from "lucide-react";

interface HelpMenuProps {
  readonly onOpenHelp: () => void;
  readonly onOpenShortcuts: () => void;
  readonly onStartTour?: (tourId: string) => void;
}

export default function HelpMenu({ onOpenHelp, onOpenShortcuts, onStartTour }: HelpMenuProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const btnRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    function handleClick(e: MouseEvent) {
      if (
        menuRef.current && !menuRef.current.contains(e.target as Node) &&
        btnRef.current && !btnRef.current.contains(e.target as Node)
      ) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [open]);

  const items = [
    {
      id: "getting-started",
      icon: <BookOpen size={13} />,
      label: t("help.menu.gettingStarted"),
      action: () => { setOpen(false); onOpenHelp(); },
    },
    {
      id: "shortcuts",
      icon: <Keyboard size={13} />,
      label: t("help.menu.keyboardShortcuts"),
      shortcut: "⌘/",
      action: () => { setOpen(false); onOpenShortcuts(); },
    },
    {
      id: "tour",
      icon: <Compass size={13} />,
      label: t("tour.startTour"),
      action: () => { setOpen(false); onStartTour?.("ssh"); },
    },
    { id: "sep1", divider: true as const },
    {
      id: "online-docs",
      icon: <ExternalLink size={13} />,
      label: t("help.menu.onlineDocs"),
      action: () => {
        setOpen(false);
        window.open("https://aalmonstorth.github.io/CrossTerm/", "_blank", "noopener,noreferrer");
      },
    },
    {
      id: "troubleshooting",
      icon: <AlertTriangle size={13} />,
      label: t("help.menu.troubleshooting"),
      action: () => { setOpen(false); onOpenHelp(); },
    },
    {
      id: "report-issue",
      icon: <ExternalLink size={13} />,
      label: t("help.menu.reportIssue"),
      action: () => {
        setOpen(false);
        const version = (globalThis as Record<string, unknown>).__TAURI__
          ? "unknown"
          : "dev";
        const nav = navigator as unknown as Record<string, unknown>;
        const uaData = nav.userAgentData as Record<string, string> | undefined;
        const os = uaData?.platform ?? "unknown";
        const title = encodeURIComponent("Bug Report");
        const body = encodeURIComponent(
          `## Environment\n- **App version:** ${version}\n- **OS:** ${os}\n- **User agent:** ${navigator.userAgent}\n\n## Description\n\nDescribe the bug here…\n\n## Steps to Reproduce\n\n1. \n2. \n3. \n\n## Expected Behavior\n\n\n## Actual Behavior\n\n\n## Screenshots (if applicable)\n\n`
        );
        const url = `https://github.com/user/crossterm/issues/new?title=${title}&body=${body}`;
        globalThis.open(url, "_blank", "noopener,noreferrer");
      },
    },
    { id: "sep2", divider: true as const },
    {
      id: "about",
      icon: <Info size={13} />,
      label: t("help.menu.about"),
      action: () => { setOpen(false); },
    },
  ];

  return (
    <div className="relative">
      <button
        ref={btnRef}
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-1 px-1.5 h-full text-text-secondary hover:text-text-primary transition-colors"
        title={t("help.title")}
      >
        <HelpCircle size={11} />
      </button>

      {open && (
        <div
          ref={menuRef}
          className="absolute bottom-full right-0 mb-1 min-w-[180px] bg-surface-elevated border border-border-default rounded-lg shadow-[var(--shadow-3)] py-1 z-[8000]"
          style={{ animation: "paletteIn var(--duration-short) var(--ease-decelerate)" }}
        >
          {items.map((item) =>
            "divider" in item ? (
              <div key={item.id} className="h-px bg-border-subtle mx-2 my-1" />
            ) : (
              <button
                key={item.id}
                onClick={item.action}
                className="flex items-center gap-2.5 w-full px-3 py-1.5 text-xs text-left text-text-secondary hover:bg-surface-secondary hover:text-text-primary transition-colors"
              >
                <span className="shrink-0">{item.icon}</span>
                <span className="flex-1">{item.label}</span>
                {"shortcut" in item && item.shortcut && (
                  <span className="text-[10px] text-text-disabled">{item.shortcut}</span>
                )}
              </button>
            ),
          )}
        </div>
      )}
    </div>
  );
}
