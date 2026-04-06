import { useEffect, useRef, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import type { RdpConfig, RdpConnectionStatus, RdpScaleMode } from "@/types";
import RdpToolbar from "./RdpToolbar";

interface RdpViewerProps {
  readonly sessionId: string;
  readonly config: RdpConfig;
}

interface RdpFramePayload {
  connection_id: string;
  width: number;
  height: number;
  data_base64: string;
}

export default function RdpViewer({ sessionId, config }: RdpViewerProps) {
  const { t } = useTranslation();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const connectionIdRef = useRef<string | null>(null);
  const resizeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const [status, setStatus] = useState<RdpConnectionStatus>("connecting");
  const [scaleMode, setScaleMode] = useState<RdpScaleMode>("fit");
  const [clipboardSync, setClipboardSync] = useState(config.clipboard_sync);

  // Connect on mount
  useEffect(() => {
    let cancelled = false;

    async function connect() {
      try {
        const id = await invoke<string>("rdp_connect", { config });
        if (cancelled) {
          await invoke("rdp_disconnect", { connectionId: id });
          return;
        }
        connectionIdRef.current = id;
        setStatus("connected");
      } catch (err) {
        if (!cancelled) {
          setStatus({ error: String(err) });
        }
      }
    }

    connect();

    return () => {
      cancelled = true;
      const id = connectionIdRef.current;
      if (id) {
        invoke("rdp_disconnect", { connectionId: id }).catch(() => {});
        connectionIdRef.current = null;
      }
    };
  }, [sessionId, config]);

  // Listen for frame events
  useEffect(() => {
    const unlisten = listen<RdpFramePayload>("rdp:frame", (event) => {
      const { connection_id, width, height, data_base64 } = event.payload;
      if (connection_id !== connectionIdRef.current) return;

      const canvas = canvasRef.current;
      if (!canvas) return;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;

      canvas.width = width;
      canvas.height = height;

      const raw = atob(data_base64);
      const bytes = new Uint8ClampedArray(raw.length);
      for (let i = 0; i < raw.length; i++) {
        bytes[i] = raw.codePointAt(i) ?? 0;
      }

      const imageData = new ImageData(bytes, width, height);
      ctx.putImageData(imageData, 0, 0);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Listen for disconnect events
  useEffect(() => {
    const unlisten = listen<{ connection_id: string; reason: string }>(
      "rdp:disconnected",
      (event) => {
        if (event.payload.connection_id === connectionIdRef.current) {
          setStatus("disconnected");
          connectionIdRef.current = null;
        }
      }
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // ResizeObserver with 300ms debounce
  const sendResize = useCallback(() => {
    const id = connectionIdRef.current;
    if (!id) return;
    const container = containerRef.current;
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const width = Math.round(rect.width);
    const height = Math.round(rect.height);
    if (width > 0 && height > 0) {
      invoke("rdp_resize", { connectionId: id, width, height }).catch(() => {});
    }
  }, []);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const observer = new ResizeObserver(() => {
      if (resizeTimerRef.current) {
        clearTimeout(resizeTimerRef.current);
      }
      resizeTimerRef.current = setTimeout(sendResize, 300);
    });

    observer.observe(container);

    return () => {
      observer.disconnect();
      if (resizeTimerRef.current) {
        clearTimeout(resizeTimerRef.current);
      }
    };
  }, []);

  const handleDisconnect = useCallback(() => {
    const id = connectionIdRef.current;
    if (id) {
      invoke("rdp_disconnect", { connectionId: id }).catch(() => {});
    }
  }, []);

  const handleCtrlAltDel = useCallback(() => {
    const id = connectionIdRef.current;
    if (id) {
      invoke("rdp_send_ctrl_alt_del", { connectionId: id }).catch(() => {});
    }
  }, []);

  const handleScreenshot = useCallback(async () => {
    const id = connectionIdRef.current;
    if (!id) return;
    try {
      await invoke<string>("rdp_screenshot", { connectionId: id });
    } catch {
      // screenshot failed silently
    }
  }, []);

  const handleFullScreen = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;

    if (document.fullscreenElement) {
      document.exitFullscreen().catch(() => {});
    } else {
      container.requestFullscreen().catch(() => {});
    }
  }, []);

  const handleClipboardToggle = useCallback(() => {
    setClipboardSync((prev) => !prev);
  }, []);

  // Canvas scale style based on mode
  const canvasStyle = (() => {
    switch (scaleMode) {
      case "fit":
        return { width: "100%", height: "100%", objectFit: "contain" as const };
      case "actual":
        return {};
      case "fit-width":
        return { width: "100%", height: "auto" };
      case "fit-height":
        return { width: "auto", height: "100%" };
      default:
        return { width: "100%", height: "100%", objectFit: "contain" as const };
    }
  })();

  const isConnecting = status === "connecting";
  const isError = typeof status === "object" && "error" in status;
  const isDisconnected = status === "disconnected";

  return (
    <div
      ref={containerRef}
      className={clsx(
        "relative flex items-center justify-center w-full h-full",
        "bg-surface-primary overflow-hidden"
      )}
    >
      {/* Connection status overlay */}
      {(isConnecting || isError || isDisconnected) && (
        <div className="absolute inset-0 z-20 flex flex-col items-center justify-center bg-surface-overlay/80">
          {isConnecting && (
            <>
              <div className="mb-3 h-8 w-8 animate-spin rounded-full border-2 border-accent-primary border-t-transparent" />
              <p className="text-text-secondary text-sm">
                {t("rdp.connecting")}
              </p>
            </>
          )}
          {isDisconnected && (
            <p className="text-text-secondary text-sm">
              {t("rdp.disconnected")}
            </p>
          )}
          {isError && (
            <p className="text-status-disconnected text-sm">
              {(status as { error: string }).error}
            </p>
          )}
        </div>
      )}

      {/* Canvas */}
      <canvas
        ref={canvasRef}
        className="block"
        style={canvasStyle}
      />

      {/* Toolbar */}
      {status === "connected" && (
        <RdpToolbar
          scaleMode={scaleMode}
          onScaleModeChange={setScaleMode}
          clipboardSync={clipboardSync}
          onClipboardToggle={handleClipboardToggle}
          onDisconnect={handleDisconnect}
          onFullScreen={handleFullScreen}
          onCtrlAltDel={handleCtrlAltDel}
          onScreenshot={handleScreenshot}
        />
      )}
    </div>
  );
}
