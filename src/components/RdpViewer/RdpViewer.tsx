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
  x: number;
  y: number;
  width: number;
  height: number;
  data_base64: string;
}

// ── PS/2 Set-1 scan code table ───────────────────────────────────────────
// Maps KeyboardEvent.code → { scan: number; extended?: boolean }
interface ScanEntry { scan: number; extended?: boolean }

const SCAN_MAP: Record<string, ScanEntry> = {
  // Letters
  KeyA: { scan: 0x1e }, KeyB: { scan: 0x30 }, KeyC: { scan: 0x2e },
  KeyD: { scan: 0x20 }, KeyE: { scan: 0x12 }, KeyF: { scan: 0x21 },
  KeyG: { scan: 0x22 }, KeyH: { scan: 0x23 }, KeyI: { scan: 0x17 },
  KeyJ: { scan: 0x24 }, KeyK: { scan: 0x25 }, KeyL: { scan: 0x26 },
  KeyM: { scan: 0x32 }, KeyN: { scan: 0x31 }, KeyO: { scan: 0x18 },
  KeyP: { scan: 0x19 }, KeyQ: { scan: 0x10 }, KeyR: { scan: 0x13 },
  KeyS: { scan: 0x1f }, KeyT: { scan: 0x14 }, KeyU: { scan: 0x16 },
  KeyV: { scan: 0x2f }, KeyW: { scan: 0x11 }, KeyX: { scan: 0x2d },
  KeyY: { scan: 0x15 }, KeyZ: { scan: 0x2c },
  // Digits
  Digit1: { scan: 0x02 }, Digit2: { scan: 0x03 }, Digit3: { scan: 0x04 },
  Digit4: { scan: 0x05 }, Digit5: { scan: 0x06 }, Digit6: { scan: 0x07 },
  Digit7: { scan: 0x08 }, Digit8: { scan: 0x09 }, Digit9: { scan: 0x0a },
  Digit0: { scan: 0x0b },
  // Editing
  Escape: { scan: 0x01 }, Backspace: { scan: 0x0e }, Tab: { scan: 0x0f },
  Enter: { scan: 0x1c }, Space: { scan: 0x39 }, CapsLock: { scan: 0x3a },
  // Punctuation
  Minus: { scan: 0x0c }, Equal: { scan: 0x0d },
  BracketLeft: { scan: 0x1a }, BracketRight: { scan: 0x1b },
  Backslash: { scan: 0x2b }, Semicolon: { scan: 0x27 },
  Quote: { scan: 0x28 }, Backquote: { scan: 0x29 },
  Comma: { scan: 0x33 }, Period: { scan: 0x34 }, Slash: { scan: 0x35 },
  // Modifiers
  ShiftLeft: { scan: 0x2a }, ShiftRight: { scan: 0x36 },
  ControlLeft: { scan: 0x1d }, ControlRight: { scan: 0x1d, extended: true },
  AltLeft: { scan: 0x38 }, AltRight: { scan: 0x38, extended: true },
  MetaLeft: { scan: 0x5b, extended: true }, MetaRight: { scan: 0x5c, extended: true },
  // Function keys
  F1: { scan: 0x3b }, F2: { scan: 0x3c }, F3: { scan: 0x3d }, F4: { scan: 0x3e },
  F5: { scan: 0x3f }, F6: { scan: 0x40 }, F7: { scan: 0x41 }, F8: { scan: 0x42 },
  F9: { scan: 0x43 }, F10: { scan: 0x44 }, F11: { scan: 0x57 }, F12: { scan: 0x58 },
  // Navigation (all extended)
  Insert: { scan: 0x52, extended: true }, Delete: { scan: 0x53, extended: true },
  Home: { scan: 0x47, extended: true }, End: { scan: 0x4f, extended: true },
  PageUp: { scan: 0x49, extended: true }, PageDown: { scan: 0x51, extended: true },
  ArrowUp: { scan: 0x48, extended: true }, ArrowDown: { scan: 0x50, extended: true },
  ArrowLeft: { scan: 0x4b, extended: true }, ArrowRight: { scan: 0x4d, extended: true },
  // Numpad
  Numpad0: { scan: 0x52 }, Numpad1: { scan: 0x4f }, Numpad2: { scan: 0x50 },
  Numpad3: { scan: 0x51 }, Numpad4: { scan: 0x4b }, Numpad5: { scan: 0x4c },
  Numpad6: { scan: 0x4d }, Numpad7: { scan: 0x47 }, Numpad8: { scan: 0x48 },
  Numpad9: { scan: 0x49 }, NumpadDecimal: { scan: 0x53 },
  NumpadAdd: { scan: 0x4e }, NumpadSubtract: { scan: 0x4a },
  NumpadMultiply: { scan: 0x37 },
  NumpadDivide: { scan: 0x35, extended: true },
  NumpadEnter: { scan: 0x1c, extended: true },
  NumLock: { scan: 0x45 }, ScrollLock: { scan: 0x46 },
  PrintScreen: { scan: 0x37, extended: true },
};

