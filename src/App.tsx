import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import clsx from "clsx";
import {
  Terminal,
  Code2,
  Lock,
  Plus,
  X,
  ChevronLeft,
  ChevronRight,
  User,
  Sun,
  Moon,
  Monitor,
  Globe,
  FolderTree,
  FolderOpen,
  SplitSquareHorizontal,
  SplitSquareVertical,
  Pencil,
  Copy,
  MoreVertical,
  Radio,
  KeyRound,
} from "lucide-react";
import { useAppStore } from "@/stores/appStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useTerminalStore } from "@/stores/terminalStore";
import { useVaultStore } from "@/stores/vaultStore";
import { SidebarMode, BottomPanelMode, ThemeVariant, ConnectionStatus, SessionType, SplitDirection } from "@/types";
import type { Session } from "@/types";
import { useBreakpoint } from "@/hooks/useBreakpoint";
import TerminalTab from "@/components/Terminal/TerminalTab";
import SshTerminalTab from "@/components/Terminal/SshTerminalTab";
import SplitPaneContainer from "@/components/Terminal/SplitPaneContainer";
import CommandPalette from "@/components/Shared/CommandPalette";
import QuickConnect from "@/components/Shared/QuickConnect";
import FirstLaunchWizard from "@/components/Shared/FirstLaunchWizard";
import HelpPanel from "@/components/Help/HelpPanel";
import ShortcutOverlay from "@/components/Help/ShortcutOverlay";
import HelpMenu from "@/components/Help/HelpMenu";
import WhatsNewPanel from "@/components/Help/WhatsNewPanel";
import FeatureTour from "@/components/Help/FeatureTour";
import TipOfTheDay from "@/components/Help/TipOfTheDay";
import VaultUnlock from "@/components/Vault/VaultUnlock";
import CredentialManager from "@/components/Vault/CredentialManager";
import SettingsPanel from "@/components/Settings/SettingsPanel";
import SessionEditor from "@/components/SessionTree/SessionEditor";
import SftpBrowser from "@/components/SftpBrowser/SftpBrowser";

const SESSION_TYPE_ICONS: Record<string, string> = {
  ssh: "⌨",
  sftp: "📁",
  rdp: "🖥",
  vnc: "🖥",
  local_shell: "⌨",
  serial: "🔌",
  telnet: "📡",
  cloud_shell: "☁",
  wsl: "🐧",
  kubernetes_exec: "☸",
  docker_exec: "🐳",
  web_console: "🌐",
  scp: "📤",
};

const STATUS_SHAPES: Record<ConnectionStatus, string> = {
  [ConnectionStatus.Connected]: "●",
  [ConnectionStatus.Connecting]: "◌",
  [ConnectionStatus.Disconnected]: "○",
  [ConnectionStatus.Idle]: "○",
};

const STATUS_DOT_COLORS: Record<ConnectionStatus, string> = {
  [ConnectionStatus.Connected]: "text-status-connected",
  [ConnectionStatus.Disconnected]: "text-status-disconnected",
  [ConnectionStatus.Connecting]: "text-status-connecting",
  [ConnectionStatus.Idle]: "text-status-idle",
};

function StatusDot({ status }: { readonly status: ConnectionStatus }) {
  return (
    <span
      className={clsx(
        "inline-block text-[8px] leading-none",
        STATUS_DOT_COLORS[status],
        status === ConnectionStatus.Connecting && "animate-pulse"
      )}
      aria-label={status}
    >
      {STATUS_SHAPES[status]}
    </span>
  );
}

// ─── Region A: Title Bar ───────────────────────────────────────

function TitleBar() {
  const { t } = useTranslation();
  const activeProfileId = useAppStore((s) => s.activeProfileId);
  const profiles = useAppStore((s) => s.profiles);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);

  const activeProfile = profiles.find((p) => p.id === activeProfileId);

  const cycleTheme = useCallback(() => {
    const order = [ThemeVariant.Dark, ThemeVariant.Light, ThemeVariant.System] as const;
    const idx = order.indexOf(theme);
    setTheme(order[(idx + 1) % order.length]);
  }, [theme, setTheme]);

  let themeIcon = <Sun size={14} />;
  if (theme === ThemeVariant.Light) themeIcon = <Moon size={14} />;
  else if (theme === ThemeVariant.System) themeIcon = <Monitor size={14} />;

  const themeLabel = theme === ThemeVariant.System ? t("themes.system") : t(`themes.${theme}`);

  return (
    <header
      className="flex items-center h-10 px-3 bg-surface-primary border-b border-border-subtle no-select shrink-0"
      data-tauri-drag-region
    >
      <span className="text-sm font-semibold text-accent-primary mr-3 tracking-wide">
        CrossTerm
      </span>
      <div className="flex-1" data-tauri-drag-region />
      <button
        onClick={cycleTheme}
        className="p-1.5 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
        title={themeLabel}
        data-tooltip={themeLabel}
      >
        {themeIcon}
      </button>
      <div className="flex items-center gap-1.5 ml-2 px-2 py-1 rounded hover:bg-surface-elevated cursor-pointer">
        <User size={14} className="text-text-secondary" />
        <span className="text-xs text-text-secondary">{activeProfile?.name ?? t("titleBar.profile")}</span>
      </div>
    </header>
  );
}

// ─── Region B: Tab Bar ─────────────────────────────────────────

interface TabContextMenuState {
  x: number;
  y: number;
  tabId: string;
}

