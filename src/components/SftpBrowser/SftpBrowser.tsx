import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import clsx from "clsx";
import {
  ChevronRight,
  Download,
  FolderOpen,
  HardDrive,
  Home,
  Loader2,
  RefreshCw,
  Server,
  Upload,
  WifiOff,
} from "lucide-react";
import { formatFileSize, formatDate } from "@/utils/formatters";

interface SftpFileEntry {
  name: string;
  is_dir: boolean;
  size: number;
  modified: string | null;
  permissions: string | null;
}

interface TransferProgress {
  transfer_id: string;
  filename: string;
  bytes_transferred: number;
  total_bytes: number;
  direction: string;
}

interface TransferComplete {
  transfer_id: string;
  success: boolean;
}

interface SftpBrowserProps {
  readonly connectionId?: string;
}

function joinPath(base: string, name: string): string {
  if (base === "/") {
    return `/${name}`;
  }
  return `${base.replace(/\/$/, "")}/${name}`;
}

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
      {parts.map((part, index) => (
        <div key={`${part}-${index}`} className="flex items-center gap-0.5 shrink-0">
          <ChevronRight size={10} className="text-text-disabled" />
          <button
            onClick={() => onNavigate(`/${parts.slice(0, index + 1).join("/")}`)}
            className="px-1 py-0.5 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors duration-[var(--duration-micro)] truncate max-w-[100px]"
          >
            {part}
          </button>
        </div>
      ))}
    </div>
  );
}

