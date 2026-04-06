import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Download,
  File,
  Folder,
  Link2,
  Loader2,
  LogIn,
  RefreshCw,
  Trash2,
  Upload,
  WifiOff,
} from "lucide-react";
import type { FtpConfig, FtpEntry } from "@/types";

function entryIcon(entryType: string, size = 14) {
  switch (entryType) {
    case "directory":
      return <Folder size={size} className="text-accent-primary" />;
    case "link":
      return <Link2 size={size} className="text-text-secondary" />;
    default:
      return <File size={size} className="text-text-secondary" />;
  }
}

export default function FtpBrowser() {
  const { t } = useTranslation();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [entries, setEntries] = useState<FtpEntry[]>([]);
  const [currentPath, setCurrentPath] = useState("/");
  const [loading, setLoading] = useState(false);
  const [connecting, setConnecting] = useState(false);

  // Connection form state
  const [host, setHost] = useState("");
  const [port, setPort] = useState(21);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [useTls, setUseTls] = useState(false);
  const [passiveMode, setPassiveMode] = useState(true);

  const connect = useCallback(async () => {
    setConnecting(true);
    try {
      const config: FtpConfig = {
        host,
        port,
        username: username || undefined,
        password: password || undefined,
        use_tls: useTls,
        passive_mode: passiveMode,
      };
      const connId = await invoke<string>("ftp_connect", { config });
      setConnectionId(connId);
    } catch {
      // Connection failed
    } finally {
      setConnecting(false);
    }
  }, [host, port, username, password, useTls, passiveMode]);

  const disconnect = useCallback(async () => {
    if (!connectionId) return;
    try {
      await invoke("ftp_disconnect", { connId: connectionId });
    } catch {
      // Disconnect failed
    } finally {
      setConnectionId(null);
      setEntries([]);
      setCurrentPath("/");
    }
  }, [connectionId]);

  const loadDirectory = useCallback(
    async (path: string) => {
      if (!connectionId) return;
      setLoading(true);
      try {
        const result = await invoke<FtpEntry[]>("ftp_list", {
          connId: connectionId,
          path,
        });
        setEntries(result);
        setCurrentPath(path);
      } catch {
        setEntries([]);
      } finally {
        setLoading(false);
      }
    },
    [connectionId]
  );

  useEffect(() => {
    if (connectionId) {
      loadDirectory("/");
    }
  }, [connectionId, loadDirectory]);

  const navigateTo = useCallback(
    (name: string) => {
      const newPath =
        currentPath === "/" ? `/${name}` : `${currentPath}/${name}`;
      loadDirectory(newPath);
    },
    [currentPath, loadDirectory]
  );

  const navigateUp = useCallback(() => {
    const parent = currentPath.split("/").slice(0, -1).join("/") || "/";
    loadDirectory(parent);
  }, [currentPath, loadDirectory]);

  const handleDelete = useCallback(
    async (name: string) => {
      if (!connectionId) return;
      const fullPath =
        currentPath === "/" ? `/${name}` : `${currentPath}/${name}`;
      try {
        await invoke("ftp_delete", { connId: connectionId, path: fullPath });
        loadDirectory(currentPath);
      } catch {
        // Delete failed
      }
    },
    [connectionId, currentPath, loadDirectory]
  );

  const handleMkdir = useCallback(async () => {
    if (!connectionId) return;
    const name = prompt(t("ftp.newFolder"));
    if (!name) return;
    const fullPath =
      currentPath === "/" ? `/${name}` : `${currentPath}/${name}`;
    try {
      await invoke("ftp_mkdir", { connId: connectionId, path: fullPath });
      loadDirectory(currentPath);
    } catch {
      // Mkdir failed
    }
  }, [connectionId, currentPath, loadDirectory, t]);

  // Not connected — show connection form
  if (!connectionId) {
    return (
      <div className="flex h-full flex-col">
        {connecting ? (
          <div className="flex items-center justify-center py-16">
            <Loader2 size={24} className="animate-spin text-text-disabled" />
          </div>
        ) : (
          <div className="mx-auto flex w-full max-w-md flex-col gap-3 p-6">
            <div className="flex items-center gap-2 text-text-disabled">
              <WifiOff size={20} />
              <span className="text-sm">{t("ftp.notConnected")}</span>
            </div>
            <p className="text-xs text-text-secondary">
              {t("ftp.connectToBrowse")}
            </p>

            <div className="flex flex-col gap-2">
              <label className="text-xs font-medium text-text-secondary">
                {t("ftp.host")}
              </label>
              <input
                type="text"
                value={host}
                onChange={(e) => setHost(e.target.value)}
                placeholder="ftp.example.com"
                className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
              />
            </div>

            <div className="flex gap-2">
              <div className="flex flex-1 flex-col gap-2">
                <label className="text-xs font-medium text-text-secondary">
                  {t("ftp.port")}
                </label>
                <input
                  type="number"
                  value={port}
                  onChange={(e) => setPort(Number(e.target.value))}
                  className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
                />
              </div>
              <div className="flex flex-1 flex-col gap-2">
                <label className="text-xs font-medium text-text-secondary">
                  {t("ftp.username")}
                </label>
                <input
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  placeholder="anonymous"
                  className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
                />
              </div>
            </div>

            <div className="flex flex-col gap-2">
              <label className="text-xs font-medium text-text-secondary">
                {t("ftp.password")}
              </label>
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
              />
            </div>

            <div className="flex items-center gap-4">
              <label className="flex items-center gap-1.5 text-xs text-text-secondary">
                <input
                  type="checkbox"
                  checked={useTls}
                  onChange={(e) => setUseTls(e.target.checked)}
                  className="rounded"
                />
                {t("ftp.useTls")}
              </label>
              <label className="flex items-center gap-1.5 text-xs text-text-secondary">
                <input
                  type="checkbox"
                  checked={passiveMode}
                  onChange={(e) => setPassiveMode(e.target.checked)}
                  className="rounded"
                />
                {t("ftp.passiveMode")}
              </label>
            </div>

            <button
              onClick={connect}
              disabled={!host}
              className={clsx(
                "rounded px-3 py-1.5 text-xs font-medium",
                host
                  ? "bg-accent-primary text-text-inverse hover:bg-interactive-hover"
                  : "bg-interactive-disabled text-text-disabled"
              )}
            >
              <span className="flex items-center justify-center gap-1">
                <LogIn size={14} />
                {t("ftp.connect")}
              </span>
            </button>
          </div>
        )}
      </div>
    );
  }

  // Connected — show file browser
  return (
    <div className="flex h-full flex-col">
      {/* Toolbar */}
      <div className="flex items-center gap-2 border-b border-border-default px-3 py-2">
        <button
          onClick={navigateUp}
          disabled={currentPath === "/"}
          className="rounded p-1 text-xs hover:bg-surface-secondary disabled:opacity-50"
        >
          ..
        </button>
        <span className="flex-1 truncate font-mono text-xs text-text-secondary">
          {currentPath}
        </span>
        <button
          onClick={handleMkdir}
          className="flex items-center gap-1 rounded border border-border-default px-2 py-1 text-xs text-text-secondary hover:bg-surface-secondary"
        >
          <Folder size={12} />
          {t("ftp.newFolder")}
        </button>
        <button
          onClick={() => loadDirectory(currentPath)}
          className="rounded p-1 text-text-secondary hover:bg-surface-secondary"
        >
          <RefreshCw size={14} />
        </button>
        <button
          onClick={disconnect}
          className="flex items-center gap-1 rounded bg-status-disconnected/20 px-2 py-1 text-xs text-status-disconnected hover:bg-status-disconnected/30"
        >
          {t("ftp.disconnect")}
        </button>
      </div>

      {/* File list */}
      {loading ? (
        <div className="flex justify-center py-8">
          <Loader2 size={20} className="animate-spin text-text-disabled" />
        </div>
      ) : (
        <div className="flex-1 overflow-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-border-subtle text-left text-text-secondary">
                <th className="px-3 py-1.5">{t("sftp.name")}</th>
                <th className="px-3 py-1.5">{t("sftp.size")}</th>
                <th className="px-3 py-1.5">{t("sftp.modified")}</th>
                <th className="px-3 py-1.5">{t("sftp.permissions")}</th>
                <th className="px-3 py-1.5" />
              </tr>
            </thead>
            <tbody>
              {entries.map((entry) => (
                <tr
                  key={entry.name}
                  className="border-b border-border-subtle hover:bg-surface-secondary"
                  onDoubleClick={() => {
                    if (entry.entry_type === "directory") {
                      navigateTo(entry.name);
                    }
                  }}
                >
                  <td className="flex items-center gap-2 px-3 py-1.5">
                    {entryIcon(entry.entry_type)}
                    <span
                      className={clsx(
                        entry.entry_type === "directory" && "font-medium"
                      )}
                    >
                      {entry.name}
                    </span>
                  </td>
                  <td className="px-3 py-1.5">
                    {entry.entry_type === "file" ? entry.size : "—"}
                  </td>
                  <td className="px-3 py-1.5">{entry.modified ?? "—"}</td>
                  <td className="px-3 py-1.5 font-mono">
                    {entry.permissions ?? "—"}
                  </td>
                  <td className="flex items-center gap-1 px-3 py-1.5">
                    {entry.entry_type === "file" && (
                      <>
                        <button
                          title={t("ftp.download")}
                          className="rounded p-0.5 hover:bg-interactive-hover"
                        >
                          <Download size={14} />
                        </button>
                        <button
                          title={t("ftp.upload")}
                          className="rounded p-0.5 hover:bg-interactive-hover"
                        >
                          <Upload size={14} />
                        </button>
                      </>
                    )}
                    <button
                      onClick={() => handleDelete(entry.name)}
                      title={t("ftp.delete")}
                      className="rounded p-0.5 text-status-disconnected hover:bg-interactive-hover"
                    >
                      <Trash2 size={14} />
                    </button>
                  </td>
                </tr>
              ))}
              {entries.length === 0 && (
                <tr>
                  <td
                    colSpan={5}
                    className="px-3 py-8 text-center text-text-disabled"
                  >
                    {t("ftp.emptyDirectory")}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
