import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Cloud,
  Loader2,
  CheckCircle2,
  XCircle,
  AlertCircle,
} from "lucide-react";
import type { CloudProviderStatus } from "@/types";
import AwsPanel from "@/components/Cloud/AwsPanel";
import AzurePanel from "@/components/Cloud/AzurePanel";
import GcpPanel from "@/components/Cloud/GcpPanel";
import CloudAssetTree from "@/components/Cloud/CloudAssetTree";
import KubectlPanel from "@/components/Cloud/KubectlPanel";

type ProviderTab = "aws" | "azure" | "gcp" | "tree" | "kubectl";

function KubectlTab() {
  const { t } = useTranslation();
  const [provider, setProvider] = useState<"azure" | "gcp">("azure");
  const [cluster, setCluster] = useState("");
  const [resourceGroup, setResourceGroup] = useState("");
  const [zone, setZone] = useState("");
  const [project, setProject] = useState("");
  const [connected, setConnected] = useState(false);

  if (connected && cluster) {
    return (
      <KubectlPanel
        provider={provider}
        cluster={cluster}
        resourceGroup={resourceGroup || undefined}
        zone={zone || undefined}
        project={project || undefined}
      />
    );
  }

  return (
    <div className="flex flex-col gap-3 p-4 max-w-sm">
      <div>
        <label htmlFor="kubectl-provider" className="mb-1 block text-[11px] font-medium text-text-secondary">Provider</label>
        <select
          id="kubectl-provider"
          value={provider}
          onChange={(e) => setProvider(e.target.value as "azure" | "gcp")}
          className="w-full rounded-md border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
        >
          <option value="azure">AKS (Azure)</option>
          <option value="gcp">GKE (GCP)</option>
        </select>
      </div>
      <div>
        <label htmlFor="kubectl-cluster" className="mb-1 block text-[11px] font-medium text-text-secondary">Cluster name</label>
        <input
          id="kubectl-cluster"
          value={cluster}
          onChange={(e) => setCluster(e.target.value)}
          className="w-full rounded-md border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
        />
      </div>
      {provider === "azure" ? (
        <div>
          <label htmlFor="kubectl-resource-group" className="mb-1 block text-[11px] font-medium text-text-secondary">Resource group</label>
          <input
            id="kubectl-resource-group"
            value={resourceGroup}
            onChange={(e) => setResourceGroup(e.target.value)}
            className="w-full rounded-md border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
          />
        </div>
      ) : (
        <>
          <div>
            <label htmlFor="kubectl-zone" className="mb-1 block text-[11px] font-medium text-text-secondary">Zone</label>
            <input
              id="kubectl-zone"
              value={zone}
              onChange={(e) => setZone(e.target.value)}
              className="w-full rounded-md border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
            />
          </div>
          <div>
            <label htmlFor="kubectl-project" className="mb-1 block text-[11px] font-medium text-text-secondary">Project</label>
            <input
              id="kubectl-project"
              value={project}
              onChange={(e) => setProject(e.target.value)}
              className="w-full rounded-md border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
            />
          </div>
        </>
      )}
      <button
        type="button"
        disabled={!cluster.trim()}
        onClick={() => setConnected(true)}
        className="rounded-md bg-interactive-default px-3 py-1.5 text-xs font-medium text-text-inverse hover:bg-interactive-hover disabled:cursor-not-allowed disabled:bg-interactive-disabled disabled:text-text-disabled"
      >
        {t("cloud.connect")}
      </button>
    </div>
  );
}

function ProviderStatusBadge({
  status,
}: {
  readonly status: CloudProviderStatus | undefined;
}) {
  const { t } = useTranslation();

  if (!status) {
    return (
      <span className="flex items-center gap-1 text-xs text-text-disabled">
        <Loader2 size={12} className="animate-spin" />
      </span>
    );
  }

  if (status.cli_status.type === "not_installed") {
    return (
      <span className="flex items-center gap-1 text-xs text-status-disconnected">
        <XCircle size={12} />
        {t("cloud.notInstalled")}
      </span>
    );
  }

  if (status.profiles.length === 0 && !status.active_profile) {
    return (
      <span className="flex items-center gap-1 text-xs text-status-connecting">
        <AlertCircle size={12} />
        {t("cloud.notAuthenticated")}
      </span>
    );
  }

  return (
    <span className="flex items-center gap-1 text-xs text-status-connected">
      <CheckCircle2 size={12} />
      {t("cloud.connected")}
    </span>
  );
}

export default function CloudDashboard() {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<ProviderTab>("aws");
  const [statuses, setStatuses] = useState<CloudProviderStatus[]>([]);
  const [loading, setLoading] = useState(true);

  const detectClis = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<CloudProviderStatus[]>("cloud_detect_clis");
      setStatuses(result);
    } catch {
      // CLI detection failed; statuses remain empty
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    detectClis();
  }, [detectClis]);

  const getStatus = (provider: string) =>
    statuses.find((s) => s.provider === provider);

  const tabs: { key: ProviderTab; label: string }[] = [
    { key: "aws", label: t("cloud.aws") },
    { key: "azure", label: t("cloud.azure") },
    { key: "gcp", label: t("cloud.gcp") },
    { key: "tree", label: t("cloud.assetTree") },
    { key: "kubectl", label: t("cloud.kubectl") },
  ];

  return (
    <div className="flex h-full flex-col bg-surface-primary text-text-primary">
      {/* Header */}
      <div className="flex items-center gap-2 border-b border-border-default px-4 py-3">
        <Cloud size={20} className="text-accent-primary" />
        <h2 className="text-sm font-semibold">{t("cloud.dashboard")}</h2>
      </div>

      {/* Tab pills */}
      <div className="flex items-center gap-1 border-b border-border-default px-4 py-2">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            onClick={() => setActiveTab(tab.key)}
            className={clsx(
              "rounded-full px-3 py-1 text-xs font-medium transition-colors",
              activeTab === tab.key
                ? "bg-accent-primary text-text-inverse"
                : "bg-surface-secondary text-text-secondary hover:bg-interactive-hover"
            )}
          >
            <span className="flex items-center gap-1.5">
              {tab.label}
              {tab.key !== "tree" && tab.key !== "kubectl" && (
                <ProviderStatusBadge status={getStatus(tab.key)} />
              )}
            </span>
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto">
        {loading ? (
          <div className="flex items-center justify-center py-16">
            <Loader2 size={24} className="animate-spin text-text-disabled" />
          </div>
        ) : (
          <>
            {activeTab === "aws" && (
              <AwsPanel status={getStatus("aws")} />
            )}
            {activeTab === "azure" && (
              <AzurePanel status={getStatus("azure")} />
            )}
            {activeTab === "gcp" && (
              <GcpPanel status={getStatus("gcp")} />
            )}
            {activeTab === "tree" && <CloudAssetTree />}
            {activeTab === "kubectl" && <KubectlTab />}
          </>
        )}
      </div>
    </div>
  );
}