export default function SftpBrowser({ connectionId }: SftpBrowserProps) {
  const { t } = useTranslation();
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [remotePath, setRemotePath] = useState("/");
  const [entries, setEntries] = useState<SftpFileEntry[]>([]);
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [transfers, setTransfers] = useState<Map<string, TransferProgress>>(new Map());
  const [dragActive, setDragActive] = useState(false);
  const [localDropActive, setLocalDropActive] = useState(false);
  const [remoteDropActive, setRemoteDropActive] = useState(false);

  useEffect(() => {
    if (!connectionId) {
      setSessionId(null);
      return;
    }

    let cancelled = false;
    async function openSession() {
      setLoading(true);
      setError(null);
      try {
        const id = await invoke<string>("sftp_open", { connectionId });
        if (!cancelled) {
          setSessionId(id);
        }
      } catch (nextError) {
        if (!cancelled) {
          setError(String(nextError));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void openSession();
    return () => {
      cancelled = true;
    };
  }, [connectionId]);

  useEffect(() => {
    const id = sessionId;
    return () => {
      if (id) {
        void invoke("sftp_close", { sessionId: id }).catch(() => {});
      }
    };
  }, [sessionId]);

  const fetchDirectory = useCallback(async () => {
    if (!sessionId) {
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const nextEntries = await invoke<SftpFileEntry[]>("sftp_list", {
        sessionId,
        path: remotePath,
      });
      setEntries(nextEntries);
      setSelectedName(null);
    } catch (nextError) {
      setEntries([]);
      setError(String(nextError));
    } finally {
      setLoading(false);
    }
  }, [remotePath, sessionId]);

  useEffect(() => {
    void fetchDirectory();
  }, [fetchDirectory]);

  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];

    void listen<TransferProgress>("sftp:transfer_progress", (event) => {
      setTransfers((previous) => {
        const next = new Map(previous);
        next.set(event.payload.transfer_id, event.payload);
        return next;
      });
    }).then((unlisten) => unlisteners.push(unlisten));

    void listen<TransferComplete>("sftp:transfer_complete", (event) => {
      setTransfers((previous) => {
        const next = new Map(previous);
        next.delete(event.payload.transfer_id);
        return next;
      });
      if (event.payload.success) {
        void fetchDirectory();
      }
    }).then((unlisten) => unlisteners.push(unlisten));

    return () => {
      for (const unlisten of unlisteners) {
        unlisten();
      }
    };
  }, [fetchDirectory]);

  const selectedEntry = useMemo(
    () => entries.find((entry) => entry.name === selectedName) ?? null,
    [entries, selectedName],
  );

  const handleUpload = useCallback(async () => {
    if (!sessionId) {
      return;
    }
    try {
      const selected = await open({ directory: false, multiple: false });
      if (!selected || Array.isArray(selected)) {
        return;
      }
      const fileName = selected.split("/").pop() ?? selected.split("\\").pop() ?? selected;
      await invoke("sftp_upload", {
        sessionId,
        localPath: selected,
        remotePath: joinPath(remotePath, fileName),
      });
    } catch (nextError) {
      setError(String(nextError));
    }
  }, [remotePath, sessionId]);

  const handleDownload = useCallback(async () => {
    if (!sessionId || !selectedEntry || selectedEntry.is_dir) {
      return;
    }
    try {
      const destination = await save({ defaultPath: selectedEntry.name });
      if (!destination) {
        return;
      }
      await invoke("sftp_download", {
        sessionId,
        remotePath: joinPath(remotePath, selectedEntry.name),
        localPath: destination,
      });
    } catch (nextError) {
      setError(String(nextError));
    }
  }, [remotePath, selectedEntry, sessionId]);

  const handleExternalDrop = useCallback(
    async (event: React.DragEvent<HTMLDivElement>) => {
      event.preventDefault();
      setDragActive(false);

      if (!sessionId) {
        return;
      }

      const droppedFiles = Array.from(event.dataTransfer.files);
      for (const droppedFile of droppedFiles) {
        const localPath = (droppedFile as File & { path?: string }).path;
        if (!localPath) {
          continue;
        }
        try {
          await invoke("sftp_upload", {
            sessionId,
            localPath,
            remotePath: joinPath(remotePath, droppedFile.name),
          });
        } catch (nextError) {
          setError(String(nextError));
        }
      }
    },
    [remotePath, sessionId],
  );

  // ── Pane-to-pane drag handlers ──

  const handlePaneDragStart = useCallback(
    (e: React.DragEvent<HTMLTableRowElement>, fileName: string, source: "local" | "remote") => {
      e.dataTransfer.setData("application/x-sftp-file", JSON.stringify({ fileName, source, remotePath }));
      e.dataTransfer.effectAllowed = "copyMove";
    },
    [remotePath],
  );

  const handleLocalPaneDrop = useCallback(
    async (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      setLocalDropActive(false);
      const raw = e.dataTransfer.getData("application/x-sftp-file");
      if (!raw || !sessionId) return;
      try {
        const { fileName, source, remotePath: srcPath } = JSON.parse(raw) as {
          fileName: string;
          source: string;
          remotePath: string;
        };
        if (source === "remote") {
          // Download remote file to a local temp path via save dialog
          const { save: saveDialog } = await import("@tauri-apps/plugin-dialog");
          const destination = await saveDialog({ defaultPath: fileName });
          if (!destination) return;
          await invoke("sftp_download", {
            sessionId,
            remotePath: joinPath(srcPath, fileName),
            localPath: destination,
          });
        }
      } catch (nextError) {
        setError(String(nextError));
      }
    },
    [sessionId],
  );

  const handleRemotePaneDrop = useCallback(
    async (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      setRemoteDropActive(false);
      const raw = e.dataTransfer.getData("application/x-sftp-file");
      if (!raw || !sessionId) return;
      try {
        const { fileName, source } = JSON.parse(raw) as {
          fileName: string;
          source: string;
        };
        if (source === "local") {
          // Upload local file: prompt user for the local path via dialog
          const { open: openDialog } = await import("@tauri-apps/plugin-dialog");
          const selected = await openDialog({ directory: false, multiple: false, defaultPath: fileName });
          if (!selected || Array.isArray(selected)) return;
          const uploadName = selected.split("/").pop() ?? selected.split("\\").pop() ?? selected;
          await invoke("sftp_upload", {
            sessionId,
            localPath: selected,
            remotePath: joinPath(remotePath, uploadName),
          });
        }
      } catch (nextError) {
        setError(String(nextError));
      }
    },
    [remotePath, sessionId],
  );

  if (!connectionId) {
    return (
      <div
        className={clsx(
          "relative flex-1 flex flex-col border border-border-default rounded-lg overflow-hidden",
          dragActive ? "ring-2 ring-accent-primary ring-inset" : "",
        )}
        onDragOver={(event) => {
          if (!sessionId) {
            return;
          }
          event.preventDefault();
          setDragActive(true);
        }}
        onDragLeave={() => setDragActive(false)}
        onDrop={(event) => {
          void handleExternalDrop(event);
        }}
      >
        {dragActive ? (
          <div className="absolute inset-0 z-20 flex items-center justify-center bg-surface-primary/80 pointer-events-none">
            <div className="flex flex-col items-center gap-2 text-text-secondary">
              <Upload size={32} />
              <span className="text-sm font-medium">{t("sftp.dropFilesHere")}</span>
            </div>
          </div>
        ) : null}
        <div className="flex items-center gap-2 px-3 py-2 bg-surface-secondary border-b border-border-subtle shrink-0">
          <Server size={13} className="text-text-secondary" />
          <span className="text-xs font-medium text-text-secondary">{t("sftp.remote")}</span>
        </div>
        <div className="flex-1 flex flex-col items-center justify-center text-center px-6">
          <WifiOff size={28} className="text-text-disabled mb-3" />
          <p className="text-xs text-text-secondary mb-1">{t("sftp.notConnected")}</p>
          <p className="text-[10px] text-text-disabled">{t("sftp.connectToBrowse")}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full gap-4 bg-surface-primary" data-help-article="sftp-file-transfer">
      <div
        className={clsx(
          "w-56 shrink-0 flex flex-col border rounded-lg overflow-hidden transition-colors",
          localDropActive
            ? "border-dashed border-2 border-accent-primary bg-accent-primary/5"
            : "border-border-default",
        )}
        onDragOver={(e) => {
          if (e.dataTransfer.types.includes("application/x-sftp-file")) {
            e.preventDefault();
            setLocalDropActive(true);
          }
        }}
        onDragLeave={() => setLocalDropActive(false)}
        onDrop={(e) => { void handleLocalPaneDrop(e); }}
      >
        <div className="flex items-center gap-2 px-3 py-2 bg-surface-secondary border-b border-border-subtle shrink-0">
          <HardDrive size={13} className="text-text-secondary" />
          <span className="text-xs font-medium text-text-secondary">{t("sftp.local")}</span>
        </div>
        <div className="flex-1 flex flex-col items-center justify-center text-center px-6">
          <Upload size={28} className="text-text-disabled mb-3" />
          <p className="text-xs text-text-secondary mb-1">{t("sftp.uploadFiles")}</p>
          <p className="text-[10px] text-text-disabled mb-4">{t("sftp.chooseFilesDescription")}</p>
          <button
            onClick={() => {
              void handleUpload();
            }}
            disabled={!sessionId}
            className={clsx(
              "flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg transition-colors duration-[var(--duration-short)]",
              !sessionId
                ? "bg-interactive-disabled text-text-disabled cursor-not-allowed"
                : "bg-interactive-default hover:bg-interactive-hover text-text-primary",
            )}
          >
            <Upload size={13} />
            {t("sftp.chooseFiles")}
          </button>
        </div>
      </div>

      <div
        className={clsx(
          "flex-1 flex flex-col border rounded-lg overflow-hidden transition-colors",
          remoteDropActive
            ? "border-dashed border-2 border-accent-primary bg-accent-primary/5"
            : "border-border-default",
        )}
        onDragOver={(e) => {
          if (e.dataTransfer.types.includes("application/x-sftp-file")) {
            e.preventDefault();
            setRemoteDropActive(true);
          }
        }}
        onDragLeave={() => setRemoteDropActive(false)}
        onDrop={(e) => { void handleRemotePaneDrop(e); }}
      >
        <div className="flex items-center gap-2 px-3 py-2 bg-surface-secondary border-b border-border-subtle shrink-0">
          <Server size={13} className="text-text-secondary" />
          <span className="text-xs font-medium text-text-secondary">{t("sftp.remote")}</span>
          <div className="flex-1" />
          <button
            onClick={() => {
              void handleDownload();
            }}
            disabled={!selectedEntry || selectedEntry.is_dir}
            className="p-1 rounded hover:bg-surface-elevated text-text-disabled hover:text-text-secondary transition-colors duration-[var(--duration-micro)] disabled:opacity-50 disabled:cursor-not-allowed"
            title={t("sftp.download")}
          >
            <Download size={12} />
          </button>
          <button
            onClick={() => {
              void fetchDirectory();
            }}
            className={clsx(
              "p-1 rounded hover:bg-surface-elevated text-text-disabled hover:text-text-secondary transition-colors duration-[var(--duration-micro)]",
              loading ? "animate-spin" : "",
            )}
            title={t("actions.refresh")}
          >
            <RefreshCw size={12} />
          </button>
        </div>

        <div className="px-2 py-1.5 border-b border-border-subtle bg-surface-primary shrink-0">
          <Breadcrumb path={remotePath} onNavigate={setRemotePath} />
        </div>

        {error ? (
          <div className="px-3 py-2 bg-status-disconnected/10 text-[11px] text-status-disconnected border-b border-border-subtle">
            {error}
          </div>
        ) : null}

        <div className="flex-1 overflow-y-auto">
          {loading && entries.length === 0 ? (
            <div className="flex h-full items-center justify-center">
              <Loader2 size={20} className="animate-spin text-accent-primary" />
            </div>
          ) : (
            <table className="w-full text-xs">
              <thead>
                <tr className="text-left text-[10px] text-text-disabled uppercase tracking-wider border-b border-border-subtle">
                  <th className="px-3 py-1.5 font-normal">{t("sftp.name")}</th>
                  <th className="px-2 py-1.5 font-normal w-24 text-right">{t("sftp.size")}</th>
                  <th className="px-2 py-1.5 font-normal w-44 text-right">{t("sftp.modified")}</th>
                  <th className="px-2 py-1.5 font-normal w-24 text-right">{t("sftp.permissions")}</th>
                </tr>
              </thead>
              <tbody>
                {entries.map((entry) => (
                  <tr
                    key={entry.name}
                    className={clsx(
                      "cursor-pointer transition-colors duration-[var(--duration-micro)]",
                      selectedName === entry.name ? "bg-interactive-default/15" : "hover:bg-surface-elevated/50",
                    )}
                    draggable
                    onDragStart={(e) => handlePaneDragStart(e, entry.name, "remote")}
                    onClick={() => setSelectedName(entry.name)}
                    onDoubleClick={() => {
                      if (entry.is_dir) {
                        setRemotePath(joinPath(remotePath, entry.name));
                      }
                    }}
                  >
                    <td className="px-3 py-1.5">
                      <div className="flex items-center gap-2">
                        <FolderOpen size={14} className={entry.is_dir ? "text-status-connecting" : "text-text-secondary opacity-0"} />
                        <span className="text-text-primary truncate">{entry.name}</span>
                      </div>
                    </td>
                    <td className="px-2 py-1.5 text-right text-text-secondary">{entry.is_dir ? "-" : formatFileSize(entry.size)}</td>
                    <td className="px-2 py-1.5 text-right text-text-secondary">{formatDate(entry.modified)}</td>
                    <td className="px-2 py-1.5 text-right text-text-disabled font-mono text-[10px]">{entry.permissions ?? "-"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          {!loading && entries.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-8 text-center">
              <FolderOpen size={24} className="text-text-disabled mb-2" />
              <p className="text-xs text-text-secondary">{t("sftp.emptyDirectory")}</p>
            </div>
          ) : null}
        </div>

        {transfers.size > 0 ? (
          <div className="px-3 py-2 border-t border-border-subtle bg-surface-secondary space-y-1.5">
            {Array.from(transfers.values()).map((transfer) => {
              const percent = transfer.total_bytes > 0 ? (transfer.bytes_transferred / transfer.total_bytes) * 100 : 0;
              return (
                <div key={transfer.transfer_id} className="flex items-center gap-2">
                  {transfer.direction === "upload" ? (
                    <Upload size={11} className="text-accent-primary shrink-0" />
                  ) : (
                    <Download size={11} className="text-accent-secondary shrink-0" />
                  )}
                  <span className="text-[10px] text-text-secondary truncate flex-1">{transfer.filename}</span>
                  <div className="w-20 h-1.5 bg-surface-sunken rounded-full overflow-hidden shrink-0">
                    <div className="h-full bg-accent-primary rounded-full transition-all duration-300" style={{ width: `${percent}%` }} />
                  </div>
                  <span className="text-[10px] text-text-disabled w-8 text-right shrink-0">{Math.round(percent)}%</span>
                </div>
              );
            })}
          </div>
        ) : null}
      </div>
    </div>
  );
}
