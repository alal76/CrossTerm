import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Box,
  Loader2,
  RefreshCw,
  Terminal,
} from "lucide-react";

interface KubectlPanelProps {
  readonly provider: "azure" | "gcp";
  readonly cluster: string;
  readonly resourceGroup?: string;
  readonly zone?: string;
  readonly project?: string;
}

interface PodInfo {
  name: string;
  namespace: string;
  status: string;
  ready: string;
  restarts: number;
  age: string;
}

export default function KubectlPanel({
  provider,
  cluster,
  resourceGroup,
  zone,
  project,
}: KubectlPanelProps) {
  const { t } = useTranslation();
  const [namespace, setNamespace] = useState("default");
  const [namespaces, setNamespaces] = useState<string[]>(["default"]);
  const [pods, setPods] = useState<PodInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [execOutput, setExecOutput] = useState<string | null>(null);
  const [execCommand, setExecCommand] = useState("ls -la");
  const [selectedPod, setSelectedPod] = useState<string | null>(null);

  const loadNamespaces = useCallback(async () => {
    try {
      const output = await invoke<string>("kubectl_get_namespaces", {});
      const ns = output
        .split("\n")
        .filter((l) => l.trim().length > 0)
        .slice(1) // skip header
        .map((l) => l.split(/\s+/)[0]);
      setNamespaces(ns.length > 0 ? ns : ["default"]);
    } catch {
      setNamespaces(["default"]);
    }
  }, []);

  const loadPods = useCallback(async () => {
    setLoading(true);
    try {
      // Get credentials first
      if (provider === "azure" && resourceGroup) {
        await invoke("cloud_azure_aks_get_credentials", {
          cluster,
          resourceGroup,
        });
      } else if (provider === "gcp") {
        await invoke("cloud_gcp_gke_get_credentials", {
          cluster,
          zone: zone ?? "",
          project,
        });
      }

      // Then list pods via kubectl
      const output = await invoke<string>("kubectl_list_pods", {
        namespace,
      });
      const lines = output.split("\n").filter((l) => l.trim().length > 0);
      const podList: PodInfo[] = lines.slice(1).map((line) => {
        const parts = line.split(/\s+/);
        return {
          name: parts[0] ?? "",
          ready: parts[1] ?? "0/0",
          status: parts[2] ?? "Unknown",
          restarts: Number.parseInt(parts[3] ?? "0", 10),
          age: parts[4] ?? "",
          namespace,
        };
      });
      setPods(podList);
    } catch {
      setPods([]);
    } finally {
      setLoading(false);
    }
  }, [provider, cluster, resourceGroup, zone, project, namespace]);

  useEffect(() => {
    loadNamespaces();
  }, [loadNamespaces]);

  useEffect(() => {
    loadPods();
  }, [loadPods]);

  const handleExec = useCallback(
    async (pod: string) => {
      setSelectedPod(pod);
      setExecOutput(null);
      try {
        const execFn =
          provider === "azure"
            ? "cloud_azure_aks_exec"
            : "cloud_gcp_gke_exec";

        const params: Record<string, string> = {
          cluster,
          namespace,
          pod,
          command: execCommand,
        };

        if (provider === "azure" && resourceGroup) {
          params.resourceGroup = resourceGroup;
        } else if (provider === "gcp") {
          params.zone = zone ?? "";
          if (project) params.project = project;
        }

        const output = await invoke<string>(execFn, params);
        setExecOutput(output);
      } catch (err) {
        setExecOutput(`Error: ${String(err)}`);
      }
    },
    [provider, cluster, resourceGroup, zone, project, namespace, execCommand]
  );

  return (
    <div className="flex flex-col gap-3 p-4">
      {/* Header */}
      <div className="flex items-center gap-2">
        <Box size={16} className="text-accent-primary" />
        <h3 className="text-sm font-medium text-text-primary">
          {t("cloud.kubectl")} — {cluster}
        </h3>
      </div>

      {/* Controls */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-1">
          <label className="text-xs text-text-secondary">
            {t("cloud.namespace")}
          </label>
          <select
            value={namespace}
            onChange={(e) => setNamespace(e.target.value)}
            className="rounded border border-border-default bg-surface-secondary px-2 py-1 text-xs text-text-primary"
          >
            {namespaces.map((ns) => (
              <option key={ns} value={ns}>
                {ns}
              </option>
            ))}
          </select>
        </div>
        <button
          onClick={loadPods}
          className="flex items-center gap-1 rounded border border-border-default px-2 py-1 text-xs text-text-secondary hover:bg-surface-secondary"
        >
          <RefreshCw size={12} />
          Refresh
        </button>
      </div>

      {/* Pods table */}
      {loading ? (
        <div className="flex justify-center py-8">
          <Loader2 size={20} className="animate-spin text-text-disabled" />
        </div>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-border-subtle text-left text-text-secondary">
                <th className="px-2 py-1.5">{t("cloud.pod")}</th>
                <th className="px-2 py-1.5">Ready</th>
                <th className="px-2 py-1.5">Status</th>
                <th className="px-2 py-1.5">Restarts</th>
                <th className="px-2 py-1.5">Age</th>
                <th className="px-2 py-1.5">Actions</th>
              </tr>
            </thead>
            <tbody>
              {pods.map((pod) => (
                <tr
                  key={pod.name}
                  className={clsx(
                    "border-b border-border-subtle hover:bg-surface-secondary",
                    selectedPod === pod.name && "bg-surface-secondary"
                  )}
                >
                  <td className="px-2 py-1.5 font-mono">{pod.name}</td>
                  <td className="px-2 py-1.5">{pod.ready}</td>
                  <td className="px-2 py-1.5">
                    <span
                      className={clsx(
                        "inline-block rounded-full px-1.5 py-0.5 text-[10px] font-medium",
                        pod.status === "Running"
                          ? "bg-status-connected/20 text-status-connected"
                          : "bg-status-disconnected/20 text-status-disconnected"
                      )}
                    >
                      {pod.status}
                    </span>
                  </td>
                  <td className="px-2 py-1.5">{pod.restarts}</td>
                  <td className="px-2 py-1.5">{pod.age}</td>
                  <td className="px-2 py-1.5">
                    <button
                      onClick={() => handleExec(pod.name)}
                      title={t("cloud.exec")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Terminal size={14} />
                    </button>
                  </td>
                </tr>
              ))}
              {pods.length === 0 && (
                <tr>
                  <td
                    colSpan={6}
                    className="px-2 py-8 text-center text-text-disabled"
                  >
                    {t("cloud.noPods")}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {/* Exec panel */}
      {selectedPod && (
        <div className="flex flex-col gap-2 rounded border border-border-default bg-surface-sunken p-3">
          <div className="flex items-center gap-2">
            <Terminal size={12} className="text-accent-primary" />
            <span className="text-xs font-medium text-text-primary">
              {t("cloud.exec")} — {selectedPod}
            </span>
          </div>
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={execCommand}
              onChange={(e) => setExecCommand(e.target.value)}
              placeholder="ls -la"
              className="flex-1 rounded border border-border-default bg-surface-secondary px-2 py-1 font-mono text-xs text-text-primary"
              onKeyDown={(e) => {
                if (e.key === "Enter" && selectedPod) {
                  handleExec(selectedPod);
                }
              }}
            />
            <button
              onClick={() => handleExec(selectedPod)}
              className="rounded bg-accent-primary px-2 py-1 text-xs text-text-inverse hover:bg-interactive-hover"
            >
              Run
            </button>
          </div>
          {execOutput !== null && (
            <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded bg-surface-primary p-2 font-mono text-xs text-text-primary">
              {execOutput}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}