function TabContextMenu({
  state,
  onClose,
}: {
  readonly state: TabContextMenuState;
  readonly onClose: () => void;
}) {
  const { t } = useTranslation();
  const menuRef = useRef<HTMLDivElement>(null);
  const closeTab = useSessionStore((s) => s.closeTab);
  const openTabs = useSessionStore((s) => s.openTabs);
  const sessions = useSessionStore((s) => s.sessions);
  const addSession = useSessionStore((s) => s.addSession);
  const openTab = useSessionStore((s) => s.openTab);
  const setSplitPane = useSessionStore((s) => s.setSplitPane);
  const activeTabId = useSessionStore((s) => s.activeTabId);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [onClose]);

  const tab = openTabs.find((tt) => tt.id === state.tabId);
  if (!tab) return null;

  const handleDuplicate = () => {
    const session = sessions.find((s) => s.id === tab.sessionId);
    if (session) {
      const dup: Session = {
        ...session,
        id: crypto.randomUUID(),
        name: `${session.name} (copy)`,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      addSession(dup);
      openTab(dup);
    }
  };

  const handleSplit = (direction: SplitDirection) => {
    if (!activeTabId) return;
    setSplitPane({
      type: "container",
      direction,
      children: [
        { type: "leaf", tabId: activeTabId },
        { type: "leaf", tabId: state.tabId },
      ],
      sizes: [50, 50],
    });
  };

  const handleCloseOthers = () => {
    for (const ot of openTabs) {
      if (ot.id !== state.tabId && !ot.pinned) closeTab(ot.id);
    }
  };

  const handleCloseAll = () => {
    for (const ot of openTabs) {
      if (!ot.pinned) closeTab(ot.id);
    }
  };

  const items = [
    { key: "rename", icon: <Pencil size={13} />, label: t("tabs.rename") },
    { key: "duplicate", icon: <Copy size={13} />, label: t("tabs.duplicateTab"), action: handleDuplicate },
    { key: "sep1", divider: true as const },
    { key: "splitRight", icon: <SplitSquareHorizontal size={13} />, label: t("tabs.splitRight"), action: () => handleSplit(SplitDirection.Horizontal) },
    { key: "splitDown", icon: <SplitSquareVertical size={13} />, label: t("tabs.splitDown"), action: () => handleSplit(SplitDirection.Vertical) },
    { key: "sep2", divider: true as const },
    { key: "close", icon: <X size={13} />, label: t("tabs.closeTab"), action: () => closeTab(state.tabId) },
    { key: "closeOthers", icon: <X size={13} />, label: t("tabs.closeOthers"), action: handleCloseOthers },
    { key: "closeAll", icon: <X size={13} />, label: t("tabs.closeAll"), action: handleCloseAll, danger: true },
  ];

  return (
    <div
      ref={menuRef}
      className="fixed z-[8000] min-w-[180px] bg-surface-elevated border border-border-default rounded-lg shadow-[var(--shadow-3)] py-1 overflow-hidden"
      style={{ left: state.x, top: state.y, animation: "paletteIn var(--duration-short) var(--ease-decelerate)" }}
    >
      {items.map((item) =>
        "divider" in item ? (
          <div key={item.key} className="h-px bg-border-subtle mx-2 my-1" />
        ) : (
          <button
            key={item.key}
            onClick={() => {
              item.action?.();
              onClose();
            }}
            className={clsx(
              "flex items-center gap-2.5 w-full px-3 py-1.5 text-xs text-left",
              "transition-colors duration-[var(--duration-micro)]",
              "danger" in item && item.danger
                ? "text-status-disconnected hover:bg-status-disconnected/10"
                : "text-text-secondary hover:bg-surface-secondary hover:text-text-primary"
            )}
          >
            <span className="shrink-0">{item.icon}</span>
            {item.label}
          </button>
        )
      )}
    </div>
  );
}

function NewTabDropdown({
  onNewLocalShell,
  onNewSSH,
  onNewSFTP,
  onClose,
  anchorRef,
}: {
  readonly onNewLocalShell: () => void;
  readonly onNewSSH: () => void;
  readonly onNewSFTP: () => void;
  readonly onClose: () => void;
  readonly anchorRef: React.RefObject<HTMLButtonElement | null>;
}) {
  const { t } = useTranslation();
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (
        menuRef.current && !menuRef.current.contains(e.target as Node) &&
        anchorRef.current && !anchorRef.current.contains(e.target as Node)
      ) {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [onClose, anchorRef]);

  const rect = anchorRef.current?.getBoundingClientRect();
  const left = rect ? rect.left : 0;
  const top = rect ? rect.bottom + 2 : 0;

  return (
    <div
      ref={menuRef}
      className="fixed z-[8000] min-w-[180px] bg-surface-elevated border border-border-default rounded-lg shadow-[var(--shadow-3)] py-1"
      style={{ left, top, animation: "paletteIn var(--duration-short) var(--ease-decelerate)" }}
    >
      <button
        onClick={() => { onNewLocalShell(); onClose(); }}
        className="flex items-center gap-2.5 w-full px-3 py-1.5 text-xs text-left text-text-secondary hover:bg-surface-secondary hover:text-text-primary transition-colors"
      >
        <Terminal size={13} className="shrink-0" />
        {t("newTabMenu.localShell")}
      </button>
      <button
        onClick={() => { onNewSSH(); onClose(); }}
        className="flex items-center gap-2.5 w-full px-3 py-1.5 text-xs text-left text-text-secondary hover:bg-surface-secondary hover:text-text-primary transition-colors"
      >
        <Globe size={13} className="shrink-0" />
        {t("newTabMenu.ssh")}
      </button>
      <button
        onClick={() => { onNewSFTP(); onClose(); }}
        className="flex items-center gap-2.5 w-full px-3 py-1.5 text-xs text-left text-text-secondary hover:bg-surface-secondary hover:text-text-primary transition-colors"
      >
        <FolderTree size={13} className="shrink-0" />
        {t("newTabMenu.sftp")}
      </button>
    </div>
  );
}

