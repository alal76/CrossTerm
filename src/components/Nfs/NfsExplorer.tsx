import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { Loader2, AlertTriangle, Folder, File as FileIcon, ArrowUp, Download, RefreshCw, HardDrive } from "lucide-react";
import type { NfsConfig, NfsEntry } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface NfsExplorerProps {
  readonly sessionId: string;
  readonly config: NfsConfig;
}

const READ_CHUNK_BYTES = 1024 * 1024;

function parentPath(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  const idx = trimmed.lastIndexOf("/");
  return idx <= 0 ? "" : trimmed.slice(0, idx);
}

function childPath(dir: string, name: string): string {
  return dir === "" ? name : `${dir}/${name}`;
}

async function readWholeFile(connectionId: string, path: string, size: number): Promise<Uint8Array> {
  const out = new Uint8Array(size);
  let offset = 0;
  while (offset < size) {
    const count = Math.min(READ_CHUNK_BYTES, size - offset);
    const bytes = await invoke<number[]>("nfs_read", { id: connectionId, path, offset, count });
    if (bytes.length === 0) break;
    out.set(bytes, offset);
    offset += bytes.length;
  }
  return out;
}

export default function NfsExplorer({ sessionId, config }: NfsExplorerProps) {
  const { toast } = useToast();
  const connectionIdRef = useRef<string | null>(null);
  const [connected, setConnected] = useState(false);
  const [path, setPath] = useState("");
  const [entries, setEntries] = useState<NfsEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyName, setBusyName] = useState<string | null>(null);

  const list = useCallback(async (id: string, at: string) => {
    setLoading(true);
    try {
      const result = await invoke<NfsEntry[]>("nfs_list", { id, path: at });
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

    invoke<string>("nfs_connect", { config })
      .then(async (id) => {
        if (cancelled) {
          invoke("nfs_disconnect", { id }).catch(() => {});
          return;
        }
        connectionIdRef.current = id;
        setConnected(true);
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
      const id = connectionIdRef.current;
      if (id) invoke("nfs_disconnect", { id }).catch(() => {});
      connectionIdRef.current = null;
    };
  }, [sessionId, config, list]);

  const navigate = useCallback(
    (to: string) => {
      const id = connectionIdRef.current;
      if (!id) return;
      setPath(to);
      list(id, to);
    },
    [list]
  );

  const handleDownload = useCallback(
    async (entry: NfsEntry) => {
      const id = connectionIdRef.current;
      if (!id) return;
      setBusyName(entry.name);
      try {
        const dest = await saveDialog({ defaultPath: entry.name });
        if (!dest) return;
        const bytes = await readWholeFile(id, childPath(path, entry.name), entry.size);
        await writeFile(dest, bytes);
        toast("success", `Downloaded ${entry.name}`);
      } catch (e) {
        toast("error", `Download failed: ${String(e)}`);
      } finally {
        setBusyName(null);
      }
    },
    [path, toast]
  );

  if (loading && entries.length === 0 && !error) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Mounting {config.export_path} on {config.host}…</span>
      </div>
    );
  }

  if (error && entries.length === 0 && !connected) {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Couldn't mount {config.export_path}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-3 overflow-auto p-4">
      <div className="flex items-center justify-between gap-2">
        <h2 className="flex min-w-0 items-center gap-2 text-sm font-semibold text-text-primary">
          <HardDrive size={16} className="text-accent-primary shrink-0" />
          <span className="truncate">NFS — {config.host}:{config.export_path}/{path}</span>
        </h2>
        <div className="flex shrink-0 items-center gap-1.5">
          <button
            onClick={() => connectionIdRef.current && list(connectionIdRef.current, path)}
            disabled={loading}
            className="flex items-center gap-1.5 rounded-md border border-border-default px-2 py-1 text-xs text-text-secondary hover:text-text-primary transition-colors disabled:opacity-50"
          >
            <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
            Refresh
          </button>
        </div>
      </div>

      {error && <p className="text-xs text-status-disconnected">{error}</p>}

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
                <td className="p-4 text-center text-xs text-text-disabled">Empty directory.</td>
              </tr>
            ) : (
              entries.map((entry) => (
                <tr key={entry.name} className="border-b border-border-subtle last:border-0 hover:bg-surface-secondary">
                  <td className="px-3 py-2">
                    <button
                      onClick={() => entry.file_type === "directory" && navigate(childPath(path, entry.name))}
                      disabled={entry.file_type !== "directory"}
                      className="flex items-center gap-2 text-left text-text-primary disabled:cursor-default"
                    >
                      {entry.file_type === "directory" ? (
                        <Folder size={14} className="shrink-0 text-accent-primary" />
                      ) : (
                        <FileIcon size={14} className="shrink-0 text-text-disabled" />
                      )}
                      <span className="truncate">{entry.name}</span>
                    </button>
                  </td>
                  <td className="px-3 py-2 text-right text-xs text-text-disabled">
                    {entry.file_type === "regular" ? `${entry.size} B` : ""}
                  </td>
                  <td className="px-3 py-2 text-right">
                    {entry.file_type === "regular" && (
                      <button
                        onClick={() => handleDownload(entry)}
                        disabled={busyName === entry.name}
                        title="Download"
                        className="rounded p-1 text-text-secondary hover:bg-surface-elevated hover:text-text-primary disabled:opacity-50"
                      >
                        <Download size={13} />
                      </button>
                    )}
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
