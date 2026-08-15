import { useEffect, useRef, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import clsx from "clsx";
import { LogOut } from "lucide-react";
import type { SpiceConsoleConfig, SpiceConnectResult, SpiceFrameEvent, SpiceDisconnectedEvent } from "@/types";
import { keyboardEventToScancode } from "@/utils/spiceScancodes";

interface SpiceViewerProps {
  readonly sessionId: string;
  readonly config: SpiceConsoleConfig;
}

type SpiceStatus = "connecting" | "connected" | "disconnected" | { error: string };

export default function SpiceViewer({ sessionId, config }: SpiceViewerProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const connectionIdRef = useRef<string | null>(null);

  const [status, setStatus] = useState<SpiceStatus>("connecting");

  // Connect on mount — spice_connect returns { id, width, height } so canvas
  // dimensions are set synchronously, avoiding the event-before-invoke race.
  useEffect(() => {
    let cancelled = false;

    invoke<SpiceConnectResult>("spice_connect", { config })
      .then((result) => {
        if (cancelled) {
          invoke("spice_disconnect", { connectionId: result.id }).catch(() => {});
          return;
        }
        connectionIdRef.current = result.id;
        const canvas = canvasRef.current;
        if (canvas) {
          canvas.width = result.width;
          canvas.height = result.height;
        }
        setStatus("connected");
      })
      .catch((err) => {
        if (!cancelled) setStatus({ error: String(err) });
      });

    return () => {
      cancelled = true;
      const id = connectionIdRef.current;
      if (id) {
        invoke("spice_disconnect", { connectionId: id }).catch(() => {});
        connectionIdRef.current = null;
      }
    };
  }, [sessionId, config]);

  // Each spice:frame carries the entire current surface (SPICE's display
  // channel doesn't expose incremental dirty rects the way VNC does), so
  // every update just resizes the canvas (if needed) and blits the whole
  // image at (0,0).
  useEffect(() => {
    const unlisten = listen<SpiceFrameEvent>("spice:frame", (event) => {
      const { connection_id, width, height, data_base64 } = event.payload;
      if (connection_id !== connectionIdRef.current) return;

      const canvas = canvasRef.current;
      if (!canvas) return;
      if (canvas.width !== width) canvas.width = width;
      if (canvas.height !== height) canvas.height = height;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;

      const raw = atob(data_base64);
      const bytes = new Uint8ClampedArray(raw.length);
      for (let i = 0; i < raw.length; i++) {
        bytes[i] = raw.codePointAt(i) ?? 0;
      }
      ctx.putImageData(new ImageData(bytes, width, height), 0, 0);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen<SpiceDisconnectedEvent>("spice:disconnected", (event) => {
      if (event.payload.connection_id === connectionIdRef.current) {
        setStatus("disconnected");
        connectionIdRef.current = null;
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Mouse — absolute positioning (matches the usual SPICE/QEMU tablet-mode
  // default); mouse-mode negotiation isn't tracked, so relative mode isn't
  // supported.
  const handleMouseEvent = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const id = connectionIdRef.current;
    if (!id) return;
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const x = Math.round((e.clientX - rect.left) * (canvas.width / rect.width));
    const y = Math.round((e.clientY - rect.top) * (canvas.height / rect.height));
    invoke("spice_send_mouse_move", { connectionId: id, x, y }).catch(() => {});
  }, []);

  const handleMouseButton = useCallback((e: React.MouseEvent<HTMLCanvasElement>, pressed: boolean) => {
    const id = connectionIdRef.current;
    if (!id) return;
    const button = e.button === 0 ? 0 : e.button === 1 ? 1 : e.button === 2 ? 2 : null;
    if (button === null) return;
    invoke("spice_send_mouse_button", { connectionId: id, button, pressed }).catch(() => {});
  }, []);

  // Keyboard — translate to a PC/AT scancode before sending.
  useEffect(() => {
    function sendKey(e: KeyboardEvent, pressed: boolean) {
      const id = connectionIdRef.current;
      if (!id) return;
      const scancode = keyboardEventToScancode(e);
      if (scancode === null) return;
      e.preventDefault();
      invoke("spice_send_key", { connectionId: id, scancode, pressed }).catch(() => {});
    }

    const container = containerRef.current;
    if (!container) return;
    const down = (e: KeyboardEvent) => sendKey(e, true);
    const up = (e: KeyboardEvent) => sendKey(e, false);
    container.addEventListener("keydown", down);
    container.addEventListener("keyup", up);
    return () => {
      container.removeEventListener("keydown", down);
      container.removeEventListener("keyup", up);
    };
  }, []);

  const handleDisconnect = useCallback(() => {
    const id = connectionIdRef.current;
    if (id) invoke("spice_disconnect", { connectionId: id }).catch(() => {});
  }, []);

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
        "bg-surface-primary focus:outline-none overflow-hidden"
      )}
    >
      {(isConnecting || isError || isDisconnected) && (
        <div className="absolute inset-0 z-20 flex flex-col items-center justify-center bg-surface-overlay/80">
          {isConnecting && (
            <>
              <div className="mb-3 h-8 w-8 animate-spin rounded-full border-2 border-accent-primary border-t-transparent" />
              <p className="text-text-secondary text-sm">Connecting to {config.host}…</p>
            </>
          )}
          {isDisconnected && <p className="text-text-secondary text-sm">Disconnected</p>}
          {isError && <p className="text-status-disconnected text-sm">{(status as { error: string }).error}</p>}
        </div>
      )}

      <canvas
        ref={canvasRef}
        className="block"
        style={{ width: "100%", height: "100%", objectFit: "contain" }}
        onMouseMove={handleMouseEvent}
        onMouseDown={(e) => handleMouseButton(e, true)}
        onMouseUp={(e) => handleMouseButton(e, false)}
        onContextMenu={(e) => e.preventDefault()}
      />

      {status === "connected" && (
        <button
          onClick={handleDisconnect}
          title="Disconnect"
          className="absolute top-2 right-2 z-10 flex items-center gap-1.5 rounded-md border border-border-default bg-surface-elevated/90 px-2 py-1 text-xs text-text-secondary hover:text-text-primary transition-colors"
        >
          <LogOut size={12} />
          Disconnect
        </button>
      )}
    </div>
  );
}