function TabBar({
  onNewLocalShell,
  onNewSSH,
  onNewSFTP,
}: {
  readonly onNewLocalShell: () => void;
  readonly onNewSSH: () => void;
  readonly onNewSFTP: () => void;
}) {
  const { t } = useTranslation();
  const openTabs = useSessionStore((s) => s.openTabs);
  const activeTabId = useSessionStore((s) => s.activeTabId);
  const setActiveTab = useSessionStore((s) => s.setActiveTab);
  const closeTab = useSessionStore((s) => s.closeTab);
  const broadcastMode = useTerminalStore((s) => s.broadcastMode);
  const toggleBroadcastMode = useTerminalStore((s) => s.toggleBroadcastMode);

  const [tabContextMenu, setTabContextMenu] = useState<TabContextMenuState | null>(null);
  const [showNewTabDropdown, setShowNewTabDropdown] = useState(false);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const newTabBtnRef = useRef<HTMLButtonElement>(null);
  const [canScrollLeft, setCanScrollLeft] = useState(false);
  const [canScrollRight, setCanScrollRight] = useState(false);

  const checkScroll = useCallback(() => {
    const el = scrollContainerRef.current;
    if (!el) return;
    setCanScrollLeft(el.scrollLeft > 0);
    setCanScrollRight(el.scrollLeft + el.clientWidth < el.scrollWidth - 1);
  }, []);

  useEffect(() => {
    const el = scrollContainerRef.current;
    if (!el) return;
    checkScroll();
    el.addEventListener("scroll", checkScroll);
    const ro = new ResizeObserver(checkScroll);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", checkScroll);
      ro.disconnect();
    };
  }, [checkScroll, openTabs.length]);

  const scrollTabs = useCallback((dir: "left" | "right") => {
    const el = scrollContainerRef.current;
    if (!el) return;
    el.scrollBy({ left: dir === "left" ? -150 : 150, behavior: "smooth" });
  }, []);

  const pinnedTabs = openTabs.filter((tab) => tab.pinned).sort((a, b) => a.order - b.order);
  const unpinnedTabs = openTabs.filter((tab) => !tab.pinned).sort((a, b) => a.order - b.order);
  const sortedTabs = [...pinnedTabs, ...unpinnedTabs];

  return (
    <div className="flex items-center h-[38px] bg-surface-secondary border-b border-border-subtle no-select shrink-0">
      {canScrollLeft && (
        <button
          onClick={() => scrollTabs("left")}
          className="flex items-center justify-center w-6 h-8 text-text-secondary hover:text-text-primary hover:bg-surface-elevated transition-colors shrink-0"
        >
          <ChevronLeft size={14} />
        </button>
      )}
      <div
        ref={scrollContainerRef}
        className="flex items-center flex-1 overflow-x-auto scrollbar-none px-1 gap-0.5"
        role="tablist"
      >
        {sortedTabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            onMouseDown={(e) => {
              if (e.button === 1) {
                e.preventDefault();
                closeTab(tab.id);
              }
            }}
            onContextMenu={(e) => {
              e.preventDefault();
              setTabContextMenu({ x: e.clientX, y: e.clientY, tabId: tab.id });
            }}
            role="tab"
            aria-selected={tab.id === activeTabId}
            className={clsx(
              "group flex items-center gap-1.5 px-3 h-8 rounded-t text-xs whitespace-nowrap transition-colors",
              tab.pinned ? "min-w-[32px] justify-center" : "min-w-[120px] max-w-[240px]",
              tab.id === activeTabId
                ? "bg-surface-primary text-text-primary border-t-2 border-t-accent-primary"
                : "text-text-secondary hover:bg-surface-elevated hover:text-text-primary border-t-2 border-t-transparent"
            )}
          >
            <span className="text-sm">{SESSION_TYPE_ICONS[tab.sessionType] ?? "⌨"}</span>
            {!tab.pinned && (
              <span className="truncate flex-1 text-left">{tab.title}</span>
            )}
            <StatusDot status={tab.status} />
            {!tab.pinned && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  closeTab(tab.id);
                }}
                className="opacity-0 group-hover:opacity-100 hover:bg-surface-elevated rounded p-0.5 transition-opacity"
                data-tooltip={t("tabs.closeTab")}
              >
                <X size={12} />
              </button>
            )}
          </button>
        ))}
      </div>
      {canScrollRight && (
        <button
          onClick={() => scrollTabs("right")}
          className="flex items-center justify-center w-6 h-8 text-text-secondary hover:text-text-primary hover:bg-surface-elevated transition-colors shrink-0"
        >
          <ChevronRight size={14} />
        </button>
      )}
      <div className="relative shrink-0">
        <button
          ref={newTabBtnRef}
          onClick={() => setShowNewTabDropdown((v) => !v)}
          className="flex items-center justify-center w-8 h-8 mx-1 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
          title={t("tabs.newTab")}
          data-tooltip={t("tabs.newTab")}
        >
          <Plus size={16} />
          <MoreVertical size={10} className="ml-px" />
        </button>
        {showNewTabDropdown && (
          <NewTabDropdown
            onNewLocalShell={onNewLocalShell}
            onNewSSH={onNewSSH}
            onNewSFTP={onNewSFTP}
            onClose={() => setShowNewTabDropdown(false)}
            anchorRef={newTabBtnRef}
          />
        )}
      </div>
      {/* Broadcast toggle */}
      <button
        onClick={toggleBroadcastMode}
        className={clsx(
          "flex items-center justify-center w-8 h-8 rounded transition-colors shrink-0",
          broadcastMode
            ? "bg-accent-primary/20 text-accent-primary"
            : "text-text-secondary hover:text-text-primary hover:bg-surface-elevated"
        )}
        title={t("broadcast.tooltip")}
        data-tooltip={t("broadcast.tooltip")}
      >
        <Radio size={14} />
      </button>
      {tabContextMenu && (
        <TabContextMenu
          state={tabContextMenu}
          onClose={() => setTabContextMenu(null)}
        />
      )}
    </div>
  );
}

