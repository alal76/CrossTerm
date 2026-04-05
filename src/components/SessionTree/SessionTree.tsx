import { useState, useMemo, useCallback, useRef, useEffect } from "react";
import clsx from "clsx";
import {
  Search,
  Plus,
  ChevronRight,
  ChevronDown,
  FolderTree,
  Folder,
  FolderOpen,
  Terminal,
  Globe,
  Monitor,
  Server,
  Star,
  Clock,
  Copy,
  Pencil,
  Trash2,
  FolderInput,
  FolderPlus,
  Plug,
  Download,
  Wifi,
  X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useSessionStore } from "@/stores/sessionStore";
import { SessionType, ConnectionStatus } from "@/types";
import type { Session } from "@/types";

// ── Helpers ──

const SESSION_TYPE_ICON: Record<string, React.ReactNode> = {
  [SessionType.SSH]: <Globe size={14} />,
  [SessionType.SFTP]: <FolderTree size={14} />,
  [SessionType.LocalShell]: <Terminal size={14} />,
  [SessionType.RDP]: <Monitor size={14} />,
  [SessionType.VNC]: <Monitor size={14} />,
  [SessionType.Telnet]: <Wifi size={14} />,
  [SessionType.Serial]: <Plug size={14} />,
  [SessionType.CloudShell]: <Server size={14} />,
  [SessionType.WSL]: <Terminal size={14} />,
  [SessionType.KubernetesExec]: <Server size={14} />,
  [SessionType.DockerExec]: <Server size={14} />,
  [SessionType.WebConsole]: <Globe size={14} />,
  [SessionType.SCP]: <FolderTree size={14} />,
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
        "inline-block text-[8px] leading-none shrink-0",
        STATUS_DOT_COLORS[status],
        status === ConnectionStatus.Connecting && "animate-pulse"
      )}
      aria-label={status}
    >
      {STATUS_SHAPES[status]}
    </span>
  );
}

// ── Generic Context Menu ──

interface MenuItemDef {
  key: string;
  icon: React.ReactNode;
  label: string;
  action: () => void;
  danger?: boolean;
}

interface MenuDivider {
  key: string;
  divider: true;
}

type MenuEntry = MenuItemDef | MenuDivider;

function GenericContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  readonly x: number;
  readonly y: number;
  readonly items: MenuEntry[];
  readonly onClose: () => void;
}) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [onClose]);

  return (
    <div
      ref={menuRef}
      className="fixed z-[8000] min-w-[160px] bg-surface-elevated border border-border-default rounded-lg shadow-[var(--shadow-3)] py-1 overflow-hidden"
      style={{ left: x, top: y, animation: "paletteIn var(--duration-short) var(--ease-decelerate)" }}
    >
      {items.map((item) =>
        "divider" in item ? (
          <div key={item.key} className="h-px bg-border-subtle mx-2 my-1" />
        ) : (
          <button
            key={item.key}
            onClick={() => {
              item.action();
              onClose();
            }}
            className={clsx(
              "flex items-center gap-2.5 w-full px-3 py-1.5 text-xs text-left",
              "transition-colors duration-[var(--duration-micro)]",
              item.danger
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

// ── Context Menu State ──

interface SessionContextMenuState {
  kind: "session";
  x: number;
  y: number;
  session: Session;
}

interface FolderContextMenuState {
  kind: "folder";
  x: number;
  y: number;
  folderPath: string;
}

type ContextMenuState = SessionContextMenuState | FolderContextMenuState;

// ── Session Item ──

function SessionItem({
  session,
  status,
  isFavorite,
  onContextMenu,
  onClick,
  onToggleFavorite,
}: {
  readonly session: Session;
  readonly status: ConnectionStatus;
  readonly isFavorite: boolean;
  readonly onContextMenu: (e: React.MouseEvent, s: Session) => void;
  readonly onClick: (s: Session) => void;
  readonly onToggleFavorite: (id: string) => void;
}) {
  return (
    <button
      className={clsx(
        "flex items-center gap-2 w-full px-3 py-1.5 text-left group",
        "transition-colors duration-[var(--duration-micro)]",
        "hover:bg-surface-elevated rounded-md"
      )}
      onClick={() => onClick(session)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(e, session);
      }}
    >
      <span className="shrink-0 text-text-secondary">
        {SESSION_TYPE_ICON[session.type] ?? <Terminal size={14} />}
      </span>
      <span className="flex-1 truncate text-xs text-text-primary">{session.name}</span>
      <StatusDot status={status} />
      <button
        onClick={(e) => {
          e.stopPropagation();
          onToggleFavorite(session.id);
        }}
        className={clsx(
          "shrink-0 opacity-0 group-hover:opacity-100 transition-opacity duration-[var(--duration-micro)]",
          isFavorite ? "text-status-connecting opacity-100" : "text-text-disabled hover:text-status-connecting"
        )}
      >
        <Star size={12} fill={isFavorite ? "currentColor" : "none"} />
      </button>
    </button>
  );
}

// ── Hierarchical Folder Tree Node ──

interface FolderTreeNode {
  name: string;
  fullPath: string;
  children: Map<string, FolderTreeNode>;
  sessions: Session[];
}

function buildFolderTree(sessions: Session[]): FolderTreeNode {
  const root: FolderTreeNode = { name: "", fullPath: "", children: new Map(), sessions: [] };
  for (const s of sessions) {
    if (!s.group) {
      root.sessions.push(s);
      continue;
    }
    const parts = s.group.split("/");
    let node = root;
    let pathSoFar = "";
    for (const part of parts) {
      pathSoFar = pathSoFar ? `${pathSoFar}/${part}` : part;
      if (!node.children.has(part)) {
        node.children.set(part, { name: part, fullPath: pathSoFar, children: new Map(), sessions: [] });
      }
      node = node.children.get(part)!;
    }
    node.sessions.push(s);
  }
  return root;
}

function countAllSessions(node: FolderTreeNode): number {
  let count = node.sessions.length;
  for (const child of node.children.values()) {
    count += countAllSessions(child);
  }
  return count;
}

function FolderNode({
  node,
  depth,
  favorites,
  expandedFolders,
  onToggleFolder,
  onSessionClick,
  onSessionContextMenu,
  onFolderContextMenu,
  onToggleFavorite,
}: {
  readonly node: FolderTreeNode;
  readonly depth: number;
  readonly favorites: string[];
  readonly expandedFolders: Set<string>;
  readonly onToggleFolder: (path: string) => void;
  readonly onSessionClick: (s: Session) => void;
  readonly onSessionContextMenu: (e: React.MouseEvent, s: Session) => void;
  readonly onFolderContextMenu: (e: React.MouseEvent, path: string) => void;
  readonly onToggleFavorite: (id: string) => void;
}) {
  const expanded = expandedFolders.has(node.fullPath);
  const totalCount = countAllSessions(node);
  const sortedChildren = [...node.children.values()].sort((a, b) => a.name.localeCompare(b.name));
  const { t } = useTranslation();

  return (
    <div>
      <button
        className="flex items-center gap-1.5 w-full px-2 py-1 text-left hover:bg-surface-elevated rounded-md transition-colors duration-[var(--duration-micro)]"
        style={{ paddingLeft: `${8 + depth * 12}px` }}
        onClick={() => onToggleFolder(node.fullPath)}
        onContextMenu={(e) => {
          e.preventDefault();
          onFolderContextMenu(e, node.fullPath);
        }}
      >
        {expanded ? (
          <ChevronDown size={12} className="text-text-disabled shrink-0" />
        ) : (
          <ChevronRight size={12} className="text-text-disabled shrink-0" />
        )}
        {expanded ? (
          <FolderOpen size={13} className="text-text-secondary shrink-0" />
        ) : (
          <Folder size={13} className="text-text-secondary shrink-0" />
        )}
        <span className="text-xs font-medium text-text-secondary flex-1 truncate">{node.name}</span>
        <span className="text-[10px] text-text-disabled">{t("counts.sessions", { count: totalCount })}</span>
      </button>
      {expanded && (
        <div>
          {sortedChildren.map((child) => (
            <FolderNode
              key={child.fullPath}
              node={child}
              depth={depth + 1}
              favorites={favorites}
              expandedFolders={expandedFolders}
              onToggleFolder={onToggleFolder}
              onSessionClick={onSessionClick}
              onSessionContextMenu={onSessionContextMenu}
              onFolderContextMenu={onFolderContextMenu}
              onToggleFavorite={onToggleFavorite}
            />
          ))}
          {node.sessions.map((s) => (
            <div key={s.id} style={{ paddingLeft: `${(depth + 1) * 12}px` }}>
              <SessionItem
                session={s}
                status={ConnectionStatus.Idle}
                isFavorite={favorites.includes(s.id)}
                onContextMenu={onSessionContextMenu}
                onClick={onSessionClick}
                onToggleFavorite={onToggleFavorite}
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── Main SessionTree ──

interface SessionTreeProps {
  readonly onSessionSelect?: (session: Session) => void;
  readonly onSessionEdit?: (session: Session) => void;
  readonly onNewSession?: () => void;
  readonly onImport?: () => void;
}

export default function SessionTree({
  onSessionSelect,
  onSessionEdit,
  onNewSession,
  onImport,
}: SessionTreeProps) {
  const sessions = useSessionStore((s) => s.sessions);
  const favorites = useSessionStore((s) => s.favorites);
  const recentSessions = useSessionStore((s) => s.recentSessions);
  const toggleFavorite = useSessionStore((s) => s.toggleFavorite);
  const removeSession = useSessionStore((s) => s.removeSession);
  const addSession = useSessionStore((s) => s.addSession);
  const updateSession = useSessionStore((s) => s.updateSession);
  const addFolder = useSessionStore((s) => s.addFolder);
  const removeFolder = useSessionStore((s) => s.removeFolder);
  const openTab = useSessionStore((s) => s.openTab);

  const [searchQuery, setSearchQuery] = useState("");
  const [activeTags, setActiveTags] = useState<Set<string>>(new Set());
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());

  // Auto-expand all folder paths on mount
  useEffect(() => {
    const paths = new Set<string>();
    for (const s of sessions) {
      if (s.group) {
        const parts = s.group.split("/");
        let soFar = "";
        for (const part of parts) {
          soFar = soFar ? `${soFar}/${part}` : part;
          paths.add(soFar);
        }
      }
    }
    setExpandedFolders(paths);
  }, [sessions]);

  const handleToggleFolder = useCallback((path: string) => {
    setExpandedFolders((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  const handleSessionContextMenu = useCallback((e: React.MouseEvent, session: Session) => {
    setContextMenu({ kind: "session", x: e.clientX, y: e.clientY, session });
  }, []);

  const handleFolderContextMenu = useCallback((e: React.MouseEvent, folderPath: string) => {
    setContextMenu({ kind: "folder", x: e.clientX, y: e.clientY, folderPath });
  }, []);

  const handleSessionClick = useCallback(
    (session: Session) => {
      openTab(session);
      onSessionSelect?.(session);
    },
    [openTab, onSessionSelect]
  );

  const handleDuplicate = useCallback(
    (session: Session) => {
      const dup: Session = {
        ...session,
        id: crypto.randomUUID(),
        name: `${session.name} (copy)`,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };
      addSession(dup);
    },
    [addSession]
  );

  const handleMoveToFolder = useCallback(
    (session: Session) => {
      const folder = globalThis.prompt("Enter folder path (e.g. Production/AWS):", session.group);
      if (folder !== null) {
        updateSession(session.id, { group: folder });
        if (folder) addFolder(folder);
      }
    },
    [updateSession, addFolder]
  );

  const handleNewSubfolder = useCallback(
    (parentPath: string) => {
      const name = globalThis.prompt("New subfolder name:");
      if (name) {
        const newPath = parentPath ? `${parentPath}/${name}` : name;
        addFolder(newPath);
      }
    },
    [addFolder]
  );

  const handleRenameFolder = useCallback(
    (folderPath: string) => {
      const parts = folderPath.split("/");
      const oldName = parts[parts.length - 1];
      const newName = globalThis.prompt("Rename folder:", oldName);
      if (newName && newName !== oldName) {
        const parentPath = parts.slice(0, -1).join("/");
        const newPath = parentPath ? `${parentPath}/${newName}` : newName;
        // Move all sessions in this folder to the new path
        for (const s of sessions) {
          if (s.group === folderPath) {
            updateSession(s.id, { group: newPath });
          } else if (s.group.startsWith(folderPath + "/")) {
            updateSession(s.id, { group: s.group.replace(folderPath, newPath) });
          }
        }
        removeFolder(folderPath);
        addFolder(newPath);
      }
    },
    [sessions, updateSession, removeFolder, addFolder]
  );

  const handleDeleteFolder = useCallback(
    (folderPath: string) => {
      const count = sessions.filter(
        (s) => s.group === folderPath || s.group.startsWith(folderPath + "/")
      ).length;
      if (globalThis.confirm(`Delete folder "${folderPath}" and ungroup ${count} session(s)?`)) {
        for (const s of sessions) {
          if (s.group === folderPath || s.group.startsWith(folderPath + "/")) {
            updateSession(s.id, { group: "" });
          }
        }
        removeFolder(folderPath);
      }
    },
    [sessions, updateSession, removeFolder]
  );

  const handleToggleTag = useCallback((tag: string) => {
    setActiveTags((prev) => {
      const next = new Set(prev);
      if (next.has(tag)) {
        next.delete(tag);
      } else {
        next.add(tag);
      }
      return next;
    });
  }, []);

  // Extract all unique tags from sessions
  const allTags = useMemo(() => {
    const tagSet = new Set<string>();
    for (const s of sessions) {
      for (const tag of s.tags) {
        tagSet.add(tag);
      }
    }
    return [...tagSet].sort((a, b) => a.localeCompare(b));
  }, [sessions]);

  // Filter sessions
  const filtered = useMemo(() => {
    let result = sessions;
    if (searchQuery) {
      const q = searchQuery.toLowerCase();
      result = result.filter(
        (s) =>
          s.name.toLowerCase().includes(q) ||
          s.connection.host.toLowerCase().includes(q) ||
          s.tags.some((t) => t.toLowerCase().includes(q))
      );
    }
    if (activeTags.size > 0) {
      result = result.filter((s) =>
        s.tags.some((t) => activeTags.has(t))
      );
    }
    return result;
  }, [sessions, searchQuery, activeTags]);

  // Build hierarchical folder tree
  const folderTree = useMemo(() => buildFolderTree(filtered), [filtered]);

  // Favorites
  const favoriteSessions = useMemo(
    () => sessions.filter((s) => favorites.includes(s.id)),
    [sessions, favorites]
  );

  // Recents
  const recentItems = useMemo(() => {
    return recentSessions
      .slice(0, 5)
      .map((r) => sessions.find((s) => s.id === r.sessionId))
      .filter(Boolean) as Session[];
  }, [recentSessions, sessions]);

  // Build context menu items
  const contextMenuItems = useMemo((): MenuEntry[] => {
    if (!contextMenu) return [];
    if (contextMenu.kind === "session") {
      const s = contextMenu.session;
      return [
        { key: "connect", icon: <Plug size={13} />, label: "Connect", action: () => handleSessionClick(s) },
        { key: "edit", icon: <Pencil size={13} />, label: "Edit", action: () => onSessionEdit?.(s) },
        { key: "duplicate", icon: <Copy size={13} />, label: "Duplicate", action: () => handleDuplicate(s) },
        { key: "move", icon: <FolderInput size={13} />, label: "Move to Folder", action: () => handleMoveToFolder(s) },
        { key: "sep1", divider: true as const },
        { key: "delete", icon: <Trash2 size={13} />, label: "Delete", action: () => removeSession(s.id), danger: true },
      ];
    }
    // folder context menu
    const fp = contextMenu.folderPath;
    return [
      { key: "new-session", icon: <Plus size={13} />, label: "New Session", action: () => onNewSession?.() },
      { key: "new-subfolder", icon: <FolderPlus size={13} />, label: "New Subfolder", action: () => handleNewSubfolder(fp) },
      { key: "sep1", divider: true as const },
      { key: "rename", icon: <Pencil size={13} />, label: "Rename Folder", action: () => handleRenameFolder(fp) },
      { key: "delete-folder", icon: <Trash2 size={13} />, label: "Delete Folder", action: () => handleDeleteFolder(fp), danger: true },
    ];
  }, [contextMenu, handleSessionClick, onSessionEdit, handleDuplicate, handleMoveToFolder, removeSession, onNewSession, handleNewSubfolder, handleRenameFolder, handleDeleteFolder]);

  // Empty state
  if (sessions.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full p-6 text-center">
        <div className="w-16 h-16 rounded-2xl bg-surface-elevated flex items-center justify-center mb-4">
          <FolderTree size={28} className="text-text-disabled" />
        </div>
        <p className="text-sm text-text-primary mb-1">No sessions yet</p>
        <p className="text-xs text-text-secondary mb-5 max-w-[200px]">
          Create a new session or import from a file to get started.
        </p>
        <div className="flex gap-2">
          <button
            onClick={onNewSession}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            <Plus size={13} />
            New Session
          </button>
          <button
            onClick={onImport}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-short)]"
          >
            <Download size={13} />
            Import
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Search bar */}
      <div className="px-2 py-2 shrink-0">
        <div className="flex items-center gap-2 px-2.5 py-1.5 rounded-lg bg-surface-secondary border border-border-subtle focus-within:border-border-focus transition-colors duration-[var(--duration-short)]">
          <Search size={13} className="text-text-disabled shrink-0" />
          <input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Search sessions…"
            className="flex-1 bg-transparent text-xs text-text-primary placeholder:text-text-disabled outline-none"
            spellCheck={false}
          />
          {searchQuery && (
            <button onClick={() => setSearchQuery("")} className="text-text-disabled hover:text-text-secondary">
              <X size={12} />
            </button>
          )}
        </div>
      </div>

      {/* Tag filter chips */}
      {allTags.length > 0 && (
        <div className="px-2 pb-1 flex flex-wrap gap-1 shrink-0">
          {allTags.map((tag) => (
            <button
              key={tag}
              onClick={() => handleToggleTag(tag)}
              className={clsx(
                "px-2 py-0.5 rounded-full text-[10px] border transition-colors duration-[var(--duration-micro)]",
                activeTags.has(tag)
                  ? "bg-accent-primary/15 border-accent-primary text-accent-primary"
                  : "bg-surface-secondary border-border-subtle text-text-secondary hover:text-text-primary hover:border-border-default"
              )}
            >
              {tag}
            </button>
          ))}
          {activeTags.size > 0 && (
            <button
              onClick={() => setActiveTags(new Set())}
              className="px-2 py-0.5 rounded-full text-[10px] text-text-disabled hover:text-text-secondary transition-colors"
            >
              <X size={10} className="inline" /> Clear
            </button>
          )}
        </div>
      )}

      <div className="flex-1 overflow-y-auto px-1 pb-2">
        {/* Favorites */}
        {favoriteSessions.length > 0 && !searchQuery && (
          <div className="mb-3">
            <div className="px-2 py-1 text-[10px] uppercase tracking-wider text-text-disabled flex items-center gap-1">
              <Star size={10} />
              Favorites
            </div>
            <div className="flex gap-1.5 px-2 overflow-x-auto scrollbar-none pb-1">
              {favoriteSessions.map((s) => (
                <button
                  key={s.id}
                  onClick={() => handleSessionClick(s)}
                  className="shrink-0 flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-surface-elevated border border-border-subtle hover:border-border-default text-xs text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
                >
                  {SESSION_TYPE_ICON[s.type] ?? <Terminal size={12} />}
                  <span className="truncate max-w-[100px]">{s.name}</span>
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Recent */}
        {recentItems.length > 0 && !searchQuery && (
          <div className="mb-3">
            <div className="px-2 py-1 text-[10px] uppercase tracking-wider text-text-disabled flex items-center gap-1">
              <Clock size={10} />
              Recent
            </div>
            {recentItems.map((s) => (
              <SessionItem
                key={s.id}
                session={s}
                status={ConnectionStatus.Idle}
                isFavorite={favorites.includes(s.id)}
                onContextMenu={handleSessionContextMenu}
                onClick={handleSessionClick}
                onToggleFavorite={toggleFavorite}
              />
            ))}
          </div>
        )}

        {/* Hierarchical folder tree */}
        {[...folderTree.children.values()]
          .sort((a, b) => a.name.localeCompare(b.name))
          .map((child) => (
            <FolderNode
              key={child.fullPath}
              node={child}
              depth={0}
              favorites={favorites}
              expandedFolders={expandedFolders}
              onToggleFolder={handleToggleFolder}
              onSessionClick={handleSessionClick}
              onSessionContextMenu={handleSessionContextMenu}
              onFolderContextMenu={handleFolderContextMenu}
              onToggleFavorite={toggleFavorite}
            />
          ))}

        {/* Ungrouped (root-level sessions) */}
        {folderTree.sessions.length > 0 && (
          <div>
            {folderTree.children.size > 0 && (
              <div className="px-2 py-1 text-[10px] uppercase tracking-wider text-text-disabled">
                Ungrouped
              </div>
            )}
            {folderTree.sessions.map((s) => (
              <SessionItem
                key={s.id}
                session={s}
                status={ConnectionStatus.Idle}
                isFavorite={favorites.includes(s.id)}
                onContextMenu={handleSessionContextMenu}
                onClick={handleSessionClick}
                onToggleFavorite={toggleFavorite}
              />
            ))}
          </div>
        )}
      </div>

      {/* New session button */}
      <div className="shrink-0 px-2 py-2 border-t border-border-subtle">
        <button
          onClick={onNewSession}
          className="flex items-center gap-1.5 w-full px-2 py-1.5 text-xs rounded-md text-text-secondary hover:bg-surface-elevated hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
        >
          <Plus size={13} />
          New Session
        </button>
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <GenericContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          items={contextMenuItems}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}
