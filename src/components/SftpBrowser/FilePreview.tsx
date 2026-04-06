import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import {
  Download,
  FileText,
  Image,
  X,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import type { FilePreview as FilePreviewType } from "@/types";

interface FilePreviewProps {
  readonly sessionId: string;
  readonly path: string;
  readonly onClose: () => void;
  readonly onDownload: (path: string) => void;
}

export default function FilePreview({
  sessionId,
  path,
  onClose,
  onDownload,
}: FilePreviewProps) {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<FilePreviewType | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);

  useEffect(() => {
    let cancelled = false;

    async function loadPreview() {
      setLoading(true);
      setError(null);
      try {
        const result = await invoke<FilePreviewType>("sftp_preview", {
          sessionId,
          path,
        });
        if (!cancelled) {
          setPreview(result);
        }
      } catch (err) {
        if (!cancelled) {
          setError(String(err));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    loadPreview();
    return () => {
      cancelled = true;
    };
  }, [sessionId, path]);

  const handleZoomIn = useCallback(() => {
    setZoom((prev) => Math.min(prev + 0.25, 3));
  }, []);

  const handleZoomOut = useCallback(() => {
    setZoom((prev) => Math.max(prev - 0.25, 0.25));
  }, []);

  const isText =
    preview?.content_type.startsWith("text/") ||
    preview?.content_type === "application/json";
  const isImage = preview?.content_type.startsWith("image/");

  return (
    <div className="flex h-full flex-col border-l border-border-default bg-surface-primary">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-border-subtle px-3 py-2">
        <div className="flex items-center gap-2 overflow-hidden">
          {isText ? (
            <FileText size={14} className="text-accent-primary" />
          ) : (
            <Image size={14} className="text-accent-primary" />
          )}
          <span className="truncate text-xs font-medium text-text-primary">
            {path.split("/").pop()}
          </span>
          {preview && (
            <span className="text-[10px] text-text-disabled">
              ({formatSize(preview.size)})
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          {isImage && (
            <>
              <button
                onClick={handleZoomOut}
                title="Zoom out"
                className="rounded p-1 hover:bg-interactive-hover"
              >
                <ZoomOut size={14} />
              </button>
              <button
                onClick={handleZoomIn}
                title="Zoom in"
                className="rounded p-1 hover:bg-interactive-hover"
              >
                <ZoomIn size={14} />
              </button>
            </>
          )}
          <button
            onClick={() => onDownload(path)}
            title={t("sftp.downloadFile")}
            className="rounded p-1 hover:bg-interactive-hover"
          >
            <Download size={14} />
          </button>
          <button
            onClick={onClose}
            title={t("sftp.closePreview")}
            className="rounded p-1 hover:bg-interactive-hover"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-3">
        {loading && (
          <div className="flex items-center justify-center py-16 text-text-disabled">
            <span className="text-xs">{t("common.loading")}</span>
          </div>
        )}

        {error && (
          <div className="flex items-center justify-center py-16 text-status-disconnected">
            <span className="text-xs">{error}</span>
          </div>
        )}

        {!loading && !error && preview && (
          <>
            {isText && (
              <pre
                className={clsx(
                  "whitespace-pre-wrap break-words font-mono text-xs",
                  "rounded border border-border-subtle bg-surface-sunken p-3",
                  "text-text-primary"
                )}
              >
                {preview.data.split("\n").map((line, i) => (
                  <div key={i} className="flex">
                    <span className="mr-3 inline-block w-8 select-none text-right text-text-disabled">
                      {i + 1}
                    </span>
                    <span>{line}</span>
                  </div>
                ))}
              </pre>
            )}

            {isImage && (
              <div className="flex items-center justify-center">
                <img
                  src={`data:${preview.content_type};base64,${preview.data}`}
                  alt={path.split("/").pop() ?? "preview"}
                  style={{ transform: `scale(${zoom})` }}
                  className="max-w-full transition-transform"
                />
              </div>
            )}

            {!isText && !isImage && (
              <pre
                className={clsx(
                  "whitespace-pre font-mono text-xs",
                  "rounded border border-border-subtle bg-surface-sunken p-3",
                  "text-text-secondary"
                )}
              >
                {preview.data}
              </pre>
            )}

            {preview.truncated && (
              <p className="mt-2 text-center text-[10px] text-text-disabled">
                File truncated — showing first 1 MB
              </p>
            )}
          </>
        )}
      </div>
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