// ── Region C: Sidebar ──────────────────────────────────────────

const SIDEBAR_MODES = [
  { mode: SidebarMode.Sessions, icon: FolderOpen, label: "sidebar.sessions" as const },
  { mode: SidebarMode.Snippets, icon: Code2, label: "sidebar.snippets" as const },
  { mode: SidebarMode.Tunnels, icon: Lock, label: "sidebar.tunnels" as const },
];

function Sidebar({
  onNewSession,
  onOpenCredentials,
}: {
  readonly onNewSession: () => void;
  readonly onOpenCredentials: () => void;
}) {
  const { t } = useTranslation();
  const sidebarMode = useAppStore((s) => s.sidebarMode);
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);
  const setSidebarCollapsed = useAppStore((s) => s.setSidebarCollapsed);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const sessions = useSessionStore((s) => s.sessions);
  const favorites = useSessionStore((s) => s.favorites);
  const openTab = useSessionStore((s) => s.openTab);
  const setSidebarMode = useAppStore((s) => s.setSidebarMode);
  const breakpoint = useBreakpoint();

  // Auto-collapse on smaller breakpoints
  useEffect(() => {
    if (breakpoint === "compact" || breakpoint === "medium") {
      setSidebarCollapsed(true);
    }
  }, [breakpoint, setSidebarCollapsed]);

  // Hide sidebar entirely on compact
  if (breakpoint === "compact") {
    return null;
  }

  return (
    <nav
      className={clsx(
        "flex shrink-0 h-full border-r border-border-subtle bg-surface-secondary transition-all",
        sidebarCollapsed ? "w-12" : breakpoint === "large" ? "w-72" : "w-60"
      )}
      style={{ transitionDuration: "var(--duration-medium)", transitionTimingFunction: "var(--ease-default)" }}
    >
      {/* Icon rail */}
      <div className="flex flex-col items-center w-12 py-2 gap-1 border-r border-border-subtle shrink-0">
        {SIDEBAR_MODES.map(({ mode, icon: Icon, label }) => (
          <button
            key={mode}
            onClick={() => {
              if (sidebarMode === mode && !sidebarCollapsed) {
                toggleSidebar();
              } else {
                setSidebarMode(mode);
                if (sidebarCollapsed) toggleSidebar();
              }
            }}
            className={clsx(
              "relative flex items-center justify-center w-9 h-9 rounded transition-colors",
              sidebarMode === mode && !sidebarCollapsed
                ? "text-accent-primary bg-surface-elevated"
                : "text-text-secondary hover:text-text-primary hover:bg-surface-elevated"
            )}
            title={t(label)}
            data-tooltip={t(label)}
          >
            {sidebarMode === mode && !sidebarCollapsed && (
              <span className="absolute left-0 top-1 bottom-1 w-0.5 rounded-r bg-accent-primary" />
            )}
            <Icon size={20} />
          </button>
        ))}
        <div className="flex-1" />
        <button
          onClick={onOpenCredentials}
          className="relative flex items-center justify-center w-9 h-9 rounded transition-colors text-text-secondary hover:text-text-primary hover:bg-surface-elevated"
          title={t("sidebar.credentials")}
          data-tooltip={t("sidebar.credentials")}
        >
          <KeyRound size={20} />
        </button>
      </div>

      {/* Content panel */}
      {!sidebarCollapsed && (
        <div className="flex flex-col flex-1 min-w-0 overflow-hidden animate-fade-in">
          <div className="flex items-center justify-between px-3 py-2 border-b border-border-subtle">
            <span className="text-xs font-semibold uppercase tracking-wider text-text-secondary">
              {t(`sidebar.${sidebarMode}`)}
            </span>
            <button
              onClick={toggleSidebar}
              className="p-1 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary"
              title={t("sidebar.collapse")}
            >
              <ChevronLeft size={14} />
            </button>
          </div>
          <div className="flex-1 overflow-y-auto px-2 py-2">
            {sidebarMode === SidebarMode.Sessions && (
              <SessionsPanel
                sessions={sessions}
                favorites={favorites}
                onSelect={openTab}
                onNewSession={onNewSession}
              />
            )}
            {sidebarMode === SidebarMode.Snippets && (
              <EmptyPanel
                icon={<Code2 size={32} className="text-text-disabled" />}
                message={t("snippets.emptyState")}
              />
            )}
            {sidebarMode === SidebarMode.Tunnels && (
              <EmptyPanel
                icon={<Lock size={32} className="text-text-disabled" />}
                message={t("statusBar.noTunnels")}
              />
            )}
          </div>
        </div>
      )}
    </nav>
  );
}

