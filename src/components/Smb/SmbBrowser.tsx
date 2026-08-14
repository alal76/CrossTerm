import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { Loader2, AlertTriangle, Folder, File as FileIcon, ArrowUp, Download, Upload, Trash2, RefreshCw, HardDrive } from "lucide-react";
import type { SmbConfig, SmbEntry } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface SmbBrowserProps {
  readonly sessionId: string;
  readonly config: SmbConfig;
}

function parentPath(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  return idx <= 0 ? "" : trimmed.slice(0, idx);
}

/// smb_connect requires a share name that the generic Session model has no
/// field for, so when config.share is empty this renders a picker (backed
/// by smb_list_shares) instead of failing to connect with a confusing error.
function SharePicker({
  config,
  onPick,
}: {
  readonly config: SmbConfig;
  readonly onPick: (share: string) => void;
}) {
  const [shares, setShares] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<string[]>("smb_list_shares", {
      host: config.host,
      username: config.username,
      password: config.password,
    })
      .then((result) => !cancelled && setShares(result))
      .catch((e) => !cancelled && setError(String(e)));
    return () => {
      cancelled = true;
    };
  }, [config.host, config.username, config.password]);

  if (error) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't list shares on {config.host}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  if (shares === null) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Listing shares on {config.host}…</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col items-center justify-center gap-3 p-4">
      <h2 className="flex items-center gap-2 text-sm font-semibold text-text-primary">
        <HardDrive size={16} className="text-accent-primary" />
        Select a share on {config.host}
      </h2>
      {shares.length === 0 ? (
        <span className="text-sm text-text-disabled">No shares found.</span>
      ) : (
        <div className="flex w-full max-w-sm flex-col gap-1">
          {shares.map((share) => (
            <button
              key={share}
              onClick={() => onPick(share)}
              className="flex items-center gap-2 rounded-md border border-border-default px-3 py-2 text-left text-sm text-text-primary hover:bg-surface-elevated transition-colors"
            >
              <Folder size={14} className="text-accent-primary shrink-0" />
              {share}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function Browser({ sessionId, config }: SmbBrowserProps) {
  const { toast } = useToast();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [path, setPath] = useState("");
  const [entries, setEntries] = useState<SmbEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyName, setBusyName] = useState<string | null>(null);

  const list = useCallback(async (id: string, at: string) => {
    setLoading(true);
    try {
      const result = await invoke<SmbEntry[]>("smb_list_dir", { id, path: at });
      setEntries(result);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);

    invoke<string>("smb_connect", { config })
      .then(async (id) => {
        if (cancelled) {
          invoke("smb_disconnect", { id }).catch(() => {});
          return;
        }
        setConnectionId(id);
        await list(id, "");
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e));
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
      setConnectionId((id) => {
        if (id) invoke("smb_disconnect", { id }).catch(() => {});
        return null;
      });
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  const navigate = useCallback(
    (to: string) => {
      if (!connectionId) return;
      setPath(to);
      list(connectionId, to);
    },
    [connectionId, list]
  );

  const joinPath = (base: string, name: string) => (base ? `${base}/${name}` : name);

  const handleDownload = useCallback(
    async (entry: SmbEntry) => {
      if (!connectionId) return;
      setBusyName(entry.name);
      try {
        const dest = await saveDialog({ defaultPath: entry.name });
        if (!dest) return;
        await invoke("smb_get", { id: connectionId, remotePath: joinPath(path, entry.name), localPath: dest });
        toast("success", `Downloaded ${entry.name}`);
      } catch (e) {
        toast("error", `Download failed: ${String(e)}`);
      } finally {
        setBusyName(null);
      }
    },
    [connectionId, path, toast]
  );

  const handleUpload = useCallback(async () => {
    if (!connectionId) return;
    const selected = await openDialog({ multiple: false });
    if (!selected || Array.isArray(selected)) return;
    const name = selected.split(/[/\\]/).pop() ?? "upload";
    setBusyName(name);
    try {
      await invoke("smb_put", { id: connectionId, localPath: selected, remotePath: joinPath(path, name) });
      toast("success", `Uploaded ${name}`);
      await list(connectionId, path);
    } catch (e) {
      toast("error", `Upload failed: ${String(e)}`);
    } finally {
      setBusyName(null);
    }
  }, [connectionId, path, list, toast]);

  const handleDelete = useCallback(
    async (entry: SmbEntry) => {
      if (!connectionId) return;
      if (!window.confirm(`Delete ${entry.name}? This cannot be undone.`)) return;
      setBusyName(entry.name);
      try {
        await invoke("smb_delete", { id: connectionId, path: joinPath(path, entry.name) });
        toast("success", `Deleted ${entry.name}`);
        await list(connectionId, path);
      } catch (e) {
        toast("error", `Delete failed: ${String(e)}`);
      } finally {
        setBusyName(null);
      }
    },
    [connectionId, path, list, toast]
  );

  if (loading && entries.length === 0 && !error) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Connecting to {config.host}…</span>
      </div>
    );
  }

  if (error && entries.length === 0) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't reach {config.host}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-3 overflow-auto p-4">
      <div className="flex items-center justify-between gap-2">
        <h2 className="flex min-w-0 items-center gap-2 text-sm font-semibold text-text-primary">
          <HardDrive size={16} className="text-accent-primary shrink-0" />
          <span className="truncate">
            {config.share} — /{path}
          </span>
        </h2>
        <div className="flex shrink-0 items-center gap-1.5">
          <button
            onClick={handleUpload}
            className="flex items-center gap-1.5 rounded-md border border-border-default px-2 py-1 text-xs text-text-secondary hover:text-text-primary transition-colors"
          >
            <Upload size={12} />
            Upload
          </button>
          <button
            onClick={() => connectionId && list(connectionId, path)}
            disabled={loading}
            className="flex items-center gap-1.5 rounded-md border border-border-default px-2 py-1 text-xs text-text-secondary hover:text-text-primary transition-colors disabled:opacity-50"
          >
            <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
            Refresh
          </button>
        </div>
      </div>

      {path !== "" && (
        <button
          onClick={() => navigate(parentPath(path))}
          className="flex w-fit items-center gap-1.5 rounded px-2 py-1 text-xs text-text-secondary hover:bg-surface-elevated hover:text-text-primary transition-colors"
        >
          <ArrowUp size={12} />
          Up
        </button>
      )}

      <div className="overflow-hidden rounded-md border border-border-default">
        <table className="w-full text-sm">
          <tbody>
            {entries.length === 0 ? (
              <tr>
                <td className="p-4 text-center text-xs text-text-disabled">Empty folder.</td>
              </tr>
            ) : (
              entries.map((entry) => (
                <tr key={entry.name} className="border-b border-border-subtle last:border-0 hover:bg-surface-secondary">
                  <td className="px-3 py-2">
                    <button
                      onClick={() => entry.entry_type === "directory" && navigate(joinPath(path, entry.name))}
                      disabled={entry.entry_type !== "directory"}
                      className="flex items-center gap-2 text-left text-text-primary disabled:cursor-default"
                    >
                      {entry.entry_type === "directory" ? (
                        <Folder size={14} className="shrink-0 text-accent-primary" />
                      ) : (
                        <FileIcon size={14} className="shrink-0 text-text-disabled" />
                      )}
                      <span className="truncate">{entry.name}</span>
                    </button>
                  </td>
                  <td className="px-3 py-2 text-right text-xs text-text-disabled">
                    {entry.entry_type === "file" ? `${entry.size} B` : ""}
                  </td>
                  <td className="px-3 py-2 text-right">
                    <div className="flex items-center justify-end gap-1">
                      {entry.entry_type === "file" && (
                        <button
                          onClick={() => handleDownload(entry)}
                          disabled={busyName === entry.name}
                          title="Download"
                          className="rounded p-1 text-text-secondary hover:bg-surface-elevated hover:text-text-primary disabled:opacity-50"
                        >
                          <Download size={13} />
                        </button>
                      )}
                      <button
                        onClick={() => handleDelete(entry)}
                        disabled={busyName === entry.name}
                        title="Delete"
                        className="rounded p-1 text-text-secondary hover:bg-surface-elevated hover:text-status-disconnected disabled:opacity-50"
                      >
                        <Trash2 size={13} />
                      </button>
                    </div>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

export default function SmbBrowser({ sessionId, config }: SmbBrowserProps) {
  const [share, setShare] = useState(config.share);

  if (!share) {
    return <SharePicker config={config} onPick={setShare} />;
  }

  return <Browser sessionId={sessionId} config={{ ...config, share }} />;
}