/** Map browser `e.button` to the string the backend expects. */
function rdpMouseButton(button: number): "left" | "right" | "middle" | "none" {
  if (button === 0) return "left";
  if (button === 2) return "right";
  if (button === 1) return "middle";
  return "none";
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

  // Connect on mount — rdp_connect now returns { id, width, height } so we
  // can size the canvas synchronously without racing the rdp:connected event.
  useEffect(() => {
    let cancelled = false;

    async function connect() {
      try {
        const result = await invoke<{ id: string; width: number; height: number }>(
          "rdp_connect",
          { config }
        );
        if (cancelled) {
          await invoke("rdp_disconnect", { connectionId: result.id });
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
        invoke("rdp_disconnect", { connectionId: id }).catch(() => {});
        connectionIdRef.current = null;
      }
    };
  }, [sessionId, config]);

  // Listen for partial rect frame updates
  useEffect(() => {
    const unlisten = listen<RdpFramePayload>("rdp:frame", (event) => {
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
      "rdp:disconnected",
      (event) => {
        if (event.payload.connection_id === connectionIdRef.current) {
          setStatus("disconnected");
          connectionIdRef.current = null;
        }
      }
    );
    return () => { unlisten.then((fn) => fn()); };
  }, []);

  // Keyboard — translate e.code → PS/2 scan code and send to backend
  useEffect(() => {
    function sendKey(e: KeyboardEvent, pressed: boolean) {
      const id = connectionIdRef.current;
      if (!id) return;
      const entry = SCAN_MAP[e.code];
      if (!entry) return;
      e.preventDefault();
      invoke("rdp_send_key", {
        connectionId: id,
        event: { scan_code: entry.scan, extended: entry.extended ?? false, pressed },
      }).catch(() => {});
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

  // Mouse input
  const sendMouseEvent = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>, eventType: "move" | "down" | "up") => {
      const id = connectionIdRef.current;
      if (!id) return;

      const canvas = canvasRef.current;
      if (!canvas) return;

      const rect = canvas.getBoundingClientRect();
      const scaleX = canvas.width / rect.width;
      const scaleY = canvas.height / rect.height;
      const x = Math.round((e.clientX - rect.left) * scaleX);
      const y = Math.round((e.clientY - rect.top) * scaleY);

      invoke("rdp_send_mouse", {
        connectionId: id,
        event: { x, y, button: rdpMouseButton(e.button), event_type: eventType },
      }).catch(() => {});
    },
    []
  );

  // ResizeObserver with 300 ms debounce
  const sendResize = useCallback(() => {
    const id = connectionIdRef.current;
    if (!id) return;
    const container = containerRef.current;
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const w = Math.round(rect.width);
    const h = Math.round(rect.height);
    if (w > 0 && h > 0) {
      invoke("rdp_resize", { connectionId: id, width: w, height: h }).catch(() => {});
    }
  }, []);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const observer = new ResizeObserver(() => {
      if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current);
      resizeTimerRef.current = setTimeout(sendResize, 300);
    });
    observer.observe(container);
    return () => {
      observer.disconnect();
      if (resizeTimerRef.current) clearTimeout(resizeTimerRef.current);
    };
  }, [sendResize]);

  const handleDisconnect = useCallback(() => {
    const id = connectionIdRef.current;
    if (id) invoke("rdp_disconnect", { connectionId: id }).catch(() => {});
  }, []);

  const handleCtrlAltDel = useCallback(() => {
    const id = connectionIdRef.current;
    if (id) invoke("rdp_send_ctrl_alt_del", { connectionId: id }).catch(() => {});
  }, []);

  const handleScreenshot = useCallback(async () => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const url = canvas.toDataURL("image/png");
    const a = document.createElement("a");
    a.href = url;
    a.download = `rdp-screenshot-${Date.now()}.png`;
    a.click();
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
      role="application"
      tabIndex={0}
      className={clsx(
        "relative flex items-center justify-center w-full h-full",
        "bg-surface-primary overflow-hidden focus:outline-none"
      )}
    >
      {/* Connection status overlay */}
      {(isConnecting || isError || isDisconnected) && (
        <div className="absolute inset-0 z-20 flex flex-col items-center justify-center bg-surface-overlay/80">
          {isConnecting && (
            <>
              <div className="mb-3 h-8 w-8 animate-spin rounded-full border-2 border-accent-primary border-t-transparent" />
              <p className="text-text-secondary text-sm">{t("rdp.connecting")}</p>
            </>
          )}
          {isDisconnected && (
            <p className="text-text-secondary text-sm">{t("rdp.disconnected")}</p>
          )}
          {isError && (
            <p className="text-status-disconnected text-sm">
              {(status as { error: string }).error}
            </p>
          )}
        </div>
      )}

      {/* Canvas — sized by rdp_connect return value; updated by rdp:frame events */}
      <canvas
        ref={canvasRef}
        className="block"
        style={canvasStyle}
        onMouseMove={(e) => sendMouseEvent(e, "move")}
        onMouseDown={(e) => sendMouseEvent(e, "down")}
        onMouseUp={(e) => sendMouseEvent(e, "up")}
        onContextMenu={(e) => e.preventDefault()}
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
