import { useEffect, useRef, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import type { VncConfig, VncConnectionStatus, VncScalingMode } from "@/types";
import VncToolbar from "./VncToolbar";

/** VNC protocol requires numeric key codes — no modern standard equivalent */
function vncKeyCode(e: KeyboardEvent): number {
  return e.keyCode;
}

interface VncViewerProps {
  readonly sessionId: string;
  readonly config: VncConfig;
}

interface VncFramePayload {
  connection_id: string;
  width: number;
  height: number;
  data_base64: string;
}

export default function VncViewer({ sessionId, config }: VncViewerProps) {
  const { t } = useTranslation();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const connectionIdRef = useRef<string | null>(null);

  const [status, setStatus] = useState<VncConnectionStatus>("connecting");
  const [scalingMode, setScalingMode] = useState<VncScalingMode>("fit_to_window");
  const [viewOnly, setViewOnly] = useState(false);

  // Connect on mount
  useEffect(() => {
    let cancelled = false;

    async function connect() {
      try {
        const id = await invoke<string>("vnc_connect", { config });
        if (cancelled) {
          await invoke("vnc_disconnect", { connectionId: id });
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
        invoke("vnc_disconnect", { connectionId: id }).catch(() => {});
        connectionIdRef.current = null;
      }
    };
  }, [sessionId, config]);

  // Listen for frame events
  useEffect(() => {
    const unlisten = listen<VncFramePayload>("vnc:frame", (event) => {
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
      "vnc:disconnected",
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

  // Listen for clipboard events from server
  useEffect(() => {
    const unlisten = listen<{ connection_id: string; text: string }>(
      "vnc:clipboard",
      (event) => {
        if (event.payload.connection_id !== connectionIdRef.current) return;
        navigator.clipboard.writeText(event.payload.text).catch(() => {});
      }
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Mouse event handler
  const handleMouseEvent = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const id = connectionIdRef.current;
      if (!id || viewOnly) return;

      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const scaleX = canvas.width / rect.width;
      const scaleY = canvas.height / rect.height;
      const x = Math.round((e.clientX - rect.left) * scaleX);
      const y = Math.round((e.clientY - rect.top) * scaleY);

      let buttonMask = 0;
      if (e.buttons & 1) buttonMask |= 1;
      if (e.buttons & 2) buttonMask |= 4;
      if (e.buttons & 4) buttonMask |= 2;

      invoke("vnc_send_mouse", { connectionId: id, x, y, buttonMask }).catch(
        () => {}
      );
    },
    [viewOnly]
  );

  // Keyboard event handlers
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      const id = connectionIdRef.current;
      if (!id || viewOnly) return;
      invoke("vnc_send_key", {
        connectionId: id,
        keyCode: vncKeyCode(e),
        pressed: true,
      }).catch(() => {});
    }

    function handleKeyUp(e: KeyboardEvent) {
      const id = connectionIdRef.current;
      if (!id || viewOnly) return;
      invoke("vnc_send_key", {
        connectionId: id,
        keyCode: vncKeyCode(e),
        pressed: false,
      }).catch(() => {});
    }

    const container = containerRef.current;
    if (!container) return;

    container.addEventListener("keydown", handleKeyDown);
    container.addEventListener("keyup", handleKeyUp);

    return () => {
      container.removeEventListener("keydown", handleKeyDown);
      container.removeEventListener("keyup", handleKeyUp);
    };
  }, [viewOnly]);

  // ResizeObserver for scaling
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const observer = new ResizeObserver(() => {
      // Trigger re-render when container size changes for CSS-based scaling
    });

    observer.observe(container);

    return () => {
      observer.disconnect();
    };
  }, []);

  const handleDisconnect = useCallback(() => {
    const id = connectionIdRef.current;
    if (id) {
      invoke("vnc_disconnect", { connectionId: id }).catch(() => {});
    }
  }, []);

  const handleViewOnlyToggle = useCallback(() => {
    const id = connectionIdRef.current;
    if (!id) return;
    const next = !viewOnly;
    invoke("vnc_set_view_only", { connectionId: id, viewOnly: next })
      .then(() => setViewOnly(next))
      .catch(() => {});
  }, [viewOnly]);

  const handleScalingChange = useCallback((mode: VncScalingMode) => {
    const id = connectionIdRef.current;
    if (!id) return;
    invoke("vnc_set_scaling", { connectionId: id, mode })
      .then(() => setScalingMode(mode))
      .catch(() => {});
  }, []);

  const handleScreenshot = useCallback(async () => {
    const id = connectionIdRef.current;
    if (!id) return;
    try {
      await invoke<string>("vnc_screenshot", { connectionId: id });
    } catch {
      // screenshot failed silently
    }
  }, []);

  const handleClipboard = useCallback(async () => {
    const id = connectionIdRef.current;
    if (!id) return;
    try {
      const text = await navigator.clipboard.readText();
      await invoke("vnc_clipboard_send", { connectionId: id, text });
    } catch {
      // clipboard read failed silently
    }
  }, []);

  // Canvas scale style based on mode
  const canvasStyle = (() => {
    switch (scalingMode) {
      case "fit_to_window":
        return { width: "100%", height: "100%", objectFit: "contain" as const };
      case "scroll":
        return {};
      case "one_to_one":
        return {};
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
      role="application"
      tabIndex={0}
      className={clsx(
        "relative flex items-center justify-center w-full h-full",
        "bg-surface-primary focus:outline-none",
        scalingMode === "scroll" ? "overflow-auto" : "overflow-hidden"
      )}
    >
      {/* Connection status overlay */}
      {(isConnecting || isError || isDisconnected) && (
        <div className="absolute inset-0 z-20 flex flex-col items-center justify-center bg-surface-overlay/80">
          {isConnecting && (
            <>
              <div className="mb-3 h-8 w-8 animate-spin rounded-full border-2 border-accent-primary border-t-transparent" />
              <p className="text-text-secondary text-sm">
                {t("vnc.connecting")}
              </p>
            </>
          )}
          {isDisconnected && (
            <p className="text-text-secondary text-sm">
              {t("vnc.disconnected")}
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
        onMouseMove={handleMouseEvent}
        onMouseDown={handleMouseEvent}
        onMouseUp={handleMouseEvent}
        onContextMenu={(e) => e.preventDefault()}
      />

      {/* Toolbar */}
      {status === "connected" && (
        <VncToolbar
          scalingMode={scalingMode}
          onScalingModeChange={handleScalingChange}
          viewOnly={viewOnly}
          onViewOnlyToggle={handleViewOnlyToggle}
          onDisconnect={handleDisconnect}
          onScreenshot={handleScreenshot}
          onClipboard={handleClipboard}
        />
      )}
    </div>
  );
}
