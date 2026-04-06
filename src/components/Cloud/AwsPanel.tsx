import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  ChevronRight,
  DollarSign,
  FolderOpen,
  HardDrive,
  Loader2,
  LogIn,
  Play,
  RefreshCw,
  Square,
  Terminal,
  Wifi,
  Zap,
  Container,
} from "lucide-react";
import type {
  CloudProviderStatus,
  Ec2Instance,
  S3Bucket,
  S3Object,
  CostSummary,
} from "@/types";

interface AwsPanelProps {
  readonly status: CloudProviderStatus | undefined;
}

export default function AwsPanel({ status }: AwsPanelProps) {
  const { t } = useTranslation();
  const [selectedProfile, setSelectedProfile] = useState<string>("");
  const [profiles, setProfiles] = useState<string[]>([]);
  const [instances, setInstances] = useState<Ec2Instance[]>([]);
  const [buckets, setBuckets] = useState<S3Bucket[]>([]);
  const [objects, setObjects] = useState<S3Object[]>([]);
  const [selectedBucket, setSelectedBucket] = useState<string | null>(null);
  const [costSummary, setCostSummary] = useState<CostSummary | null>(null);
  const [activeSection, setActiveSection] = useState<"instances" | "storage" | "cost" | "lambda" | "ecs">("instances");
  const [loading, setLoading] = useState(false);

  // Lambda state
  const [lambdaName, setLambdaName] = useState("");
  const [lambdaPayload, setLambdaPayload] = useState("{}");
  const [lambdaResult, setLambdaResult] = useState<string | null>(null);

  // ECS Exec state
  const [ecsCluster, setEcsCluster] = useState("");
  const [ecsTask, setEcsTask] = useState("");
  const [ecsContainer, setEcsContainer] = useState("");
  const [ecsCommand, setEcsCommand] = useState("/bin/sh");
  const [ecsResult, setEcsResult] = useState<string | null>(null);

  useEffect(() => {
    if (status?.profiles) {
      setProfiles(status.profiles);
      if (status.active_profile) {
        setSelectedProfile(status.active_profile);
      } else if (status.profiles.length > 0) {
        setSelectedProfile(status.profiles[0]);
      }
    }
  }, [status]);

  const loadInstances = useCallback(async () => {
    if (!selectedProfile) return;
    setLoading(true);
    try {
      const result = await invoke<Ec2Instance[]>("cloud_aws_list_instances", {
        profile: selectedProfile,
      });
      setInstances(result);
    } catch {
      setInstances([]);
    } finally {
      setLoading(false);
    }
  }, [selectedProfile]);

  const loadBuckets = useCallback(async () => {
    if (!selectedProfile) return;
    setLoading(true);
    try {
      const result = await invoke<S3Bucket[]>("cloud_aws_list_buckets", {
        profile: selectedProfile,
      });
      setBuckets(result);
    } catch {
      setBuckets([]);
    } finally {
      setLoading(false);
    }
  }, [selectedProfile]);

  const loadCost = useCallback(async () => {
    if (!selectedProfile) return;
    setLoading(true);
    try {
      const result = await invoke<CostSummary>("cloud_aws_get_cost", {
        profile: selectedProfile,
      });
      setCostSummary(result);
    } catch {
      setCostSummary(null);
    } finally {
      setLoading(false);
    }
  }, [selectedProfile]);

  const loadBucketObjects = useCallback(
    async (bucket: string) => {
      if (!selectedProfile) return;
      setSelectedBucket(bucket);
      setLoading(true);
      try {
        const result = await invoke<S3Object[]>("cloud_aws_list_objects", {
          profile: selectedProfile,
          bucket,
        });
        setObjects(result);
      } catch {
        setObjects([]);
      } finally {
        setLoading(false);
      }
    },
    [selectedProfile]
  );

  const handleLambdaInvoke = useCallback(async () => {
    if (!selectedProfile || !lambdaName) return;
    setLoading(true);
    try {
      const result = await invoke<string>("cloud_aws_lambda_invoke", {
        profile: selectedProfile,
        functionName: lambdaName,
        payload: lambdaPayload,
      });
      setLambdaResult(result);
    } catch (err) {
      setLambdaResult(`Error: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  }, [selectedProfile, lambdaName, lambdaPayload]);

  const handleEcsExec = useCallback(async () => {
    if (!selectedProfile || !ecsCluster || !ecsTask) return;
    setLoading(true);
    try {
      const result = await invoke<string>("cloud_aws_ecs_exec", {
        profile: selectedProfile,
        cluster: ecsCluster,
        task: ecsTask,
        container: ecsContainer || undefined,
        command: ecsCommand,
      });
      setEcsResult(result);
    } catch (err) {
      setEcsResult(`Error: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  }, [selectedProfile, ecsCluster, ecsTask, ecsContainer, ecsCommand]);

  useEffect(() => {
    if (selectedProfile) {
      if (activeSection === "instances") loadInstances();
      else if (activeSection === "storage") loadBuckets();
      else if (activeSection === "cost") loadCost();
    }
  }, [selectedProfile, activeSection, loadInstances, loadBuckets, loadCost]);

  const handleSsoLogin = useCallback(async () => {
    if (!selectedProfile) return;
    try {
      await invoke("cloud_aws_sso_login", { profile: selectedProfile });
    } catch {
      // Login failed
    }
  }, [selectedProfile]);

  const handleInstanceAction = useCallback(
    async (instanceId: string, action: "start" | "stop" | "connect" | "ssm") => {
      if (!selectedProfile) return;
      try {
        if (action === "start") {
          await invoke("cloud_aws_start_instance", { profile: selectedProfile, instanceId });
          loadInstances();
        } else if (action === "stop") {
          await invoke("cloud_aws_stop_instance", { profile: selectedProfile, instanceId });
          loadInstances();
        } else if (action === "connect") {
          await invoke("cloud_aws_connect_instance", { profile: selectedProfile, instanceId });
        } else if (action === "ssm") {
          await invoke("cloud_aws_ssm_session", { profile: selectedProfile, instanceId });
        }
      } catch {
        // Action failed
      }
    },
    [selectedProfile, loadInstances]
  );

  if (status?.cli_status.type === "not_installed") {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-text-disabled">
        <HardDrive size={32} />
        <p className="mt-2 text-sm">{t("cloud.notInstalled")}</p>
        <p className="text-xs">Install the AWS CLI to get started.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Profile selector + SSO login */}
      <div className="flex items-center gap-3">
        <label className="text-xs font-medium text-text-secondary">
          {t("cloud.selectProfile")}
        </label>
        <select
          value={selectedProfile}
          onChange={(e) => setSelectedProfile(e.target.value)}
          className="rounded border border-border-default bg-surface-secondary px-2 py-1 text-xs text-text-primary"
        >
          {profiles.map((p) => (
            <option key={p} value={p}>
              {p}
            </option>
          ))}
        </select>
        <button
          onClick={handleSsoLogin}
          className="flex items-center gap-1 rounded bg-accent-primary px-2 py-1 text-xs text-text-inverse hover:bg-interactive-hover"
        >
          <LogIn size={12} />
          {t("cloud.ssoLogin")}
        </button>
        <button
          onClick={() => {
            if (activeSection === "instances") loadInstances();
            else if (activeSection === "storage") loadBuckets();
            else loadCost();
          }}
          className="ml-auto flex items-center gap-1 rounded border border-border-default px-2 py-1 text-xs text-text-secondary hover:bg-surface-secondary"
        >
          <RefreshCw size={12} />
          {t("cloud.refresh")}
        </button>
      </div>

      {/* Section tabs */}
      <div className="flex items-center gap-1 border-b border-border-subtle">
        {(["instances", "storage", "cost", "lambda", "ecs"] as const).map((section) => {
          const getSectionLabel = (s: string): string => {
            if (s === "lambda") return t("cloud.lambda");
            if (s === "ecs") return t("cloud.ecsExec");
            return t(`cloud.${s}`);
          };
          return (
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
              {getSectionLabel(section)}
            </button>
          );
        })}
      </div>

      {loading && (
        <div className="flex justify-center py-8">
          <Loader2 size={20} className="animate-spin text-text-disabled" />
        </div>
      )}

      {/* Instances table */}
      {!loading && activeSection === "instances" && (
        <div className="overflow-x-auto">
          <table className="w-full text-xs">
            <thead>
              <tr className="border-b border-border-subtle text-left text-text-secondary">
                <th className="px-2 py-1.5">Instance ID</th>
                <th className="px-2 py-1.5">Name</th>
                <th className="px-2 py-1.5">State</th>
                <th className="px-2 py-1.5">Type</th>
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
                  <td className="px-2 py-1.5 font-mono">{inst.id}</td>
                  <td className="px-2 py-1.5">{inst.name}</td>
                  <td className="px-2 py-1.5">
                    <span
                      className={clsx(
                        "inline-block rounded-full px-1.5 py-0.5 text-[10px] font-medium",
                        inst.state === "running"
                          ? "bg-status-connected/20 text-status-connected"
                          : "bg-status-disconnected/20 text-status-disconnected"
                      )}
                    >
                      {inst.state}
                    </span>
                  </td>
                  <td className="px-2 py-1.5">{inst.instance_type}</td>
                  <td className="px-2 py-1.5 font-mono">
                    {inst.public_ip ?? inst.private_ip ?? "—"}
                  </td>
                  <td className="flex items-center gap-1 px-2 py-1.5">
                    <button
                      onClick={() => handleInstanceAction(inst.id, "connect")}
                      title={t("cloud.connect")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Wifi size={14} />
                    </button>
                    <button
                      onClick={() => handleInstanceAction(inst.id, "start")}
                      title={t("cloud.start")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Play size={14} />
                    </button>
                    <button
                      onClick={() => handleInstanceAction(inst.id, "stop")}
                      title={t("cloud.stop")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Square size={14} />
                    </button>
                    <button
                      onClick={() => handleInstanceAction(inst.id, "ssm")}
                      title={t("cloud.ssmSession")}
                      className="rounded p-0.5 hover:bg-interactive-hover"
                    >
                      <Terminal size={14} />
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

      {/* Storage browser */}
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
                    <th className="px-2 py-1.5">Key</th>
                    <th className="px-2 py-1.5">Size</th>
                    <th className="px-2 py-1.5">Last Modified</th>
                    <th className="px-2 py-1.5">Storage Class</th>
                  </tr>
                </thead>
                <tbody>
                  {objects.map((obj) => (
                    <tr
                      key={obj.key}
                      className="border-b border-border-subtle hover:bg-surface-secondary"
                    >
                      <td className="px-2 py-1.5 font-mono">{obj.key}</td>
                      <td className="px-2 py-1.5">{obj.size}</td>
                      <td className="px-2 py-1.5">{obj.last_modified}</td>
                      <td className="px-2 py-1.5">{obj.storage_class}</td>
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
                    <div className="text-[10px] text-text-secondary">{bucket.region}</div>
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

      {/* Cost summary */}
      {!loading && activeSection === "cost" && (
        <div className="flex flex-col gap-3">
          {costSummary ? (
            <>
              <div className="rounded-lg border border-border-default bg-surface-secondary p-4">
                <div className="flex items-center gap-2">
                  <DollarSign size={20} className="text-accent-primary" />
                  <div>
                    <p className="text-xs text-text-secondary">{t("cloud.currentMonth")}</p>
                    <p className="text-lg font-semibold">
                      {costSummary.currency} {costSummary.total_cost.toFixed(2)}
                    </p>
                  </div>
                </div>
                <p className="mt-1 text-[10px] text-text-disabled">
                  {costSummary.start_date} — {costSummary.end_date}
                </p>
              </div>
              {costSummary.by_service.length > 0 && (
                <table className="w-full text-xs">
                  <thead>
                    <tr className="border-b border-border-subtle text-left text-text-secondary">
                      <th className="px-2 py-1.5">Service</th>
                      <th className="px-2 py-1.5 text-right">Cost</th>
                    </tr>
                  </thead>
                  <tbody>
                    {costSummary.by_service.map((svc) => (
                      <tr
                        key={svc.service_name}
                        className="border-b border-border-subtle"
                      >
                        <td className="px-2 py-1.5">{svc.service_name}</td>
                        <td className="px-2 py-1.5 text-right font-mono">
                          {costSummary.currency} {svc.cost.toFixed(2)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </>
          ) : (
            <p className="py-8 text-center text-xs text-text-disabled">
              {t("cloud.noResources")}
            </p>
          )}
        </div>
      )}

      {/* Lambda invoke */}
      {!loading && activeSection === "lambda" && (
        <div className="flex flex-col gap-3">
          <div>
            <label htmlFor="aws-lambda-func" className="mb-1 block text-xs text-text-secondary">
              Function Name
            </label>
            <input
              id="aws-lambda-func"
              type="text"
              value={lambdaName}
              onChange={(e) => setLambdaName(e.target.value)}
              placeholder="my-function"
              className="w-full rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
            />
          </div>
          <div>
            <label htmlFor="aws-lambda-payload" className="mb-1 block text-xs text-text-secondary">
              Payload (JSON)
            </label>
            <textarea
              id="aws-lambda-payload"
              value={lambdaPayload}
              onChange={(e) => setLambdaPayload(e.target.value)}
              rows={4}
              className="w-full rounded border border-border-default bg-surface-secondary px-2 py-1.5 font-mono text-xs text-text-primary"
            />
          </div>
          <button
            onClick={handleLambdaInvoke}
            disabled={!lambdaName}
            className="flex items-center gap-1 self-start rounded bg-accent-primary px-3 py-1.5 text-xs text-text-inverse hover:bg-interactive-hover disabled:opacity-50"
          >
            <Zap size={12} />
            {t("cloud.invoke")}
          </button>
          {lambdaResult !== null && (
            <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded border border-border-subtle bg-surface-sunken p-3 font-mono text-xs text-text-primary">
              {lambdaResult}
            </pre>
          )}
        </div>
      )}

      {/* ECS Exec */}
      {!loading && activeSection === "ecs" && (
        <div className="flex flex-col gap-3">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label htmlFor="aws-ecs-cluster" className="mb-1 block text-xs text-text-secondary">
                Cluster
              </label>
              <input
                id="aws-ecs-cluster"
                type="text"
                value={ecsCluster}
                onChange={(e) => setEcsCluster(e.target.value)}
                placeholder="my-cluster"
                className="w-full rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
              />
            </div>
            <div>
              <label htmlFor="aws-ecs-task" className="mb-1 block text-xs text-text-secondary">
                Task ID
              </label>
              <input
                id="aws-ecs-task"
                type="text"
                value={ecsTask}
                onChange={(e) => setEcsTask(e.target.value)}
                placeholder="task-id"
                className="w-full rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
              />
            </div>
            <div>
              <label htmlFor="aws-ecs-container" className="mb-1 block text-xs text-text-secondary">
                Container (optional)
              </label>
              <input
                id="aws-ecs-container"
                type="text"
                value={ecsContainer}
                onChange={(e) => setEcsContainer(e.target.value)}
                placeholder="container-name"
                className="w-full rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
              />
            </div>
            <div>
              <label htmlFor="aws-ecs-command" className="mb-1 block text-xs text-text-secondary">
                Command
              </label>
              <input
                id="aws-ecs-command"
                type="text"
                value={ecsCommand}
                onChange={(e) => setEcsCommand(e.target.value)}
                placeholder="/bin/sh"
                className="w-full rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
              />
            </div>
          </div>
          <button
            onClick={handleEcsExec}
            disabled={!ecsCluster || !ecsTask}
            className="flex items-center gap-1 self-start rounded bg-accent-primary px-3 py-1.5 text-xs text-text-inverse hover:bg-interactive-hover disabled:opacity-50"
          >
            <Container size={12} />
            {t("cloud.ecsExec")}
          </button>
          {ecsResult !== null && (
            <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded border border-border-subtle bg-surface-sunken p-3 font-mono text-xs text-text-primary">
              {ecsResult}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}
