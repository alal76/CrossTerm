import { useCallback, useEffect, useState, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import clsx from "clsx";
import {
  ChevronRight,
  ChevronDown,
  Download,
  File,
  Folder,
  FolderOpen,
  Home,
  Loader2,
  Navigation,
  RefreshCw,
  Upload,
  WifiOff,
} from "lucide-react";
import { useAppStore } from "@/stores/appStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useTerminalStore } from "@/stores/terminalStore";
import { ConnectionStatus, SessionType } from "@/types";

interface SftpFileEntry {
  name: string;
  is_dir: boolean;
  size: number;
  modified: string | null;
  permissions: string | null;
}

function joinPath(base: string, name: string): string {
  if (base === "/") return `/${name}`;
  return `${base.replace(/\/$/, "")}/${name}`;
}

function parentPath(path: string): string {
  if (path === "/" || path === "") return "/";
  const parts = path.replace(/\/$/, "").split("/").filter(Boolean);
  parts.pop();
  return parts.length === 0 ? "/" : `/${parts.join("/")}`;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

export default function RemoteFileBrowser() {
  const { t } = useTranslation();
  const followTerminal = useAppStore((s) => s.remoteFilesFollowTerminal);
  const setFollowTerminal = useAppStore((s) => s.setRemoteFilesFollowTerminal);

  const activeTabId = useSessionStore((s) => s.activeTabId);
  const openTabs = useSessionStore((s) => s.openTabs);
  const terminals = useTerminalStore((s) => s.terminals);

  // Find the active SSH connection
  const activeTab = openTabs.find((tab) => tab.id === activeTabId);
  const connectionId = (() => {
    if (!activeTab || activeTab.sessionType !== SessionType.SSH) return null;
    if (activeTab.status !== ConnectionStatus.Connected) return null;
    for (const term of terminals.values()) {
      if (term.sessionId === activeTab.sessionId && term.status === ConnectionStatus.Connected) {
        return term.id;
      }
    }
    return null;
  })();

  const [sftpSessionId, setSftpSessionId] = useState<string | null>(null);
  const [remotePath, setRemotePath] = useState("/");
  const [entries, setEntries] = useState<SftpFileEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
  const [dirContents, setDirContents] = useState<Map<string, SftpFileEntry[]>>(new Map());
  const prevConnectionId = useRef<string | null>(null);

  // Open SFTP session when connection changes
  useEffect(() => {
    if (connectionId === prevConnectionId.current) return;
    prevConnectionId.current = connectionId;

    // Close old session
    if (sftpSessionId) {
      invoke("sftp_close", { sessionId: sftpSessionId }).catch(() => {});
      setSftpSessionId(null);
    }

    if (!connectionId) {
      setEntries([]);
      setRemotePath("/");
      setExpandedDirs(new Set());
      setDirContents(new Map());
      return;
    }

    let cancelled = false;
    async function openSftp() {
      setLoading(true);
      setError(null);
      try {
        const id = await invoke<string>("sftp_open", { connectionId });
        if (!cancelled) {
          setSftpSessionId(id);
          // Get home directory
          try {
            const home = await invoke<string>("ssh_exec", { connectionId, command: "echo $HOME" });
            const homePath = home.trim() || "/";
            if (!cancelled) setRemotePath(homePath);
          } catch {
            if (!cancelled) setRemotePath("/");
          }
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void openSftp();
    return () => { cancelled = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connectionId]);

  // Clean up SFTP session on unmount
  useEffect(() => {
    return () => {
      const id = sftpSessionId;
      if (id) invoke("sftp_close", { sessionId: id }).catch(() => {});
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Follow terminal path
  useEffect(() => {
    if (!followTerminal || !connectionId || !sftpSessionId) return;

    const interval = setInterval(async () => {
      try {
        // Get CWD from the remote shell
        const cwd = await invoke<string>("ssh_exec", {
          connectionId,
          command: "pwd",
        });
        const trimmed = cwd.trim();
        if (trimmed && trimmed !== remotePath) {
          setRemotePath(trimmed);
        }
      } catch {
        // ignore — connection may have closed
      }
    }, 3000);

    return () => clearInterval(interval);
  }, [followTerminal, connectionId, sftpSessionId, remotePath]);

  // Fetch directory listing
  const fetchDirectory = useCallback(async (path?: string) => {
    if (!sftpSessionId) return;
    const target = path ?? remotePath;
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<SftpFileEntry[]>("sftp_list", {
        sessionId: sftpSessionId,
        path: target,
      });
      // Sort: dirs first, then alphabetical
      result.sort((a, b) => {
        if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
        return a.name.localeCompare(b.name);
      });
      if (!path || path === remotePath) {
        setEntries(result);
      }
      return result;
    } catch (e) {
      setError(String(e));
      return [];
    } finally {
      setLoading(false);
    }
  }, [sftpSessionId, remotePath]);

  // Reload when path or session changes
  useEffect(() => {
    if (sftpSessionId) {
      void fetchDirectory();
    }
  }, [sftpSessionId, remotePath, fetchDirectory]);

  const navigateTo = useCallback((path: string) => {
    setRemotePath(path);
    setExpandedDirs(new Set());
    setDirContents(new Map());
  }, []);

  const toggleDir = useCallback(async (dirPath: string) => {
    setExpandedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(dirPath)) {
        next.delete(dirPath);
      } else {
        next.add(dirPath);
      }
      return next;
    });

    if (!expandedDirs.has(dirPath) && sftpSessionId) {
      try {
        const contents = await invoke<SftpFileEntry[]>("sftp_list", {
          sessionId: sftpSessionId,
          path: dirPath,
        });
        contents.sort((a, b) => {
          if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
          return a.name.localeCompare(b.name);
        });
        setDirContents((prev) => new Map(prev).set(dirPath, contents));
      } catch {
        // ignore
      }
    }
  }, [expandedDirs, sftpSessionId]);

  const handleUpload = useCallback(async () => {
    if (!sftpSessionId) return;
    try {
      const selected = await open({ directory: false, multiple: false });
      if (!selected || Array.isArray(selected)) return;
      const fileName = selected.split("/").pop() ?? selected.split("\\").pop() ?? selected;
      await invoke("sftp_upload", {
        sessionId: sftpSessionId,
        localPath: selected,
        remotePath: joinPath(remotePath, fileName),
      });
      void fetchDirectory();
    } catch (e) {
      setError(String(e));
    }
  }, [sftpSessionId, remotePath, fetchDirectory]);

  const handleDownload = useCallback(async (entry: SftpFileEntry) => {
    if (!sftpSessionId || entry.is_dir) return;
    try {
      const destination = await save({ defaultPath: entry.name });
      if (!destination) return;
      await invoke("sftp_download", {
        sessionId: sftpSessionId,
        remotePath: joinPath(remotePath, entry.name),
        localPath: destination,
      });
    } catch (e) {
      setError(String(e));
    }
  }, [sftpSessionId, remotePath]);

  // No active SSH connection
  if (!connectionId) {
    return (
      <div className="flex flex-col items-center justify-center gap-3 py-8 text-center">
        <WifiOff size={32} className="text-text-disabled" />
        <p className="text-xs text-text-secondary px-2">{t("remoteFiles.noConnection")}</p>
        <p className="text-[10px] text-text-disabled px-2">{t("remoteFiles.connectFirst")}</p>
      </div>
    );
  }

  // Breadcrumb path
  const pathParts = remotePath.split("/").filter(Boolean);

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* Toolbar */}
      <div className="flex items-center gap-1 px-2 py-1.5 border-b border-border-subtle shrink-0">
        <button
          onClick={() => navigateTo("/")}
          className="p-1 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
          title={t("remoteFiles.parent")}
        >
          <Home size={13} />
        </button>
        <button
          onClick={() => navigateTo(parentPath(remotePath))}
          className="p-1 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
          title={t("remoteFiles.parent")}
          disabled={remotePath === "/"}
        >
          ↑
        </button>
        <button
          onClick={() => { void fetchDirectory(); }}
          className="p-1 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
          title={t("remoteFiles.refresh")}
        >
          <RefreshCw size={13} />
        </button>
        <button
          onClick={() => { void handleUpload(); }}
          className="p-1 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
          title={t("remoteFiles.upload")}
        >
          <Upload size={13} />
        </button>
        <div className="flex-1" />
        <button
          onClick={() => setFollowTerminal(!followTerminal)}
          className={clsx(
            "p-1 rounded transition-colors",
            followTerminal
              ? "bg-accent-primary/20 text-accent-primary"
              : "text-text-secondary hover:text-text-primary hover:bg-surface-elevated"
          )}
          title={t("remoteFiles.followTerminal")}
        >
          <Navigation size={13} />
        </button>
      </div>

      {/* Path breadcrumb */}
      <div className="flex items-center gap-0.5 px-2 py-1 border-b border-border-subtle text-[10px] overflow-x-auto scrollbar-none shrink-0">
        <button
          onClick={() => navigateTo("/")}
          className="shrink-0 px-1 py-0.5 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
        >
          /
        </button>
        {pathParts.map((part, index) => (
          <div key={`${part}-${index}`} className="flex items-center gap-0.5 shrink-0">
            <ChevronRight size={8} className="text-text-disabled" />
            <button
              onClick={() => navigateTo(`/${pathParts.slice(0, index + 1).join("/")}`)}
              className="px-1 py-0.5 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors truncate max-w-[80px]"
            >
              {part}
            </button>
          </div>
        ))}
      </div>

      {/* Error display */}
      {error && (
        <div className="px-2 py-1 text-[10px] text-status-error bg-status-error/10 border-b border-border-subtle">
          {error}
        </div>
      )}

      {/* File tree */}
      <div className="flex-1 overflow-y-auto overflow-x-hidden">
        {loading && entries.length === 0 ? (
          <div className="flex items-center justify-center py-8">
            <Loader2 size={20} className="animate-spin text-text-disabled" />
          </div>
        ) : (
          <div className="py-1">
            {entries.map((entry) => (
              <FileTreeItem
                key={entry.name}
                entry={entry}
                path={joinPath(remotePath, entry.name)}
                depth={0}
                expandedDirs={expandedDirs}
                dirContents={dirContents}
                onToggleDir={toggleDir}
                onNavigate={navigateTo}
                onDownload={handleDownload}
              />
            ))}
            {entries.length === 0 && !loading && (
              <p className="text-[10px] text-text-disabled text-center py-4">
                {t("sftp.emptyDirectory")}
              </p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}

function FileTreeItem({
  entry,
  path,
  depth,
  expandedDirs,
  dirContents,
  onToggleDir,
  onNavigate,
  onDownload,
}: {
  readonly entry: SftpFileEntry;
  readonly path: string;
  readonly depth: number;
  readonly expandedDirs: Set<string>;
  readonly dirContents: Map<string, SftpFileEntry[]>;
  readonly onToggleDir: (path: string) => void;
  readonly onNavigate: (path: string) => void;
  readonly onDownload: (entry: SftpFileEntry) => void;
}) {
  const isExpanded = expandedDirs.has(path);
  const children = dirContents.get(path);

  return (
    <>
      <button
        className="flex items-center gap-1 w-full px-2 py-[3px] text-[11px] hover:bg-surface-elevated transition-colors text-left group"
        style={{ paddingLeft: `${8 + depth * 12}px` }}
        onClick={() => {
          if (entry.is_dir) {
            onToggleDir(path);
          }
        }}
        onDoubleClick={() => {
          if (entry.is_dir) {
            onNavigate(path);
          }
        }}
      >
        {entry.is_dir ? (
          <>
            {isExpanded ? (
              <ChevronDown size={10} className="text-text-disabled shrink-0" />
            ) : (
              <ChevronRight size={10} className="text-text-disabled shrink-0" />
            )}
            {isExpanded ? (
              <FolderOpen size={12} className="text-accent-secondary shrink-0" />
            ) : (
              <Folder size={12} className="text-accent-secondary shrink-0" />
            )}
          </>
        ) : (
          <>
            <span className="w-[10px] shrink-0" />
            <File size={12} className="text-text-disabled shrink-0" />
          </>
        )}
        <span className="truncate flex-1 text-text-primary">{entry.name}</span>
        {!entry.is_dir && (
          <span className="text-[9px] text-text-disabled shrink-0">{formatSize(entry.size)}</span>
        )}
        {!entry.is_dir && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onDownload(entry);
            }}
            className="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-surface-secondary text-text-disabled hover:text-text-primary transition-all"
            title="Download"
          >
            <Download size={10} />
          </button>
        )}
      </button>
      {entry.is_dir && isExpanded && children && (
        <>
          {children.map((child) => (
            <FileTreeItem
              key={child.name}
              entry={child}
              path={joinPath(path, child.name)}
              depth={depth + 1}
              expandedDirs={expandedDirs}
              dirContents={dirContents}
              onToggleDir={onToggleDir}
              onNavigate={onNavigate}
              onDownload={onDownload}
            />
          ))}
        </>
      )}
    </>
  );
}
