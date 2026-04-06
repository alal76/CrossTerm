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

type ProviderTab = "aws" | "azure" | "gcp" | "tree";

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
              {tab.key !== "tree" && (
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
          </>
        )}
      </div>
    </div>
  );
}