function SessionsPanel({
  sessions,
  favorites: _favorites,
  onSelect,
  onNewSession,
}: {
  sessions: import("@/types").Session[];
  favorites: string[];
  onSelect: (session: import("@/types").Session) => void;
  onNewSession?: () => void;
}) {
  const { t } = useTranslation();

  if (sessions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-8 text-center">
        <Terminal size={32} className="text-text-disabled" />
        <p className="text-xs text-text-secondary px-2">{t("sessions.emptyState")}</p>
        <button
          onClick={onNewSession}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium bg-interactive-default text-text-inverse hover:bg-interactive-hover transition-colors"
        >
          <Plus size={12} />
          {t("sessions.newSession")}
        </button>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-0.5">
      {sessions.map((session) => (
        <button
          key={session.id}
          onClick={() => onSelect(session)}
          className="flex items-center gap-2 px-2 py-1.5 rounded text-xs hover:bg-surface-elevated transition-colors text-left w-full"
        >
          <span>{SESSION_TYPE_ICONS[session.type] ?? "⌨"}</span>
          <span className="truncate flex-1 text-text-primary">{session.name}</span>
          <span className="text-text-disabled truncate text-[10px]">
            {session.connection.host}
          </span>
        </button>
      ))}
    </div>
  );
}

function EmptyPanel({ icon, message }: { icon: React.ReactNode; message: string }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-8 text-center">
      {icon}
      <p className="text-xs text-text-secondary px-2">{message}</p>
    </div>
  );
}

// ─── Region D: Session Canvas ──────────────────────────────────

function SessionCanvas() {
  const { t } = useTranslation();
  const activeTabId = useSessionStore((s) => s.activeTabId);
  const openTabs = useSessionStore((s) => s.openTabs);
  const sessions = useSessionStore((s) => s.sessions);
  const splitPane = useSessionStore((s) => s.splitPane);
  const activeTab = openTabs.find((tab) => tab.id === activeTabId);

  if (!activeTab) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-4 text-center">
        <div className="w-16 h-16 rounded-2xl bg-surface-elevated flex items-center justify-center">
          <Plus size={28} className="text-text-disabled" />
        </div>
        <div>
          <h2 className="text-lg font-medium text-text-primary mb-1">{t("emptyCanvas.title")}</h2>
          <p className="text-sm text-text-secondary">
            {t("emptyCanvas.hint", { shortcut: "Ctrl+T" })}
          </p>
        </div>
      </div>
    );
  }

  // If a split pane layout is active, render it
  if (splitPane) {
    return (
      <div className="h-full w-full bg-surface-sunken">
        <SplitPaneContainer pane={splitPane} activeTabId={activeTabId} />
      </div>
    );
  }

  return (
    <main className="h-full w-full bg-surface-sunken relative">
      {openTabs.map((tab) => {
        const session = sessions.find((s) => s.id === tab.sessionId);
        const isActive = tab.id === activeTabId;

        if (tab.sessionType === SessionType.SSH && session) {
          const username = (session.connection.protocolOptions?.["username"] as string) ?? "root";
          return (
            <div
              key={tab.id}
              className={clsx("absolute inset-0", isActive ? "z-10" : "z-0 hidden")}
            >
              <SshTerminalTab
                sessionId={tab.sessionId}
                isActive={isActive}
                host={session.connection.host}
                port={session.connection.port}
                username={username}
                auth={{ type: "password", password: (session.connection.protocolOptions?.["password"] as string) ?? "" }}
              />
            </div>
          );
        }

        return (
          <div
            key={tab.id}
            className={clsx("absolute inset-0", isActive ? "z-10" : "z-0 hidden")}
          >
            <TerminalTab
              sessionId={tab.sessionId}
              isActive={isActive}
            />
          </div>
        );
      })}
    </main>
  );
}

// ─── Region E: Bottom Panel ────────────────────────────────────

function BottomPanel() {
  const { t } = useTranslation();
  const bottomPanelMode = useAppStore((s) => s.bottomPanelMode);
  const setBottomPanelMode = useAppStore((s) => s.setBottomPanelMode);
  const toggleBottomPanel = useAppStore((s) => s.toggleBottomPanel);

  const modes = [
    { mode: BottomPanelMode.SFTP, label: t("bottomPanel.sftp") },
    { mode: BottomPanelMode.Snippets, label: t("bottomPanel.snippets") },
    { mode: BottomPanelMode.AuditLog, label: t("bottomPanel.auditLog") },
    { mode: BottomPanelMode.Search, label: t("bottomPanel.search") },
  ] as const;

  return (
    <aside className="flex flex-col h-[30%] min-h-[120px] border-t border-border-default bg-surface-secondary animate-slide-bottom shrink-0" aria-label="Bottom Panel">
      {/* Drag handle */}
      <div className="flex justify-center py-0.5 cursor-ns-resize">
        <div className="w-8 h-0.5 rounded bg-border-strong" />
      </div>
      {/* Mode tabs */}
      <div className="flex items-center gap-0 px-2 border-b border-border-subtle">
        {modes.map(({ mode, label }) => (
          <button
            key={mode}
            onClick={() => setBottomPanelMode(mode)}
            className={clsx(
              "px-3 py-1.5 text-xs transition-colors border-b-2",
              bottomPanelMode === mode
                ? "text-text-primary border-b-accent-primary"
                : "text-text-secondary hover:text-text-primary border-b-transparent"
            )}
          >
            {label}
          </button>
        ))}
        <div className="flex-1" />
        <button
          onClick={toggleBottomPanel}
          className="p-1 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary"
        >
          <X size={14} />
        </button>
      </div>
      {/* Content */}
      <div className="flex-1 overflow-auto p-3 text-xs text-text-secondary">
        {bottomPanelMode === BottomPanelMode.SFTP && (
          <SftpBrowser />
        )}
        {bottomPanelMode === BottomPanelMode.Snippets && (
          <EmptyPanel
            icon={<Code2 size={24} className="text-text-disabled" />}
            message={t("snippets.emptyState")}
          />
        )}
        {bottomPanelMode === BottomPanelMode.AuditLog && (
          <div className="text-text-disabled text-center py-4">
            {t("sessions.noResults")}
          </div>
        )}
        {bottomPanelMode === BottomPanelMode.Search && (
          <div className="text-text-disabled text-center py-4">
            {t("sessions.noResults")}
          </div>
        )}
      </div>
    </aside>
  );
}

