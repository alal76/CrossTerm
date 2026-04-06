import { useState, useEffect, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import {
  MonitorOff,
  Maximize,
  Keyboard,
  Scaling,
  ClipboardCopy,
  Camera,
} from "lucide-react";
import type { RdpScaleMode } from "@/types";

interface RdpToolbarProps {
  readonly scaleMode: RdpScaleMode;
  readonly onScaleModeChange: (mode: RdpScaleMode) => void;
  readonly clipboardSync: boolean;
  readonly onClipboardToggle: () => void;
  readonly onDisconnect: () => void;
  readonly onFullScreen: () => void;
  readonly onCtrlAltDel: () => void;
  readonly onScreenshot: () => void;
}

const HIDE_DELAY = 3000;
const SHOW_EDGE_PX = 32;

const SCALE_MODES: { value: RdpScaleMode; labelKey: string }[] = [
  { value: "fit", labelKey: "rdp.fitToPane" },
  { value: "actual", labelKey: "rdp.actualSize" },
  { value: "fit-width", labelKey: "rdp.fitWidth" },
  { value: "fit-height", labelKey: "rdp.fitHeight" },
];

export default function RdpToolbar({
  scaleMode,
  onScaleModeChange,
  clipboardSync,
  onClipboardToggle,
  onDisconnect,
  onFullScreen,
  onCtrlAltDel,
  onScreenshot,
}: RdpToolbarProps) {
  const { t } = useTranslation();
  const [visible, setVisible] = useState(true);
  const [scaleSelectorOpen, setScaleSelectorOpen] = useState(false);
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const toolbarRef = useRef<HTMLDivElement>(null);

  const resetHideTimer = useCallback(() => {
    if (hideTimerRef.current) {
      clearTimeout(hideTimerRef.current);
    }
    setVisible(true);
    hideTimerRef.current = setTimeout(() => {
      setVisible(false);
      setScaleSelectorOpen(false);
    }, HIDE_DELAY);
  }, []);

  // Auto-hide after 3s
  useEffect(() => {
    resetHideTimer();
    return () => {
      if (hideTimerRef.current) {
        clearTimeout(hideTimerRef.current);
      }
    };
  }, [resetHideTimer]);

  // Show on mouse near top edge
  useEffect(() => {
    function handleMouseMove(e: MouseEvent) {
      if (e.clientY <= SHOW_EDGE_PX) {
        resetHideTimer();
      }
    }
    globalThis.addEventListener("mousemove", handleMouseMove);
    return () => globalThis.removeEventListener("mousemove", handleMouseMove);
  }, [resetHideTimer]);

  const handleScaleModeSelect = useCallback(
    (mode: RdpScaleMode) => {
      onScaleModeChange(mode);
      setScaleSelectorOpen(false);
      resetHideTimer();
    },
    [onScaleModeChange, resetHideTimer]
  );

  return (
    <div
      ref={toolbarRef}
      role="toolbar"
      className={clsx(
        "absolute top-2 left-1/2 -translate-x-1/2 z-30",
        "flex items-center gap-1 rounded-lg px-2 py-1",
        "bg-surface-overlay/90 backdrop-blur-sm shadow-lg",
        "transition-opacity duration-200",
        visible ? "opacity-100" : "opacity-0 pointer-events-none"
      )}
      onMouseEnter={resetHideTimer}
    >
      {/* Disconnect */}
      <ToolbarButton
        title={t("rdp.disconnect")}
        onClick={onDisconnect}
        variant="danger"
      >
        <MonitorOff size={16} />
      </ToolbarButton>

      {/* Full Screen */}
      <ToolbarButton title={t("rdp.fullScreen")} onClick={onFullScreen}>
        <Maximize size={16} />
      </ToolbarButton>

      {/* Ctrl+Alt+Del */}
      <ToolbarButton title={t("rdp.ctrlAltDel")} onClick={onCtrlAltDel}>
        <Keyboard size={16} />
      </ToolbarButton>

      {/* Scale Mode */}
      <div className="relative">
        <ToolbarButton
          title={t("rdp.scaleMode")}
          onClick={() => setScaleSelectorOpen((prev) => !prev)}
          active={scaleSelectorOpen}
        >
          <Scaling size={16} />
        </ToolbarButton>
        {scaleSelectorOpen && (
          <div className="absolute top-full mt-1 left-1/2 -translate-x-1/2 flex flex-col bg-surface-elevated rounded-md shadow-lg py-1 min-w-[140px]">
            {SCALE_MODES.map((mode) => (
              <button
                key={mode.value}
                className={clsx(
                  "px-3 py-1.5 text-xs text-left whitespace-nowrap",
                  "hover:bg-interactive-hover",
                  scaleMode === mode.value
                    ? "text-accent-primary font-medium"
                    : "text-text-primary"
                )}
                onClick={() => handleScaleModeSelect(mode.value)}
              >
                {t(mode.labelKey)}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Clipboard Sync */}
      <ToolbarButton
        title={t("rdp.clipboard")}
        onClick={onClipboardToggle}
        active={clipboardSync}
      >
        <ClipboardCopy size={16} />
      </ToolbarButton>

      {/* Screenshot */}
      <ToolbarButton title={t("rdp.screenshot")} onClick={onScreenshot}>
        <Camera size={16} />
      </ToolbarButton>
    </div>
  );
}

// ── Toolbar button helper ───────────────────────────────────────────────

interface ToolbarButtonProps {
  readonly title: string;
  readonly onClick: () => void;
  readonly children: React.ReactNode;
  readonly active?: boolean;
  readonly variant?: "default" | "danger";
}

function ToolbarButton({
  title,
  onClick,
  children,
  active,
  variant = "default",
}: ToolbarButtonProps) {
  return (
    <button
      title={title}
      onClick={onClick}
      className={clsx(
        "flex items-center justify-center rounded p-1.5",
        "transition-colors duration-100",
        variant === "danger" && "text-status-disconnected hover:bg-status-disconnected/20",
        variant !== "danger" && active && "text-accent-primary bg-interactive-active/20",
        variant !== "danger" && !active && "text-text-secondary hover:text-text-primary hover:bg-interactive-hover"
      )}
    >
      {children}
    </button>
  );
}
