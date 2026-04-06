import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  ChevronDown,
  ChevronRight,
  Cloud,
  Database,
  FolderOpen,
  Globe,
  HardDrive,
  Loader2,
  Network,
  RefreshCw,
  Server,
} from "lucide-react";
import type { CloudAssetNode, CloudAssetType } from "@/types";

function assetIcon(type: CloudAssetType, size = 14) {
  switch (type) {
    case "provider":
      return <Cloud size={size} />;
    case "region":
      return <Globe size={size} />;
    case "resource_group":
      return <FolderOpen size={size} />;
    case "compute":
      return <Server size={size} />;
    case "storage":
      return <HardDrive size={size} />;
    case "kubernetes":
      return <Network size={size} />;
    case "database":
      return <Database size={size} />;
    case "network":
      return <Network size={size} />;
    default:
      return <FolderOpen size={size} />;
  }
}

function TreeNode({
  node,
  depth,
  onConnect,
}: {
  readonly node: CloudAssetNode;
  readonly depth: number;
  readonly onConnect: (node: CloudAssetNode) => void;
}) {
  const [expanded, setExpanded] = useState(depth < 2);
  const hasChildren = node.children.length > 0;

  return (
    <div>
      <button
        className={clsx(
          "flex w-full items-center gap-1 rounded px-1 py-0.5 text-xs hover:bg-surface-secondary",
          "text-text-primary"
        )}
        style={{ paddingLeft: `${depth * 16 + 4}px` }}
        onClick={() => {
          if (hasChildren) setExpanded(!expanded);
        }}
        onDoubleClick={() => {
          if (node.node_type === "compute") {
            onConnect(node);
          }
        }}
      >
        {hasChildren ? (
          (() => {
            const ExpandIcon = expanded ? ChevronDown : ChevronRight;
            return <ExpandIcon size={12} className="shrink-0 text-text-secondary" />;
          })()
        ) : (
          <span className="w-3 shrink-0" />
        )}
        <span className="shrink-0 text-accent-primary">
          {assetIcon(node.node_type)}
        </span>
        <span className="truncate">{node.name}</span>
        {node.metadata.state && (
          <span
            className={clsx(
              "ml-auto rounded-full px-1.5 py-0.5 text-[10px]",
              node.metadata.state === "running" || node.metadata.state === "RUNNING"
                ? "bg-status-connected/20 text-status-connected"
                : "bg-status-disconnected/20 text-status-disconnected"
            )}
          >
            {node.metadata.state}
          </span>
        )}
      </button>
      {expanded &&
        hasChildren &&
        node.children.map((child) => (
          <TreeNode
            key={child.id}
            node={child}
            depth={depth + 1}
            onConnect={onConnect}
          />
        ))}
    </div>
  );
}

export default function CloudAssetTree() {
  const { t } = useTranslation();
  const [tree, setTree] = useState<CloudAssetNode[]>([]);
  const [loading, setLoading] = useState(true);

  const loadTree = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<CloudAssetNode[]>("cloud_get_asset_tree");
      setTree(result);
    } catch {
      setTree([]);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadTree();
  }, [loadTree]);

  const handleConnect = useCallback((_node: CloudAssetNode) => {
    // Connection logic delegated to session/terminal creation
  }, []);

  return (
    <div className="flex flex-col gap-2 p-4">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-text-primary">
          {t("cloud.assetTree")}
        </h3>
        <button
          onClick={loadTree}
          className="flex items-center gap-1 rounded border border-border-default px-2 py-1 text-xs text-text-secondary hover:bg-surface-secondary"
        >
          <RefreshCw size={12} />
          {t("cloud.refresh")}
        </button>
      </div>

      {(() => {
        if (loading) {
          return (
            <div className="flex justify-center py-8">
              <Loader2 size={20} className="animate-spin text-text-disabled" />
            </div>
          );
        }
        if (tree.length > 0) {
          return (
            <div className="rounded border border-border-default bg-surface-secondary p-1">
              {tree.map((node) => (
                <TreeNode
                  key={node.id}
                  node={node}
                  depth={0}
                  onConnect={handleConnect}
                />
              ))}
            </div>
          );
        }
        return (
          <div className="flex flex-col items-center justify-center py-16 text-text-disabled">
            <Cloud size={32} />
            <p className="mt-2 text-sm">{t("cloud.noResources")}</p>
          </div>
        );
      })()}
    </div>
  );
}