// ─── Region F: Status Bar ──────────────────────────────────────

function StatusBar({
  onOpenHelp,
  onOpenShortcuts,
  onStartTour,
}: {
  readonly onOpenHelp: () => void;
  readonly onOpenShortcuts: () => void;
  readonly onStartTour: (tourId: string) => void;
}) {
  const { t } = useTranslation();
  const activeProfileId = useAppStore((s) => s.activeProfileId);
  const profiles = useAppStore((s) => s.profiles);
  const activeTabId = useSessionStore((s) => s.activeTabId);
  const openTabs = useSessionStore((s) => s.openTabs);
  const terminals = useTerminalStore((s) => s.terminals);
  const activeTab = openTabs.find((tab) => tab.id === activeTabId);
  const activeProfile = profiles.find((p) => p.id === activeProfileId);

  // Resolve terminal dimensions for the active tab
  let termCols = 80;
  let termRows = 24;
  if (activeTab) {
    for (const term of terminals.values()) {
      if (term.sessionId === activeTab.sessionId) {
        termCols = term.cols;
        termRows = term.rows;
        break;
      }
    }
  }

  return (
    <footer className="flex items-center h-7 px-3 bg-surface-primary border-t border-border-subtle text-[11px] text-text-secondary no-select shrink-0 gap-4" role="status">
      <span className="flex items-center gap-1">
        <User size={11} />
        {activeProfile?.name ?? t("statusBar.profile")}
      </span>
      {activeTab && (
        <>
          <span className="flex items-center gap-1">
            <StatusDot status={activeTab.status} />
            {t(`status.${activeTab.status}`)}
          </span>
          <span>{t("statusBar.encoding")}</span>
          <span>{termCols} × {termRows}</span>
        </>
      )}
      <div className="flex-1" />
      <span className="flex items-center gap-1">
        <Lock size={11} />
        {t("statusBar.noTunnels")}
      </span>
      <HelpMenu onOpenHelp={onOpenHelp} onOpenShortcuts={onOpenShortcuts} onStartTour={onStartTour} />
    </footer>
  );
}

// ─── Compact Bottom Navigation ─────────────────────────────────

function BottomNav({
  onNewLocalShell,
}: {
  readonly onNewLocalShell: () => void;
}) {
  const { t } = useTranslation();
  const sidebarMode = useAppStore((s) => s.sidebarMode);
  const setSidebarMode = useAppStore((s) => s.setSidebarMode);

  return (
    <nav className="flex items-center justify-around h-12 bg-surface-secondary border-t border-border-subtle shrink-0">
      {SIDEBAR_MODES.map(({ mode, icon: Icon, label }) => (
        <button
          key={mode}
          onClick={() => setSidebarMode(mode)}
          className={clsx(
            "flex flex-col items-center justify-center gap-0.5 flex-1 h-full transition-colors",
            sidebarMode === mode
              ? "text-accent-primary"
              : "text-text-secondary hover:text-text-primary"
          )}
        >
          <Icon size={18} />
          <span className="text-[9px]">{t(label)}</span>
        </button>
      ))}
      <button
        onClick={onNewLocalShell}
        className="flex flex-col items-center justify-center gap-0.5 flex-1 h-full text-text-secondary hover:text-text-primary transition-colors"
      >
        <Plus size={18} />
        <span className="text-[9px]">{t("tabs.newTab")}</span>
      </button>
    </nav>
  );
}

// ─── App ───────────────────────────────────────────────────────

