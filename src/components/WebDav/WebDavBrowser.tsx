import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { readFile, writeFile } from "@tauri-apps/plugin-fs";
import { Loader2, AlertTriangle, Folder, File as FileIcon, ArrowUp, Download, Upload, Trash2, RefreshCw, Globe } from "lucide-react";
import type { WebDavConfig, WebDavEntry } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface WebDavBrowserProps {
  readonly sessionId: string;
  readonly config: WebDavConfig;
}

function parentPath(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  return idx <= 0 ? "/" : trimmed.slice(0, idx);
}

export default function WebDavBrowser({ sessionId, config }: WebDavBrowserProps) {
  const { toast } = useToast();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [path, setPath] = useState("/");
  const [entries, setEntries] = useState<WebDavEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyHref, setBusyHref] = useState<string | null>(null);

  const list = useCallback(async (id: string, at: string) => {
    setLoading(true);
    try {
      const result = await invoke<WebDavEntry[]>("webdav_list", { id, path: at });
      // The requested collection itself is typically included in PROPFIND
      // depth-1 results — filter it out so the listing shows only children.
      setEntries(result.filter((e) => e.href.replace(/\/+$/, "") !== at.replace(/\/+$/, "")));
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

    invoke<string>("webdav_connect", { config })
      .then(async (id) => {
        if (cancelled) {
          invoke("webdav_disconnect", { id }).catch(() => {});
          return;
        }
        setConnectionId(id);
        await list(id, "/");
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
        if (id) invoke("webdav_disconnect", { id }).catch(() => {});
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

  const handleDownload = useCallback(
    async (entry: WebDavEntry) => {
      if (!connectionId) return;
      setBusyHref(entry.href);
      try {
        const dest = await saveDialog({ defaultPath: entry.name });
        if (!dest) return;
        const bytes = await invoke<number[]>("webdav_get", { id: connectionId, path: entry.href });
        await writeFile(dest, new Uint8Array(bytes));
        toast("success", `Downloaded ${entry.name}`);
      } catch (e) {
        toast("error", `Download failed: ${String(e)}`);
      } finally {
        setBusyHref(null);
      }
    },
    [connectionId, toast]
  );

  const handleUpload = useCallback(async () => {
    if (!connectionId) return;
    const selected = await openDialog({ multiple: false });
    if (!selected || Array.isArray(selected)) return;
    const name = selected.split(/[/\\]/).pop() ?? "upload";
    const dest = path.endsWith("/") ? `${path}${name}` : `${path}/${name}`;
    setBusyHref(dest);
    try {
      const bytes = await readFile(selected);
      await invoke("webdav_put", { id: connectionId, path: dest, content: Array.from(bytes), contentType: null });
      toast("success", `Uploaded ${name}`);
      await list(connectionId, path);
    } catch (e) {
      toast("error", `Upload failed: ${String(e)}`);
    } finally {
      setBusyHref(null);
    }
  }, [connectionId, path, list, toast]);

  const handleDelete = useCallback(
    async (entry: WebDavEntry) => {
      if (!connectionId) return;
      if (!window.confirm(`Delete ${entry.name}? This cannot be undone.`)) return;
      setBusyHref(entry.href);
      try {
        await invoke("webdav_delete", { id: connectionId, path: entry.href });
        toast("success", `Deleted ${entry.name}`);
        await list(connectionId, path);
      } catch (e) {
        toast("error", `Delete failed: ${String(e)}`);
      } finally {
        setBusyHref(null);
      }
    },
    [connectionId, path, list, toast]
  );

  if (loading && entries.length === 0 && !error) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Connecting to {config.url}…</span>
      </div>
    );
  }

  if (error && entries.length === 0) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't reach {config.url}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-3 overflow-auto p-4">
      <div className="flex items-center justify-between gap-2">
        <h2 className="flex min-w-0 items-center gap-2 text-sm font-semibold text-text-primary">
          <Globe size={16} className="text-accent-primary shrink-0" />
          <span className="truncate">WebDAV — {path}</span>
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

      {path !== "/" && (
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
                <tr key={entry.href} className="border-b border-border-subtle last:border-0 hover:bg-surface-secondary">
                  <td className="px-3 py-2">
                    <button
                      onClick={() => entry.entry_type === "collection" && navigate(entry.href)}
                      disabled={entry.entry_type !== "collection"}
                      className="flex items-center gap-2 text-left text-text-primary disabled:cursor-default"
                    >
                      {entry.entry_type === "collection" ? (
                        <Folder size={14} className="shrink-0 text-accent-primary" />
                      ) : (
                        <FileIcon size={14} className="shrink-0 text-text-disabled" />
                      )}
                      <span className="truncate">{entry.name}</span>
                    </button>
                  </td>
                  <td className="px-3 py-2 text-right text-xs text-text-disabled">
                    {entry.content_length != null ? `${entry.content_length} B` : ""}
                  </td>
                  <td className="px-3 py-2 text-right">
                    <div className="flex items-center justify-end gap-1">
                      {entry.entry_type === "file" && (
                        <button
                          onClick={() => handleDownload(entry)}
                          disabled={busyHref === entry.href}
                          title="Download"
                          className="rounded p-1 text-text-secondary hover:bg-surface-elevated hover:text-text-primary disabled:opacity-50"
                        >
                          <Download size={13} />
                        </button>
                      )}
                      <button
                        onClick={() => handleDelete(entry)}
                        disabled={busyHref === entry.href}
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
