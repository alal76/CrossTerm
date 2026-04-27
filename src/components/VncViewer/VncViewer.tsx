import { useEffect, useRef, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import type { VncConfig, VncConnectionStatus, VncScalingMode } from "@/types";
import VncToolbar from "./VncToolbar";

interface VncViewerProps {
  readonly sessionId: string;
  readonly config: VncConfig;
}

interface VncFramePayload {
  connection_id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  data_base64: string;
}

interface VncResizePayload {
  connection_id: string;
  width: number;
  height: number;
}

// ── X11 keysym lookup ────────────────────────────────────────────────────
// Maps KeyboardEvent.key to the corresponding X11 keysym value.
// Printable single characters fall through to their Unicode codepoint.
const KEYSYM_MAP: Record<string, number> = {
  Backspace: 0xff08, Tab: 0xff09, Enter: 0xff0d, Escape: 0xff1b,
  Delete: 0xffff, Home: 0xff50, End: 0xff57,
  PageUp: 0xff55, PageDown: 0xff56, Insert: 0xff63,
  ArrowLeft: 0xff51, ArrowUp: 0xff52, ArrowRight: 0xff53, ArrowDown: 0xff54,
  F1: 0xffbe, F2: 0xffbf, F3: 0xffc0, F4: 0xffc1,
  F5: 0xffc2, F6: 0xffc3, F7: 0xffc4, F8: 0xffc5,
  F9: 0xffc6, F10: 0xffc7, F11: 0xffc8, F12: 0xffc9,
  // Modifiers — location 2 = right-hand side
  Shift: 0xffe1, Control: 0xffe3, Alt: 0xffe9, Meta: 0xffeb,
  CapsLock: 0xffe5, NumLock: 0xff7f, ScrollLock: 0xff14,
  Pause: 0xff13, PrintScreen: 0xff61,
};

// Right-hand-side modifier keysyms
const KEYSYM_RIGHT: Record<string, number> = {
  Shift: 0xffe2, Control: 0xffe4, Alt: 0xffea, Meta: 0xffec,
};

function vncKeysym(e: KeyboardEvent): number {
  const key = e.key;

  // Right-hand modifiers
  if (e.location === 2 && key in KEYSYM_RIGHT) return KEYSYM_RIGHT[key];

  // Special key table
  if (key in KEYSYM_MAP) return KEYSYM_MAP[key];

  // Printable characters: use Unicode codepoint (matches X11 keysym for BMP,
  // and 0x1000000 | codepoint for supplementary planes).
  if (key.length === 1) {
    const cp = key.codePointAt(0) ?? 0;
    return cp > 0xff ? 0x1000000 | cp : cp;
  }

  return 0;
}

export default function VncViewer({ sessionId, config }: VncViewerProps) {
  const { t } = useTranslation();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const connectionIdRef = useRef<string | null>(null);

  const [status, setStatus] = useState<VncConnectionStatus>("connecting");
  const [scalingMode, setScalingMode] = useState<VncScalingMode>("fit_to_window");
  const [viewOnly, setViewOnly] = useState(false);

  // Connect on mount — vnc_connect returns { id, width, height } so canvas
  // dimensions are set synchronously, avoiding the event-before-invoke race.
  useEffect(() => {
    let cancelled = false;

    async function connect() {
      try {
        const result = await invoke<{ id: string; width: number; height: number }>(
          "vnc_connect",
          { config }
        );
        if (cancelled) {
          await invoke("vnc_disconnect", { connectionId: result.id });
          return;
        }
        connectionIdRef.current = result.id;
        const canvas = canvasRef.current;
        if (canvas) {
          canvas.width = result.width;
          canvas.height = result.height;
        }
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

  // Server-initiated resize (e.g. DesktopSize pseudo-encoding)
  useEffect(() => {
    const unlisten = listen<VncResizePayload>("vnc:resize", (event) => {
      const { connection_id, width, height } = event.payload;
      if (connection_id !== connectionIdRef.current) return;
      const canvas = canvasRef.current;
      if (!canvas) return;
      canvas.width = width;
      canvas.height = height;
    });
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // Listen for partial rect frame updates
  useEffect(() => {
    const unlisten = listen<VncFramePayload>("vnc:frame", (event) => {
      const { connection_id, x, y, width, height, data_base64 } = event.payload;
      if (connection_id !== connectionIdRef.current) return;

      const canvas = canvasRef.current;
      if (!canvas) return;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;

      const raw = atob(data_base64);
      const bytes = new Uint8ClampedArray(raw.length);
      for (let i = 0; i < raw.length; i++) {
        bytes[i] = raw.codePointAt(i) ?? 0;
      }

      ctx.putImageData(new ImageData(bytes, width, height), x, y);
    });

    return () => { unlisten.then((fn) => fn()); };
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
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // Copy server clipboard to local clipboard
  useEffect(() => {
    const unlisten = listen<{ connection_id: string; text: string }>(
      "vnc:clipboard",
      (event) => {
        if (event.payload.connection_id !== connectionIdRef.current) return;
        navigator.clipboard.writeText(event.payload.text).catch(() => {});
      }
    );
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // Mouse — translate canvas coords then send button mask
  const handleMouseEvent = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const id = connectionIdRef.current;
      if (!id || viewOnly) return;

      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const x = Math.round((e.clientX - rect.left) * (canvas.width / rect.width));
      const y = Math.round((e.clientY - rect.top) * (canvas.height / rect.height));

      let buttonMask = 0;
      if (e.buttons & 1) buttonMask |= 1;
      if (e.buttons & 2) buttonMask |= 4;
      if (e.buttons & 4) buttonMask |= 2;

      invoke("vnc_send_mouse", { connectionId: id, x, y, buttonMask }).catch(() => {});
    },
    [viewOnly]
  );

  // Keyboard — translate to X11 keysym before sending
  useEffect(() => {
    function sendKey(e: KeyboardEvent, pressed: boolean) {
      const id = connectionIdRef.current;
      if (!id || viewOnly) return;
      const keysym = vncKeysym(e);
      if (!keysym) return;
      e.preventDefault();
      invoke("vnc_send_key", { connectionId: id, keyCode: keysym, pressed }).catch(() => {});
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
  }, [viewOnly]);

  const handleDisconnect = useCallback(() => {
    const id = connectionIdRef.current;
    if (id) invoke("vnc_disconnect", { connectionId: id }).catch(() => {});
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

  const handleScreenshot = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const url = canvas.toDataURL("image/png");
    const a = document.createElement("a");
    a.href = url;
    a.download = `vnc-screenshot-${Date.now()}.png`;
    a.click();
  }, []);

  const handleClipboard = useCallback(async () => {
    const id = connectionIdRef.current;
    if (!id) return;
    try {
      const text = await navigator.clipboard.readText();
      await invoke("vnc_clipboard_send", { connectionId: id, text });
    } catch {
      // silent
    }
  }, []);

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
              <p className="text-text-secondary text-sm">{t("vnc.connecting")}</p>
            </>
          )}
          {isDisconnected && (
            <p className="text-text-secondary text-sm">{t("vnc.disconnected")}</p>
          )}
          {isError && (
            <p className="text-status-disconnected text-sm">
              {(status as { error: string }).error}
            </p>
          )}
        </div>
      )}

      {/* Canvas — sized by vnc_connect return value; updated by vnc:frame events */}
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
