import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Key,
  Import,
  Download,
  Trash2,
  Plus,
  Minus,
  ShieldCheck,
  FileKey2,
  RefreshCw,
  Loader2,
  AlertCircle,
} from "lucide-react";
import type { SshKeyInfo, AgentKey, CertificateInfo } from "@/types";

// ── Key Manager Component ──

export default function KeyManager() {
  const { t } = useTranslation();
  const [keys, setKeys] = useState<SshKeyInfo[]>([]);
  const [agentKeys, setAgentKeys] = useState<AgentKey[]>([]);
  const [certificates, setCertificates] = useState<CertificateInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<"keys" | "agent" | "certs">("keys");

  const fetchKeys = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<SshKeyInfo[]>("keymgr_list_keys");
      setKeys(result);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const fetchAgentKeys = useCallback(async () => {
    try {
      const result = await invoke<AgentKey[]>("keymgr_agent_list");
      setAgentKeys(result);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const fetchCertificates = useCallback(async () => {
    try {
      const result = await invoke<CertificateInfo[]>("keymgr_cert_list");
      setCertificates(result);
    } catch (err) {
      setError(String(err));
    }
  }, []);

  useEffect(() => {
    void fetchKeys();
    void fetchAgentKeys();
    void fetchCertificates();
  }, [fetchKeys, fetchAgentKeys, fetchCertificates]);

  const handleImport = useCallback(async () => {
    try {
      await invoke<SshKeyInfo>("keymgr_import_key", {
        path: "~/.ssh/id_ed25519",
        name: `imported-${Date.now()}`,
      });
      await fetchKeys();
    } catch (err) {
      setError(String(err));
    }
  }, [fetchKeys]);

  const handleExport = useCallback(async (keyId: string) => {
    try {
      await invoke<number[]>("keymgr_export_key", {
        keyId,
        format: "openssh",
      });
    } catch (err) {
      setError(String(err));
    }
  }, []);

  const handleDelete = useCallback(async (keyId: string) => {
    try {
      await invoke("keymgr_delete_key", { keyId });
      await fetchKeys();
    } catch (err) {
      setError(String(err));
    }
  }, [fetchKeys]);

  const handleAgentAdd = useCallback(async (keyId: string) => {
    try {
      await invoke("keymgr_agent_add", { keyId, lifetime: 3600 });
      await fetchAgentKeys();
    } catch (err) {
      setError(String(err));
    }
  }, [fetchAgentKeys]);

  const handleAgentRemove = useCallback(async (fingerprint: string) => {
    try {
      await invoke("keymgr_agent_remove", { fingerprint });
      await fetchAgentKeys();
    } catch (err) {
      setError(String(err));
    }
  }, [fetchAgentKeys]);

  const handleAgentRemoveAll = useCallback(async () => {
    try {
      await invoke("keymgr_agent_remove_all");
      await fetchAgentKeys();
    } catch (err) {
      setError(String(err));
    }
  }, [fetchAgentKeys]);

  return (
    <div className="flex flex-col h-full bg-surface-primary text-text-primary">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border-default">
        <div className="flex items-center gap-2">
          <Key size={20} className="text-accent-primary" />
          <h2 className="text-base font-semibold">{t("keymgr.title")}</h2>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleImport}
            className="flex items-center gap-1 px-3 py-1.5 text-sm rounded bg-interactive-default hover:bg-interactive-hover text-text-primary"
          >
            <Import size={14} />
            {t("keymgr.import")}
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-border-default">
        {(["keys", "agent", "certs"] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={clsx(
              "px-4 py-2 text-sm font-medium border-b-2 transition-colors",
              activeTab === tab
                ? "border-accent-primary text-accent-primary"
                : "border-transparent text-text-secondary hover:text-text-primary"
            )}
          >
            {tab === "keys" && t("keymgr.title")}
            {tab === "agent" && t("keymgr.agent")}
            {tab === "certs" && t("keymgr.certificates")}
          </button>
        ))}
      </div>

      {/* Error */}
      {error && (
        <div className="flex items-center gap-2 px-4 py-2 bg-status-disconnected/10 text-status-disconnected text-sm">
          <AlertCircle size={14} />
          {error}
        </div>
      )}

      {/* Content */}
      <div className="flex-1 overflow-auto p-4">
        {(() => {
          if (loading) {
            return (
              <div className="flex items-center justify-center h-32">
                <Loader2 size={20} className="animate-spin text-text-secondary" />
              </div>
            );
          }
          switch (activeTab) {
            case "keys":
              return (
                <KeysTable
                  keys={keys}
                  onExport={handleExport}
                  onDelete={handleDelete}
                  onAgentAdd={handleAgentAdd}
                />
              );
            case "agent":
              return (
                <AgentSection
                  agentKeys={agentKeys}
                  onRemove={handleAgentRemove}
                  onRemoveAll={handleAgentRemoveAll}
                  onRefresh={fetchAgentKeys}
                />
              );
            default:
              return (
                <CertificatesSection
                  certificates={certificates}
                  onRefresh={fetchCertificates}
                />
              );
          }
        })()}
      </div>
    </div>
  );
}

// ── Keys Table ──

function KeysTable({
  keys,
  onExport,
  onDelete,
  onAgentAdd,
}: {
  readonly keys: SshKeyInfo[];
  readonly onExport: (id: string) => void;
  readonly onDelete: (id: string) => void;
  readonly onAgentAdd: (id: string) => void;
}) {
  const { t } = useTranslation();

  if (keys.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-32 text-text-secondary">
        <FileKey2 size={32} className="mb-2 opacity-50" />
        <p className="text-sm">{t("keymgr.noKeys")}</p>
      </div>
    );
  }

  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="text-left text-text-secondary border-b border-border-subtle">
          <th className="py-2 px-2 font-medium">Name</th>
          <th className="py-2 px-2 font-medium">Type</th>
          <th className="py-2 px-2 font-medium">Fingerprint</th>
          <th className="py-2 px-2 font-medium">Tags</th>
          <th className="py-2 px-2 font-medium text-right">Actions</th>
        </tr>
      </thead>
      <tbody>
        {keys.map((key) => (
          <tr
            key={key.id}
            className="border-b border-border-subtle hover:bg-surface-secondary"
          >
            <td className="py-2 px-2 font-medium">{key.name}</td>
            <td className="py-2 px-2">
              <span className="px-1.5 py-0.5 rounded bg-surface-elevated text-xs">
                {key.key_type}
              </span>
            </td>
            <td className="py-2 px-2 font-mono text-xs text-text-secondary truncate max-w-[200px]">
              {key.fingerprint}
            </td>
            <td className="py-2 px-2">
              <div className="flex gap-1">
                {key.tags.map((tag) => (
                  <span
                    key={tag}
                    className="px-1.5 py-0.5 rounded bg-accent-primary/10 text-accent-primary text-xs"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            </td>
            <td className="py-2 px-2">
              <div className="flex items-center justify-end gap-1">
                <button
                  onClick={() => onAgentAdd(key.id)}
                  className="p-1 rounded hover:bg-interactive-hover"
                  title={t("keymgr.addToAgent")}
                >
                  <Plus size={14} />
                </button>
                <button
                  onClick={() => onExport(key.id)}
                  className="p-1 rounded hover:bg-interactive-hover"
                  title={t("keymgr.export")}
                >
                  <Download size={14} />
                </button>
                <button
                  onClick={() => onDelete(key.id)}
                  className="p-1 rounded hover:bg-interactive-hover text-status-disconnected"
                  title={t("keymgr.delete")}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// ── Agent Section ──

function AgentSection({
  agentKeys,
  onRemove,
  onRemoveAll,
  onRefresh,
}: {
  readonly agentKeys: AgentKey[];
  readonly onRemove: (fingerprint: string) => void;
  readonly onRemoveAll: () => void;
  readonly onRefresh: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-text-secondary">{t("keymgr.agent")}</h3>
        <div className="flex gap-2">
          <button
            onClick={onRefresh}
            className="p-1 rounded hover:bg-interactive-hover"
          >
            <RefreshCw size={14} />
          </button>
          {agentKeys.length > 0 && (
            <button
              onClick={onRemoveAll}
              className="flex items-center gap-1 px-2 py-1 text-xs rounded bg-status-disconnected/10 text-status-disconnected hover:bg-status-disconnected/20"
            >
              <Minus size={12} />
              Remove All
            </button>
          )}
        </div>
      </div>

      {agentKeys.length === 0 ? (
        <p className="text-sm text-text-secondary">No keys loaded in SSH agent.</p>
      ) : (
        <div className="space-y-2">
          {agentKeys.map((ak) => (
            <div
              key={ak.fingerprint}
              className="flex items-center justify-between p-3 rounded bg-surface-secondary"
            >
              <div>
                <p className="text-sm font-mono">{ak.fingerprint}</p>
                <p className="text-xs text-text-secondary">
                  {ak.key_type}
                  {ak.lifetime != null && ` • ${ak.lifetime}s remaining`}
                  {ak.comment && ` • ${ak.comment}`}
                </p>
              </div>
              <button
                onClick={() => onRemove(ak.fingerprint)}
                className="p-1 rounded hover:bg-interactive-hover text-status-disconnected"
                title={t("keymgr.removeFromAgent")}
              >
                <Minus size={14} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── Certificates Section ──

function CertificatesSection({
  certificates,
  onRefresh,
}: {
  readonly certificates: CertificateInfo[];
  readonly onRefresh: () => void;
}) {
  const { t } = useTranslation();

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-text-secondary">
          {t("keymgr.certificates")}
        </h3>
        <div className="flex gap-2">
          <button
            onClick={onRefresh}
            className="p-1 rounded hover:bg-interactive-hover"
          >
            <RefreshCw size={14} />
          </button>
          <button className="flex items-center gap-1 px-3 py-1.5 text-sm rounded bg-interactive-default hover:bg-interactive-hover">
            <ShieldCheck size={14} />
            {t("keymgr.signCert")}
          </button>
        </div>
      </div>

      {certificates.length === 0 ? (
        <p className="text-sm text-text-secondary">No certificates found.</p>
      ) : (
        <div className="space-y-2">
          {certificates.map((cert) => (
            <div
              key={cert.id}
              className="p-3 rounded bg-surface-secondary"
            >
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium">
                  {cert.cert_type === "user" ? "User" : "Host"} Certificate
                </span>
                <span className="text-xs text-text-secondary">
                  Serial: {cert.serial}
                </span>
              </div>
              <div className="mt-1 text-xs text-text-secondary">
                <p>Principals: {cert.principals.join(", ")}</p>
                <p>
                  Valid: {new Date(cert.valid_after).toLocaleDateString()} –{" "}
                  {new Date(cert.valid_before).toLocaleDateString()}
                </p>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
