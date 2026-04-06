import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
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
  AzureSubscription,
  AzureVm,
  AzureStorageAccount,
} from "@/types";

interface AzurePanelProps {
  readonly status: CloudProviderStatus | undefined;
}

export default function AzurePanel({ status }: AzurePanelProps) {
  const { t } = useTranslation();
  const [subscriptions, setSubscriptions] = useState<AzureSubscription[]>([]);
  const [selectedSub, setSelectedSub] = useState<string>("");
  const [vms, setVms] = useState<AzureVm[]>([]);
  const [storageAccounts, setStorageAccounts] = useState<AzureStorageAccount[]>([]);
  const [activeSection, setActiveSection] = useState<"instances" | "storage">("instances");
  const [loading, setLoading] = useState(false);

  const loadSubscriptions = useCallback(async () => {
    try {
      const result = await invoke<AzureSubscription[]>("cloud_azure_list_subscriptions");
      setSubscriptions(result);
      if (result.length > 0) {
        setSelectedSub(result[0].id);
      }
    } catch {
      setSubscriptions([]);
    }
  }, []);

  useEffect(() => {
    if (status?.cli_status.type === "installed") {
      loadSubscriptions();
    }
  }, [status, loadSubscriptions]);

  const loadVms = useCallback(async () => {
    if (!selectedSub) return;
    setLoading(true);
    try {
      const result = await invoke<AzureVm[]>("cloud_azure_list_vms", {
        subscriptionId: selectedSub,
      });
      setVms(result);
    } catch {
      setVms([]);
    } finally {
      setLoading(false);
    }
  }, [selectedSub]);

  const loadStorage = useCallback(async () => {
    if (!selectedSub) return;
    setLoading(true);
    try {
      const result = await invoke<AzureStorageAccount[]>(
        "cloud_azure_list_storage_accounts",
        { subscriptionId: selectedSub }
      );
      setStorageAccounts(result);
    } catch {
      setStorageAccounts([]);
    } finally {
      setLoading(false);
    }
  }, [selectedSub]);

  useEffect(() => {
    if (selectedSub) {
      if (activeSection === "instances") loadVms();
      else loadStorage();
    }
  }, [selectedSub, activeSection, loadVms, loadStorage]);

  const handleLogin = useCallback(async () => {
    try {
      await invoke("cloud_azure_login");
      loadSubscriptions();
    } catch {
      // Login failed
    }
  }, [loadSubscriptions]);

  const handleVmAction = useCallback(
    async (vmId: string, action: "start" | "stop" | "connect" | "bastion") => {
      if (!selectedSub) return;
      try {
        if (action === "start") {
          await invoke("cloud_azure_start_vm", { subscriptionId: selectedSub, vmId });
          loadVms();
        } else if (action === "stop") {
          await invoke("cloud_azure_stop_vm", { subscriptionId: selectedSub, vmId });
          loadVms();
        } else if (action === "connect") {
          await invoke("cloud_azure_connect_vm", { subscriptionId: selectedSub, vmId });
        } else if (action === "bastion") {
          await invoke("cloud_azure_bastion_connect", { subscriptionId: selectedSub, vmId });
        }
      } catch {
        // Action failed
      }
    },
    [selectedSub, loadVms]
  );

  const handleCloudShell = useCallback(async () => {
    try {
      await invoke("cloud_azure_cloud_shell");
    } catch {
      // Cloud Shell launch failed
    }
  }, []);

  if (status?.cli_status.type === "not_installed") {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-text-disabled">
        <HardDrive size={32} />
        <p className="mt-2 text-sm">{t("cloud.notInstalled")}</p>
        <p className="text-xs">Install the Azure CLI to get started.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Subscription selector + login */}
      <div className="flex items-center gap-3">
        <label className="text-xs font-medium text-text-secondary">
          {t("cloud.selectSubscription")}
        </label>
        <select
          value={selectedSub}
          onChange={(e) => setSelectedSub(e.target.value)}
          className="rounded border border-border-default bg-surface-secondary px-2 py-1 text-xs text-text-primary"
        >
          {subscriptions.map((sub) => (
            <option key={sub.id} value={sub.id}>
              {sub.name}
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
            if (activeSection === "instances") loadVms();
            else loadStorage();
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

      {/* VM table */}
      {!loading && activeSection === "instances" && (
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-border-subtle text-left text-text-secondary">
                <th className="px-2 py-1.5">Name</th>
                <th className="px-2 py-1.5">Resource Group</th>
                <th className="px-2 py-1.5">State</th>
                <th className="px-2 py-1.5">Size</th>
                <th className="px-2 py-1.5">IP</th>
                <th className="px-2 py-1.5">Actions</th>
              </tr>
            </thead>
            <tbody>
              {vms.map((vm) => (
                <tr
                  key={vm.id}
                  className="border-b border-border-subtle hover:bg-surface-secondary"
                >
                  <td className="px-2 py-1.5 font-medium">{vm.name}</td>
                  <td className="px-2 py-1.5">{vm.resource_group}</td>
                  <td className="px-2 py-1.5">
                    <span
                      className={clsx(
                        "inline-block rounded-full px-1.5 py-0.5 text-[10px] font-medium",
                        vm.status === "running"
                          ? "bg-status-connected/20 text-status-connected"
                          : "bg-status-disconnected/20 text-status-disconnected"
                      )}
                    >
                      {vm.status}
                    </span>
                  </td>
                  <td className="px-2 py-1.5">{vm.size}</td>
                  <td className="px-2 py-1.5 font-mono">
                    {vm.public_ip ?? vm.private_ip ?? "—"}
                  </td>
                  <td className="flex items-center gap-1 px-2 py-1.5">
                    <button
                      onClick={() => handleVmAction(vm.id, "connect")}
                      title={t("cloud.connect")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Wifi size={14} />
                    </button>
                    <button
                      onClick={() => handleVmAction(vm.id, "start")}
                      title={t("cloud.start")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Play size={14} />
                    </button>
                    <button
                      onClick={() => handleVmAction(vm.id, "stop")}
                      title={t("cloud.stop")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Square size={14} />
                    </button>
                    <button
                      onClick={() => handleVmAction(vm.id, "bastion")}
                      title={t("cloud.bastion")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Shield size={14} />
                    </button>
                  </td>
                </tr>
              ))}
              {vms.length === 0 && (
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

      {/* Storage accounts */}
      {!loading && activeSection === "storage" && (
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-3">
          {storageAccounts.map((sa) => (
            <div
              key={sa.name}
              className="flex flex-col rounded border border-border-default p-2"
            >
              <div className="text-xs font-medium">{sa.name}</div>
              <div className="text-[10px] text-text-secondary">
                {sa.kind} • {sa.location}
              </div>
              <div className="text-[10px] text-text-disabled">{sa.sku}</div>
            </div>
          ))}
          {storageAccounts.length === 0 && (
            <p className="col-span-full py-8 text-center text-xs text-text-disabled">
              {t("cloud.noResources")}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
