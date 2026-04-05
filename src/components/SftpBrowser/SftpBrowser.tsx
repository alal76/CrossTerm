import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import clsx from "clsx";
import {
  FolderOpen,
  File,
  FileText,
  FileCode,
  FileImage,
  ChevronRight,
  RefreshCw,
  Upload,
  Download,
  Home,
  HardDrive,
  Server,
  WifiOff,
  FolderPlus,
  Pencil,
  Trash2,
  X,
  Loader2,
  Check,
} from "lucide-react";

// ── Types ──

interface SftpFileEntry {
  name: string;
  is_dir: boolean;
  size: number;
  modified: string | null;
  permissions: string | null;
  owner: number | null;
  group: number | null;
}

interface TransferProgress {
  transfer_id: string;
  session_id: string;
  filename: string;
  bytes_transferred: number;
  total_bytes: number;
  direction: string;
}

interface TransferComplete {
  transfer_id: string;
  session_id: string;
  filename: string;
  direction: string;
  success: boolean;
  error: string | null;
}

// ── Helpers ──

function formatSize(bytes: number): string {
  if (bytes === 0) return "—";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(1)} GB`;
}

function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return d.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function joinPath(base: string, name: string): string {
  return base === "/" ? `/${name}` : `${base}/${name}`;
}

function getFileIcon(entry: SftpFileEntry): React.ReactNode {
  if (entry.is_dir) return <FolderOpen size={14} className="text-status-connecting" />;
  const ext = entry.name.split(".").pop()?.toLowerCase();
  if (ext && ["png", "jpg", "jpeg", "gif", "svg", "webp"].includes(ext))
    return <FileImage size={14} className="text-accent-primary" />;
  if (ext && ["ts", "tsx", "js", "jsx", "py", "rs", "go", "sh"].includes(ext))
    return <FileCode size={14} className="text-accent-secondary" />;
  if (ext && ["md", "txt", "log", "json", "yaml", "yml", "toml"].includes(ext))
    return <FileText size={14} className="text-text-link" />;
  return <File size={14} className="text-text-secondary" />;
}

// ── Breadcrumb ──

function Breadcrumb({
  path,
  onNavigate,
}: {
  readonly path: string;
  readonly onNavigate: (path: string) => void;
}) {
  const parts = path.split("/").filter(Boolean);

  return (
    <div className="flex items-center gap-0.5 text-xs overflow-x-auto scrollbar-none">
      <button
        onClick={() => onNavigate("/")}
        className="shrink-0 p-1 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)]"
      >
        <Home size={12} />
      </button>
      {parts.map((part, i) => (
        <div key={`${part}-${i}`} className="flex items-center gap-0.5 shrink-0">
          <ChevronRight size={10} className="text-text-disabled" />
          <button
            onClick={() => onNavigate("/" + parts.slice(0, i + 1).join("/"))}
            className="px-1 py-0.5 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)] truncate max-w-[100px]"
          >
            {part}
          </button>
        </div>
      ))}
    </div>
  );
}

// ── Context Menu ──

function ContextMenu({
  x,
  y,
  entry,
  onRename,
  onDelete,
  onDownload,
  onClose,
}: {
  readonly x: number;
  readonly y: number;
  readonly entry: SftpFileEntry;
  readonly onRename: () => void;
  readonly onDelete: () => void;
  readonly onDownload: () => void;
  readonly onClose: () => void;
}) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [onClose]);

  return (
    <div
      ref={menuRef}
      className="fixed z-[9000] min-w-[140px] bg-surface-elevated border border-border-default rounded-lg shadow-[var(--shadow-3)] py-1"
      style={{ left: x, top: y }}
    >
      {!entry.is_dir && (
        <button
          onClick={onDownload}
          className="flex items-center gap-2 w-full px-3 py-1.5 text-xs text-text-primary hover:bg-surface-secondary transition-colors"
        >
          <Download size={13} />
          Download
        </button>
      )}
      <button
        onClick={onRename}
        className="flex items-center gap-2 w-full px-3 py-1.5 text-xs text-text-primary hover:bg-surface-secondary transition-colors"
      >
        <Pencil size={13} />
        Rename
      </button>
      <button
        onClick={onDelete}
        className="flex items-center gap-2 w-full px-3 py-1.5 text-xs text-status-disconnected hover:bg-status-disconnected/10 transition-colors"
      >
        <Trash2 size={13} />
        Delete
      </button>
    </div>
  );
}

// ── Transfer Progress Bar ──

function TransferBar({
  transfers,
}: {
  readonly transfers: Map<string, TransferProgress>;
}) {
  if (transfers.size === 0) return null;

  return (
    <div className="px-3 py-2 border-t border-border-subtle bg-surface-secondary space-y-1.5">
      {Array.from(transfers.values()).map((t) => {
        const pct = t.total_bytes > 0 ? (t.bytes_transferred / t.total_bytes) * 100 : 0;
        return (
          <div key={t.transfer_id} className="flex items-center gap-2">
            {t.direction === "upload" ? (
              <Upload size={11} className="text-accent-primary shrink-0" />
            ) : (
              <Download size={11} className="text-accent-secondary shrink-0" />
            )}
            <span className="text-[10px] text-text-secondary truncate flex-1">
              {t.filename}
            </span>
            <div className="w-20 h-1.5 bg-surface-sunken rounded-full overflow-hidden shrink-0">
              <div
                className="h-full bg-accent-primary rounded-full transition-all duration-300"
                style={{ width: `${pct}%` }}
              />
            </div>
            <span className="text-[10px] text-text-disabled w-8 text-right shrink-0">
              {Math.round(pct)}%
            </span>
          </div>
        );
      })}
    </div>
  );
}

// ── Local Pane (Upload Zone) ──

function LocalPane({
  onUpload,
  uploadDisabled,
}: {
  readonly onUpload: () => void;
  readonly uploadDisabled: boolean;
}) {
  return (
    <div className="flex-1 flex flex-col border border-border-default rounded-lg overflow-hidden">
      <div className="flex items-center gap-2 px-3 py-2 bg-surface-secondary border-b border-border-subtle shrink-0">
        <HardDrive size={13} className="text-text-secondary" />
        <span className="text-xs font-medium text-text-secondary">Local</span>
      </div>
      <div className="flex-1 flex flex-col items-center justify-center text-center px-6">
        <Upload size={28} className="text-text-disabled mb-3" />
        <p className="text-xs text-text-secondary mb-1">Upload files</p>
        <p className="text-[10px] text-text-disabled mb-4">
          Select files from your computer to upload to the remote server.
        </p>
        <button
          onClick={onUpload}
          disabled={uploadDisabled}
          className={clsx(
            "flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg transition-colors duration-[var(--duration-short)]",
            uploadDisabled
              ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
              : "bg-interactive-default hover:bg-interactive-hover text-text-primary"
          )}
        >
          <Upload size={13} />
          Choose Files
        </button>
      </div>
    </div>
  );
}

// ── Remote File Pane ──

function RemotePane({
  files,
  path,
  loading,
  error,
  selected,
  renaming,
  renameValue,
  newFolderMode,
  newFolderName,
  onNavigate,
  onDoubleClick,
  onSelect,
  onContextMenu,
  onRefresh,
  onNewFolder,
  onRenameChange,
  onRenameSubmit,
  onRenameCancel,
  onNewFolderChange,
  onNewFolderSubmit,
  onNewFolderCancel,
}: {
  readonly files: SftpFileEntry[];
  readonly path: string;
  readonly loading: boolean;
  readonly error: string | null;
  readonly selected: string | null;
  readonly renaming: string | null;
  readonly renameValue: string;
  readonly newFolderMode: boolean;
  readonly newFolderName: string;
  readonly onNavigate: (path: string) => void;
  readonly onDoubleClick: (entry: SftpFileEntry) => void;
  readonly onSelect: (name: string | null) => void;
  readonly onContextMenu: (e: React.MouseEvent, entry: SftpFileEntry) => void;
  readonly onRefresh: () => void;
  readonly onNewFolder: () => void;
  readonly onRenameChange: (val: string) => void;
  readonly onRenameSubmit: () => void;
  readonly onRenameCancel: () => void;
  readonly onNewFolderChange: (val: string) => void;
  readonly onNewFolderSubmit: () => void;
  readonly onNewFolderCancel: () => void;
}) {
  return (
    <div className="flex-1 flex flex-col border border-border-default rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-3 py-2 bg-surface-secondary border-b border-border-subtle shrink-0">
        <Server size={13} className="text-text-secondary" />
        <span className="text-xs font-medium text-text-secondary">Remote</span>
        <div className="flex-1" />
        <button
          onClick={onNewFolder}
          className="p-1 rounded hover:bg-surface-elevated text-text-disabled hover:text-text-secondary transition-colors duration-[var(--duration-micro)]"
          title="New folder"
        >
          <FolderPlus size={12} />
        </button>
        <button
          onClick={onRefresh}
          className={clsx(
            "p-1 rounded hover:bg-surface-elevated text-text-disabled hover:text-text-secondary transition-colors duration-[var(--duration-micro)]",
            loading && "animate-spin"
          )}
          title="Refresh"
        >
          <RefreshCw size={12} />
        </button>
      </div>

      {/* Breadcrumb */}
      <div className="px-2 py-1.5 border-b border-border-subtle bg-surface-primary shrink-0">
        <Breadcrumb path={path} onNavigate={onNavigate} />
      </div>

      {/* Error */}
      {error && (
        <div className="px-3 py-2 bg-status-disconnected/10 text-[11px] text-status-disconnected border-b border-border-subtle">
          {error}
        </div>
      )}

      {/* Content */}
      {loading && files.length === 0 ? (
        <div className="flex-1 flex items-center justify-center">
          <Loader2 size={20} className="animate-spin text-accent-primary" />
        </div>
      ) : (
        <div className="flex-1 overflow-y-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-left text-[10px] text-text-disabled uppercase tracking-wider border-b border-border-subtle">
                <th className="px-3 py-1.5 font-normal">Name</th>
                <th className="px-2 py-1.5 font-normal w-20 text-right">Size</th>
                <th className="px-2 py-1.5 font-normal w-28 text-right">Modified</th>
                <th className="px-2 py-1.5 font-normal w-20 text-right">Perms</th>
              </tr>
            </thead>
            <tbody>
              {newFolderMode && (
                <tr className="bg-interactive-default/10">
                  <td colSpan={4} className="px-3 py-1.5">
                    <div className="flex items-center gap-2">
                      <FolderOpen size={14} className="text-status-connecting" />
                      <input
                        autoFocus
                        value={newFolderName}
                        onChange={(e) => onNewFolderChange(e.target.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter") onNewFolderSubmit();
                          if (e.key === "Escape") onNewFolderCancel();
                        }}
                        onBlur={onNewFolderCancel}
                        placeholder="New folder name"
                        className="flex-1 bg-transparent text-text-primary outline-none text-xs"
                      />
                      <button
                        onClick={onNewFolderSubmit}
                        className="p-0.5 text-status-connected hover:text-text-primary"
                      >
                        <Check size={12} />
                      </button>
                      <button
                        onClick={onNewFolderCancel}
                        className="p-0.5 text-text-disabled hover:text-text-primary"
                      >
                        <X size={12} />
                      </button>
                    </div>
                  </td>
                </tr>
              )}
              {files.map((entry) => (
                <tr
                  key={entry.name}
                  className={clsx(
                    "cursor-pointer transition-colors duration-[var(--duration-micro)]",
                    selected === entry.name
                      ? "bg-interactive-default/15"
                      : "hover:bg-surface-elevated/50"
                  )}
                  onClick={() => onSelect(entry.name)}
                  onDoubleClick={() => onDoubleClick(entry)}
                  onContextMenu={(e) => onContextMenu(e, entry)}
                >
                  <td className="px-3 py-1.5">
                    <div className="flex items-center gap-2">
                      {getFileIcon(entry)}
                      {renaming === entry.name ? (
                        <input
                          autoFocus
                          value={renameValue}
                          onChange={(e) => onRenameChange(e.target.value)}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") onRenameSubmit();
                            if (e.key === "Escape") onRenameCancel();
                          }}
                          onBlur={onRenameCancel}
                          className="flex-1 bg-transparent text-text-primary outline-none border-b border-border-focus text-xs"
                          onClick={(e) => e.stopPropagation()}
                        />
                      ) : (
                        <span className="text-text-primary truncate">{entry.name}</span>
                      )}
                    </div>
                  </td>
                  <td className="px-2 py-1.5 text-right text-text-secondary">
                    {entry.is_dir ? "—" : formatSize(entry.size)}
                  </td>
                  <td className="px-2 py-1.5 text-right text-text-secondary">
                    {formatDate(entry.modified)}
                  </td>
                  <td className="px-2 py-1.5 text-right text-text-disabled font-mono text-[10px]">
                    {entry.permissions ?? "—"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {!loading && files.length === 0 && (
            <div className="flex flex-col items-center justify-center py-8 text-center">
              <FolderOpen size={24} className="text-text-disabled mb-2" />
              <p className="text-xs text-text-secondary">Empty directory</p>
            </div>
          )}
        </div>
      )}

      {/* Footer */}
      <div className="px-3 py-1.5 border-t border-border-subtle bg-surface-secondary text-[10px] text-text-disabled shrink-0">
        {loading ? "Loading…" : `${files.length} items`}
        {selected && ` · ${selected}`}
      </div>
    </div>
  );
}

// ── Not Connected Pane ──

function NotConnectedPane() {
  return (
    <div className="flex-1 flex flex-col border border-border-default rounded-lg overflow-hidden">
      <div className="flex items-center gap-2 px-3 py-2 bg-surface-secondary border-b border-border-subtle shrink-0">
        <Server size={13} className="text-text-secondary" />
        <span className="text-xs font-medium text-text-secondary">Remote</span>
      </div>
      <div className="flex-1 flex flex-col items-center justify-center text-center px-6">
        <WifiOff size={28} className="text-text-disabled mb-3" />
        <p className="text-xs text-text-secondary mb-1">Not connected</p>
        <p className="text-[10px] text-text-disabled">
          Connect to a remote server to browse files.
        </p>
      </div>
    </div>
  );
}

// ── Main Component ──

interface SftpBrowserProps {
  readonly connectionId?: string;
}

export default function SftpBrowser({ connectionId }: SftpBrowserProps) {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [remotePath, setRemotePath] = useState("/");
  const [remoteFiles, setRemoteFiles] = useState<SftpFileEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    entry: SftpFileEntry;
  } | null>(null);
  const [transfers, setTransfers] = useState<Map<string, TransferProgress>>(
    new Map()
  );
  const [renaming, setRenaming] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [newFolderMode, setNewFolderMode] = useState(false);
  const [newFolderName, setNewFolderName] = useState("");

  const connected = !!sessionId;

  // ── Open SFTP session ──

  useEffect(() => {
    if (!connectionId) return;
    let cancelled = false;

    async function openSession() {
      try {
        setLoading(true);
        setError(null);
        const id = await invoke<string>("sftp_open", { connectionId });
        if (!cancelled) {
          setSessionId(id);
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    openSession();
    return () => {
      cancelled = true;
    };
  }, [connectionId]);

  // ── Close SFTP session on unmount ──

  useEffect(() => {
    const id = sessionId;
    return () => {
      if (id) {
        invoke("sftp_close", { sessionId: id }).catch(() => {});
      }
    };
  }, [sessionId]);

  // ── Fetch directory listing ──

  const fetchDirectory = useCallback(async () => {
    if (!sessionId) return;
    setLoading(true);
    setError(null);
    try {
      const files = await invoke<SftpFileEntry[]>("sftp_list", {
        sessionId,
        path: remotePath,
      });
      setRemoteFiles(files);
      setSelected(null);
    } catch (e) {
      setError(String(e));
      setRemoteFiles([]);
    } finally {
      setLoading(false);
    }
  }, [sessionId, remotePath]);

  useEffect(() => {
    fetchDirectory();
  }, [fetchDirectory]);

  // ── Transfer event listeners ──

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    listen<TransferProgress>("sftp:transfer_progress", (event) => {
      setTransfers((prev) => {
        const next = new Map(prev);
        next.set(event.payload.transfer_id, event.payload);
        return next;
      });
    }).then((fn) => unlisteners.push(fn));

    listen<TransferComplete>("sftp:transfer_complete", (event) => {
      setTransfers((prev) => {
        const next = new Map(prev);
        next.delete(event.payload.transfer_id);
        return next;
      });
      if (event.payload.success) {
        fetchDirectory();
      }
    }).then((fn) => unlisteners.push(fn));

    return () => {
      unlisteners.forEach((fn) => fn());
    };
  }, [fetchDirectory]);

  // ── Navigation ──

  function navigateTo(path: string) {
    setRemotePath(path || "/");
    setContextMenu(null);
  }

  function handleDoubleClick(entry: SftpFileEntry) {
    if (entry.is_dir) {
      navigateTo(joinPath(remotePath, entry.name));
    }
  }

  // ── Context menu handlers ──

  function handleContextMenu(e: React.MouseEvent, entry: SftpFileEntry) {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, entry });
    setSelected(entry.name);
  }

  async function handleDelete(entry: SftpFileEntry) {
    const fullPath = joinPath(remotePath, entry.name);
    try {
      if (entry.is_dir) {
        await invoke("sftp_rmdir", { sessionId, path: fullPath });
      } else {
        await invoke("sftp_delete", { sessionId, path: fullPath });
      }
      await fetchDirectory();
    } catch (e) {
      setError(String(e));
    }
    setContextMenu(null);
  }

  function startRename(entry: SftpFileEntry) {
    setRenaming(entry.name);
    setRenameValue(entry.name);
    setContextMenu(null);
  }

  async function handleRenameSubmit() {
    if (!renaming || !renameValue.trim() || renameValue === renaming) {
      setRenaming(null);
      return;
    }
    const oldPath = joinPath(remotePath, renaming);
    const newPath = joinPath(remotePath, renameValue.trim());
    try {
      await invoke("sftp_rename", { sessionId, oldPath, newPath });
      await fetchDirectory();
    } catch (e) {
      setError(String(e));
    }
    setRenaming(null);
  }

  async function handleNewFolderSubmit() {
    if (!newFolderName.trim()) {
      setNewFolderMode(false);
      return;
    }
    const path = joinPath(remotePath, newFolderName.trim());
    try {
      await invoke("sftp_mkdir", { sessionId, path });
      await fetchDirectory();
    } catch (e) {
      setError(String(e));
    }
    setNewFolderMode(false);
    setNewFolderName("");
  }

  // ── Upload / Download ──

  async function handleUpload() {
    if (!sessionId) return;
    try {
      const result = await open({ multiple: false, directory: false });
      if (!result) return;
      const localPath = result;
      const filename =
        localPath.split("/").pop() ?? localPath.split("\\").pop() ?? localPath;
      const remote = joinPath(remotePath, filename);
      await invoke("sftp_upload", {
        sessionId,
        localPath,
        remotePath: remote,
      });
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleDownload(entry: SftpFileEntry) {
    if (!sessionId) return;
    try {
      const localPath = await save({ defaultPath: entry.name });
      if (!localPath) return;
      const remote = joinPath(remotePath, entry.name);
      await invoke("sftp_download", {
        sessionId,
        remotePath: remote,
        localPath,
      });
    } catch (e) {
      setError(String(e));
    }
    setContextMenu(null);
  }

  return (
    <div className="flex flex-col h-full bg-surface-primary">
      {/* Toolbar */}
      <div className="flex items-center justify-center gap-2 px-4 py-2 border-b border-border-subtle shrink-0">
        <button
          onClick={handleUpload}
          disabled={!connected}
          className={clsx(
            "flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg transition-colors duration-[var(--duration-short)]",
            connected
              ? "bg-interactive-default hover:bg-interactive-hover text-text-primary"
              : "bg-interactive-disabled text-text-disabled cursor-not-allowed"
          )}
        >
          <Upload size={13} />
          Upload
        </button>
        <button
          onClick={() => {
            const entry = remoteFiles.find((f) => f.name === selected);
            if (entry && !entry.is_dir) handleDownload(entry);
          }}
          disabled={
            !connected ||
            !selected ||
            remoteFiles.find((f) => f.name === selected)?.is_dir !== false
          }
          className={clsx(
            "flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg transition-colors duration-[var(--duration-short)]",
            connected &&
              selected &&
              remoteFiles.find((f) => f.name === selected)?.is_dir === false
              ? "bg-interactive-default hover:bg-interactive-hover text-text-primary"
              : "bg-interactive-disabled text-text-disabled cursor-not-allowed"
          )}
        >
          <Download size={13} />
          Download
        </button>
      </div>

      {/* Dual panes */}
      <div className="flex-1 flex gap-2 p-2 min-h-0">
        <LocalPane onUpload={handleUpload} uploadDisabled={!connected} />
        {connected ? (
          <RemotePane
            files={remoteFiles}
            path={remotePath}
            loading={loading}
            error={error}
            selected={selected}
            renaming={renaming}
            renameValue={renameValue}
            newFolderMode={newFolderMode}
            newFolderName={newFolderName}
            onNavigate={navigateTo}
            onDoubleClick={handleDoubleClick}
            onSelect={setSelected}
            onContextMenu={handleContextMenu}
            onRefresh={fetchDirectory}
            onNewFolder={() => setNewFolderMode(true)}
            onRenameChange={setRenameValue}
            onRenameSubmit={handleRenameSubmit}
            onRenameCancel={() => setRenaming(null)}
            onNewFolderChange={setNewFolderName}
            onNewFolderSubmit={handleNewFolderSubmit}
            onNewFolderCancel={() => {
              setNewFolderMode(false);
              setNewFolderName("");
            }}
          />
        ) : (
          <NotConnectedPane />
        )}
      </div>

      {/* Transfer progress */}
      <TransferBar transfers={transfers} />

      {/* Context Menu */}
      {contextMenu && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          entry={contextMenu.entry}
          onRename={() => startRename(contextMenu.entry)}
          onDelete={() => handleDelete(contextMenu.entry)}
          onDownload={() => handleDownload(contextMenu.entry)}
          onClose={() => setContextMenu(null)}
        />
      )}
    </div>
  );
}