export default function App() {
  const { t } = useTranslation();
  const breakpoint = useBreakpoint();
  const theme = useAppStore((s) => s.theme);
  const resolvedTheme = useAppStore((s) => s.resolvedTheme);
  const setResolvedTheme = useAppStore((s) => s.setResolvedTheme);
  const bottomPanelVisible = useAppStore((s) => s.bottomPanelVisible);
  const toggleBottomPanel = useAppStore((s) => s.toggleBottomPanel);
  const setWindowDimensions = useAppStore((s) => s.setWindowDimensions);
  const settingsOpen = useAppStore((s) => s.settingsOpen);
  const setSettingsOpen = useAppStore((s) => s.setSettingsOpen);
  const setTheme = useAppStore((s) => s.setTheme);
  const firstLaunchComplete = useAppStore((s) => s.firstLaunchComplete);
  const setFirstLaunchComplete = useAppStore((s) => s.setFirstLaunchComplete);
  const lockVault = useVaultStore((s) => s.lockVault);
  const addSession = useSessionStore((s) => s.addSession);
  const openTab = useSessionStore((s) => s.openTab);
  const openTabs = useSessionStore((s) => s.openTabs);
  const activeTabId = useSessionStore((s) => s.activeTabId);
  const setActiveTab = useSessionStore((s) => s.setActiveTab);
  const closeTab = useSessionStore((s) => s.closeTab);
  const updateTabStatus = useSessionStore((s) => s.updateTabStatus);
  const [showQuickConnect, setShowQuickConnect] = useState(false);
  const [showCredentialManager, setShowCredentialManager] = useState(false);
  const [showSessionEditor, setShowSessionEditor] = useState(false);
  const [editingSession, setEditingSession] = useState<Session | null>(null);
  const [showHelpPanel, setShowHelpPanel] = useState(false);
  const [showShortcutOverlay, setShowShortcutOverlay] = useState(false);
  const [helpArticleSlug, setHelpArticleSlug] = useState<string | undefined>(undefined);
  const [showFeatureTour, setShowFeatureTour] = useState(false);
  const [activeTourId, setActiveTourId] = useState<string | undefined>(undefined);
  const [liveAnnouncement, setLiveAnnouncement] = useState("");

  // ── Task 3: Load settings from backend on startup ──
  useEffect(() => {
    async function loadSettings() {
      try {
        const settings = await invoke<{ theme?: ThemeVariant }>("settings_get");
        if (settings?.theme) {
          setTheme(settings.theme);
        }
        // If settings were loaded successfully, first launch is already done
        setFirstLaunchComplete(true);
      } catch {
        // Backend not ready or first launch — wizard will handle it
      }
    }
    loadSettings();
  }, [setTheme, setFirstLaunchComplete]);

  // ── Task 4: Listen for terminal:exit and ssh:disconnected to update tab status ──
  useEffect(() => {
    const unlistenExit = listen<{ terminal_id: string }>(
      "terminal:exit",
      (event) => {
        const terminals = useTerminalStore.getState().terminals;
        const sessionId = terminals.get(event.payload.terminal_id)?.sessionId;
        if (!sessionId) {
          return;
        }
        const tabs = useSessionStore.getState().openTabs;
        const tab = tabs.find((t) => t.sessionId === sessionId);
        if (tab) {
          updateTabStatus(tab.id, ConnectionStatus.Disconnected);
          setLiveAnnouncement(t("announcements.sessionDisconnected", { name: tab.title }));
        }
      }
    );

    const unlistenSshDisconnect = listen<{ connection_id: string; session_id?: string }>(
      "ssh:disconnected",
      (event) => {
        const tabs = useSessionStore.getState().openTabs;
        const terminals = useTerminalStore.getState().terminals;
        const sessionId = event.payload.session_id ?? terminals.get(event.payload.connection_id)?.sessionId;
        if (!sessionId) {
          return;
        }
        const tab = tabs.find((t) => t.sessionId === sessionId);
        if (tab) {
          updateTabStatus(tab.id, ConnectionStatus.Disconnected);
          setLiveAnnouncement(t("announcements.sshDisconnected", { name: tab.title }));
        }
      }
    );

    return () => {
      unlistenExit.then((fn) => fn());
      unlistenSshDisconnect.then((fn) => fn());
    };
  }, [updateTabStatus]);

  // Apply theme class based on resolvedTheme
  useEffect(() => {
    const root = document.documentElement;
    if (resolvedTheme === ThemeVariant.Light) {
      root.classList.add("light");
      root.classList.remove("dark");
    } else {
      root.classList.add("dark");
      root.classList.remove("light");
    }
  }, [resolvedTheme]);

  // OS theme auto-follow when theme === "system"
  useEffect(() => {
    if (theme !== ThemeVariant.System) return;
    const mql = globalThis.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => {
      setResolvedTheme(e.matches ? ThemeVariant.Dark : ThemeVariant.Light);
    };
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, [theme, setResolvedTheme]);

  // Track window dimensions
  useEffect(() => {
    const handleResize = () => {
      setWindowDimensions(globalThis.innerWidth, globalThis.innerHeight);
    };
    globalThis.addEventListener("resize", handleResize);
    handleResize();
    return () => globalThis.removeEventListener("resize", handleResize);
  }, [setWindowDimensions]);

  // Create a new local shell tab
  const handleNewLocalShell = useCallback(() => {
    const now = new Date().toISOString();
    const session: Session = {
      id: crypto.randomUUID(),
      name: t("sessionTypes.local_shell"),
      type: SessionType.LocalShell,
      group: "",
      tags: [],
      connection: { host: "localhost", port: 0 },
      createdAt: now,
      updatedAt: now,
      autoReconnect: false,
      keepAliveIntervalSeconds: 0,
    };
    addSession(session);
    openTab(session);
  }, [addSession, openTab]);

  // Tab navigation helpers
  const switchToTabByOffset = useCallback(
    (offset: number) => {
      const sorted = [...openTabs].sort((a, b) => a.order - b.order);
      if (sorted.length === 0) return;
      const currentIdx = sorted.findIndex((t) => t.id === activeTabId);
      const nextIdx = (currentIdx + offset + sorted.length) % sorted.length;
      setActiveTab(sorted[nextIdx].id);
    },
    [openTabs, activeTabId, setActiveTab],
  );

  const switchToTabByIndex = useCallback(
    (idx: number) => {
      const sorted = [...openTabs].sort((a, b) => a.order - b.order);
      if (idx < sorted.length) setActiveTab(sorted[idx].id);
    },
    [openTabs, setActiveTab],
  );

  // Keyboard shortcuts
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // F1: open help panel (no modifier needed)
      if (e.key === "F1") {
        e.preventDefault();
        // Context-sensitive: check for data-help-article on active element or ancestors
        const active = document.activeElement as HTMLElement | null;
        const helpEl = active?.closest("[data-help-article]") as HTMLElement | null;
        const slug = helpEl?.dataset.helpArticle;
        setHelpArticleSlug(slug);
        setShowHelpPanel((v) => !v);
        return;
      }

      const mod = e.ctrlKey || e.metaKey;
      if (!mod && !e.ctrlKey) return;

      const key = e.key.toLowerCase();
      const shift = e.shiftKey;

      const handled = handleModShortcut(key, shift, e);
      if (handled) e.preventDefault();
    },
    [toggleBottomPanel, handleNewLocalShell, activeTabId, closeTab, settingsOpen, setSettingsOpen, switchToTabByOffset, switchToTabByIndex, showHelpPanel, showShortcutOverlay],
  );

  function handleModShortcut(key: string, shift: boolean, e: KeyboardEvent): boolean {
    if (key === "j") { toggleBottomPanel(); return true; }
    if (key === "t" && !shift) { handleNewLocalShell(); return true; }
    if (key === "w" && !shift && activeTabId) { closeTab(activeTabId); return true; }
    if (key === ",") { setSettingsOpen(!settingsOpen); return true; }
    if (key === "n" && shift) { setShowQuickConnect((v) => !v); return true; }
    if (key === "/") { setShowShortcutOverlay((v) => !v); return true; }
    if (e.key === "Tab" && e.ctrlKey) { switchToTabByOffset(shift ? -1 : 1); return true; }
    if (e.key >= "1" && e.key <= "9" && !shift) { switchToTabByIndex(Number.parseInt(e.key, 10) - 1); return true; }
    return false;
  }

  useEffect(() => {
    globalThis.addEventListener("keydown", handleKeyDown);
    return () => globalThis.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  return (
    <>
      {firstLaunchComplete ? (
        <div className="flex flex-col h-screen w-screen overflow-hidden bg-surface-primary">
          <div className="sr-only" aria-live="polite" aria-atomic="true">
            {liveAnnouncement}
          </div>
          {/* Region A */}
          <TitleBar />

          <div className="flex flex-1 min-h-0">
            {/* Region C - hidden on compact via Sidebar internal logic */}
            <Sidebar
              onNewSession={() => { setEditingSession(null); setShowSessionEditor(true); }}
              onOpenCredentials={() => setShowCredentialManager(true)}
            />

            <div className="flex flex-col flex-1 min-w-0">
              {/* Region B */}
              <TabBar
                onNewLocalShell={handleNewLocalShell}
                onNewSSH={() => setShowQuickConnect(true)}
                onNewSFTP={() => {
                  const setBottomPanelMode = useAppStore.getState().setBottomPanelMode;
                  const toggleBottomPanel = useAppStore.getState().toggleBottomPanel;
                  const bottomPanelVisible = useAppStore.getState().bottomPanelVisible;
                  setBottomPanelMode(BottomPanelMode.SFTP);
                  if (!bottomPanelVisible) toggleBottomPanel();
                }}
              />

              {/* Region D */}
              <div className="flex-1 min-h-0 overflow-hidden">
                <SessionCanvas />
              </div>

              {/* Region E - only show on expanded+ breakpoints */}
              {bottomPanelVisible && breakpoint !== "compact" && <BottomPanel />}
            </div>
          </div>

          {/* Compact bottom nav */}
          {breakpoint === "compact" && (
            <BottomNav
              onNewLocalShell={handleNewLocalShell}
            />
          )}

          {/* Region F - hide on compact */}
          {breakpoint !== "compact" && <StatusBar onOpenHelp={() => setShowHelpPanel(true)} onOpenShortcuts={() => setShowShortcutOverlay(true)} onStartTour={(id) => { setActiveTourId(id); setShowFeatureTour(true); }} />}

          {/* Overlays */}
          <CommandPalette
            onNewLocalShell={handleNewLocalShell}
            onNewSSHSession={() => setShowQuickConnect(true)}
            onOpenSettings={() => setSettingsOpen(true)}
            onLockVault={() => lockVault()}
            onOpenHelp={() => setShowHelpPanel(true)}
            onOpenShortcuts={() => setShowShortcutOverlay(true)}
            onOpenHelpArticle={(slug) => { setHelpArticleSlug(slug); setShowHelpPanel(true); }}
          />
          {showQuickConnect && <QuickConnect onConnect={() => setShowQuickConnect(false)} />}
          <HelpPanel open={showHelpPanel} onClose={() => setShowHelpPanel(false)} articleSlug={helpArticleSlug} />
          <ShortcutOverlay open={showShortcutOverlay} onClose={() => setShowShortcutOverlay(false)} />
          <WhatsNewPanel />
          <TipOfTheDay />
          <VaultUnlock />
          {settingsOpen && <SettingsPanel />}
          {showCredentialManager && <CredentialManager />}
          {showSessionEditor && (
            <SessionEditor
              session={editingSession}
              onClose={() => { setShowSessionEditor(false); setEditingSession(null); }}
            />
          )}
          {showFeatureTour && activeTourId && (
            <FeatureTour
              tourId={activeTourId}
              onComplete={() => { setShowFeatureTour(false); setActiveTourId(undefined); }}
            />
          )}
        </div>
      ) : (
        <div className="h-screen w-screen overflow-hidden bg-surface-primary">
          <FirstLaunchWizard />
        </div>
      )}
    </>
  );
}
