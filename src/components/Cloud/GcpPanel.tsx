import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  ChevronRight,
  FolderOpen,
  HardDrive,
  Loader2,
  LogIn,
  Play,
  RefreshCw,
  Shield,
  Square,
  Terminal,
  Wifi,
} from "lucide-react";
import type {
  CloudProviderStatus,
  GcpConfig,
  GcpInstance,
  GcsBucket,
  GcsObject,
} from "@/types";

interface GcpPanelProps {
  readonly status: CloudProviderStatus | undefined;
}

export default function GcpPanel({ status }: GcpPanelProps) {
  const { t } = useTranslation();
  const [configs, setConfigs] = useState<GcpConfig[]>([]);
  const [selectedProject, setSelectedProject] = useState<string>("");
  const [instances, setInstances] = useState<GcpInstance[]>([]);
  const [buckets, setBuckets] = useState<GcsBucket[]>([]);
  const [objects, setObjects] = useState<GcsObject[]>([]);
  const [selectedBucket, setSelectedBucket] = useState<string | null>(null);
  const [activeSection, setActiveSection] = useState<"instances" | "storage">("instances");
  const [loading, setLoading] = useState(false);

  const loadConfigs = useCallback(async () => {
    try {
      const result = await invoke<GcpConfig[]>("cloud_gcp_list_configs");
      setConfigs(result);
      const active = result.find((c) => c.is_active);
      if (active) {
        setSelectedProject(active.project);
      } else if (result.length > 0) {
        setSelectedProject(result[0].project);
      }
    } catch {
      setConfigs([]);
    }
  }, []);

  useEffect(() => {
    if (status?.cli_status.type === "installed") {
      loadConfigs();
    }
  }, [status, loadConfigs]);

  const loadInstances = useCallback(async () => {
    if (!selectedProject) return;
    setLoading(true);
    try {
      const result = await invoke<GcpInstance[]>("cloud_gcp_list_instances", {
        project: selectedProject,
      });
      setInstances(result);
    } catch {
      setInstances([]);
    } finally {
      setLoading(false);
    }
  }, [selectedProject]);

  const loadBuckets = useCallback(async () => {
    if (!selectedProject) return;
    setLoading(true);
    try {
      const result = await invoke<GcsBucket[]>("cloud_gcp_list_buckets", {
        project: selectedProject,
      });
      setBuckets(result);
    } catch {
      setBuckets([]);
    } finally {
      setLoading(false);
    }
  }, [selectedProject]);

  const loadBucketObjects = useCallback(
    async (bucket: string) => {
      if (!selectedProject) return;
      setSelectedBucket(bucket);
      setLoading(true);
      try {
        const result = await invoke<GcsObject[]>("cloud_gcp_list_objects", {
          project: selectedProject,
          bucket,
        });
        setObjects(result);
      } catch {
        setObjects([]);
      } finally {
        setLoading(false);
      }
    },
    [selectedProject]
  );

  useEffect(() => {
    if (selectedProject) {
      if (activeSection === "instances") loadInstances();
      else loadBuckets();
    }
  }, [selectedProject, activeSection, loadInstances, loadBuckets]);

  const handleLogin = useCallback(async () => {
    try {
      await invoke("cloud_gcp_auth_login");
      loadConfigs();
    } catch {
      // Login failed
    }
  }, [loadConfigs]);

  const handleInstanceAction = useCallback(
    async (instanceName: string, zone: string, action: "start" | "stop" | "connect" | "iap") => {
      if (!selectedProject) return;
      try {
        if (action === "start") {
          await invoke("cloud_gcp_start_instance", {
            project: selectedProject,
            zone,
            instance: instanceName,
          });
          loadInstances();
        } else if (action === "stop") {
          await invoke("cloud_gcp_stop_instance", {
            project: selectedProject,
            zone,
            instance: instanceName,
          });
          loadInstances();
        } else if (action === "connect") {
          await invoke("cloud_gcp_ssh_connect", {
            project: selectedProject,
            zone,
            instance: instanceName,
          });
        } else if (action === "iap") {
          await invoke("cloud_gcp_iap_tunnel", {
            project: selectedProject,
            zone,
            instance: instanceName,
          });
        }
      } catch {
        // Action failed
      }
    },
    [selectedProject, loadInstances]
  );

  const handleCloudShell = useCallback(async () => {
    try {
      await invoke("cloud_gcp_cloud_shell");
    } catch {
      // Cloud Shell launch failed
    }
  }, []);

  if (status?.cli_status.type === "not_installed") {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-text-disabled">
        <HardDrive size={32} />
        <p className="mt-2 text-sm">{t("cloud.notInstalled")}</p>
        <p className="text-xs">Install the Google Cloud SDK to get started.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Project selector + login */}
      <div className="flex items-center gap-3">
        <label className="text-xs font-medium text-text-secondary">
          {t("cloud.selectProject")}
        </label>
        <select
          value={selectedProject}
          onChange={(e) => setSelectedProject(e.target.value)}
          className="rounded border border-border-default bg-surface-secondary px-2 py-1 text-xs text-text-primary"
        >
          {configs.map((c) => (
            <option key={c.name} value={c.project}>
              {c.project} ({c.name})
            </option>
          ))}
        </select>
        <button
          onClick={handleLogin}
          className="flex items-center gap-1 rounded bg-accent-primary px-2 py-1 text-xs text-text-inverse hover:bg-interactive-hover"
        >
          <LogIn size={12} />
          {t("cloud.login")}
        </button>
        <button
          onClick={handleCloudShell}
          className="flex items-center gap-1 rounded border border-border-default px-2 py-1 text-xs text-text-secondary hover:bg-surface-secondary"
        >
          <Terminal size={12} />
          {t("cloud.cloudShell")}
        </button>
        <button
          onClick={() => {
            if (activeSection === "instances") loadInstances();
            else loadBuckets();
          }}
          className="ml-auto flex items-center gap-1 rounded border border-border-default px-2 py-1 text-xs text-text-secondary hover:bg-surface-secondary"
        >
          <RefreshCw size={12} />
          {t("cloud.refresh")}
        </button>
      </div>

      {/* Section tabs */}
      <div className="flex items-center gap-1 border-b border-border-subtle">
        {(["instances", "storage"] as const).map((section) => (
          <button
            key={section}
            onClick={() => setActiveSection(section)}
            className={clsx(
              "border-b-2 px-3 py-1.5 text-xs font-medium transition-colors",
              activeSection === section
                ? "border-accent-primary text-accent-primary"
                : "border-transparent text-text-secondary hover:text-text-primary"
            )}
          >
            {t(`cloud.${section}`)}
          </button>
        ))}
      </div>

      {loading && (
        <div className="flex justify-center py-8">
          <Loader2 size={20} className="animate-spin text-text-disabled" />
        </div>
      )}

      {/* Instance table */}
      {!loading && activeSection === "instances" && (
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-border-subtle text-left text-text-secondary">
                <th className="px-2 py-1.5">Name</th>
                <th className="px-2 py-1.5">Zone</th>
                <th className="px-2 py-1.5">Status</th>
                <th className="px-2 py-1.5">Machine Type</th>
                <th className="px-2 py-1.5">IP</th>
                <th className="px-2 py-1.5">Actions</th>
              </tr>
            </thead>
            <tbody>
              {instances.map((inst) => (
                <tr
                  key={inst.id}
                  className="border-b border-border-subtle hover:bg-surface-secondary"
                >
                  <td className="px-2 py-1.5 font-medium">{inst.name}</td>
                  <td className="px-2 py-1.5">{inst.zone}</td>
                  <td className="px-2 py-1.5">
                    <span
                      className={clsx(
                        "inline-block rounded-full px-1.5 py-0.5 text-[10px] font-medium",
                        inst.status === "RUNNING"
                          ? "bg-status-connected/20 text-status-connected"
                          : "bg-status-disconnected/20 text-status-disconnected"
                      )}
                    >
                      {inst.status}
                    </span>
                  </td>
                  <td className="px-2 py-1.5">{inst.machine_type}</td>
                  <td className="px-2 py-1.5 font-mono">
                    {inst.external_ip ?? inst.internal_ip ?? "—"}
                  </td>
                  <td className="flex items-center gap-1 px-2 py-1.5">
                    <button
                      onClick={() => handleInstanceAction(inst.name, inst.zone, "connect")}
                      title={t("cloud.connect")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Wifi size={14} />
                    </button>
                    <button
                      onClick={() => handleInstanceAction(inst.name, inst.zone, "start")}
                      title={t("cloud.start")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Play size={14} />
                    </button>
                    <button
                      onClick={() => handleInstanceAction(inst.name, inst.zone, "stop")}
                      title={t("cloud.stop")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Square size={14} />
                    </button>
                    <button
                      onClick={() => handleInstanceAction(inst.name, inst.zone, "iap")}
                      title={t("cloud.iapTunnel")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Shield size={14} />
                    </button>
                  </td>
                </tr>
              ))}
              {instances.length === 0 && (
                <tr>
                  <td colSpan={6} className="px-2 py-8 text-center text-text-disabled">
                    {t("cloud.noResources")}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {/* GCS bucket browser */}
      {!loading && activeSection === "storage" && (
        <div className="flex flex-col gap-2">
          {selectedBucket ? (
            <>
              <button
                onClick={() => {
                  setSelectedBucket(null);
                  setObjects([]);
                }}
                className="flex items-center gap-1 text-xs text-text-link hover:underline"
              >
                ← {t("cloud.buckets")}
              </button>
              <h3 className="text-xs font-medium">{selectedBucket}</h3>
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b border-border-subtle text-left text-text-secondary">
                    <th className="px-2 py-1.5">Name</th>
                    <th className="px-2 py-1.5">Size</th>
                    <th className="px-2 py-1.5">Content Type</th>
                    <th className="px-2 py-1.5">Created</th>
                  </tr>
                </thead>
                <tbody>
                  {objects.map((obj) => (
                    <tr
                      key={obj.name}
                      className="border-b border-border-subtle hover:bg-surface-secondary"
                    >
                      <td className="px-2 py-1.5 font-mono">{obj.name}</td>
                      <td className="px-2 py-1.5">{obj.size}</td>
                      <td className="px-2 py-1.5">{obj.content_type}</td>
                      <td className="px-2 py-1.5">{obj.time_created}</td>
                    </tr>
                  ))}
                  {objects.length === 0 && (
                    <tr>
                      <td colSpan={4} className="px-2 py-8 text-center text-text-disabled">
                        {t("cloud.noResources")}
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </>
          ) : (
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
              {buckets.map((bucket) => (
                <button
                  key={bucket.name}
                  onClick={() => loadBucketObjects(bucket.name)}
                  className="flex items-center gap-2 rounded border border-border-default p-2 text-left hover:bg-surface-secondary"
                >
                  <FolderOpen size={16} className="text-accent-primary" />
                  <div>
                    <div className="text-xs font-medium">{bucket.name}</div>
                    <div className="text-[10px] text-text-secondary">
                      {bucket.location} • {bucket.storage_class}
                    </div>
                  </div>
                  <ChevronRight size={14} className="ml-auto text-text-disabled" />
                </button>
              ))}
              {buckets.length === 0 && (
                <p className="col-span-full py-8 text-center text-xs text-text-disabled">
                  {t("cloud.noResources")}
                </p>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
