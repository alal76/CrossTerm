import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { useTranslation } from "react-i18next";
import clsx from "clsx";
import { ChevronLeft, ChevronRight, X } from "lucide-react";
import type { TourStep } from "@/types";

// ── Tour Definitions ──

function useTourDefinitions(): Record<string, TourStep[]> {
  const { t } = useTranslation();

  return useMemo(
    () => ({
      ssh: [
        {
          targetSelector: "[data-tour='new-session']",
          title: t("tour.ssh.step1Title"),
          description: t("tour.ssh.step1Desc"),
          position: "bottom" as const,
        },
        {
          targetSelector: "[data-tour='quick-connect']",
          title: t("tour.ssh.step2Title"),
          description: t("tour.ssh.step2Desc"),
          position: "bottom" as const,
        },
        {
          targetSelector: "[data-tour='session-canvas']",
          title: t("tour.ssh.step3Title"),
          description: t("tour.ssh.step3Desc"),
          position: "top" as const,
        },
      ],
      sftp: [
        {
          targetSelector: "[data-tour='bottom-panel']",
          title: t("tour.sftp.step1Title"),
          description: t("tour.sftp.step1Desc"),
          position: "top" as const,
        },
        {
          targetSelector: "[data-help-article='sftp-file-transfer']",
          title: t("tour.sftp.step2Title"),
          description: t("tour.sftp.step2Desc"),
          position: "top" as const,
        },
        {
          targetSelector: "[data-help-article='sftp-file-transfer']",
          title: t("tour.sftp.step3Title"),
          description: t("tour.sftp.step3Desc"),
          position: "left" as const,
        },
      ],
      vault: [
        {
          targetSelector: "[data-tour='sidebar-sessions']",
          title: t("tour.vault.step1Title"),
          description: t("tour.vault.step1Desc"),
          position: "right" as const,
        },
        {
          targetSelector: "[data-help-article='credential-vault']",
          title: t("tour.vault.step2Title"),
          description: t("tour.vault.step2Desc"),
          position: "bottom" as const,
        },
        {
          targetSelector: "[data-help-article='credential-vault']",
          title: t("tour.vault.step3Title"),
          description: t("tour.vault.step3Desc"),
          position: "right" as const,
        },
      ],
    }),
    [t],
  );
}

// ── Completed tours storage ──

const COMPLETED_TOURS_KEY = "crossterm_completed_tours";

function getCompletedTours(): Set<string> {
  try {
    const stored = localStorage.getItem(COMPLETED_TOURS_KEY);
    if (stored) {
      return new Set(JSON.parse(stored) as string[]);
    }
  } catch {
    // Ignore parse errors
  }
  return new Set();
}

function markTourCompleted(tourId: string) {
  const completed = getCompletedTours();
  completed.add(tourId);
  localStorage.setItem(COMPLETED_TOURS_KEY, JSON.stringify([...completed]));
}

// ── Spotlight cutout calculation ──

interface Rect {
  top: number;
  left: number;
  width: number;
  height: number;
}

function getTargetRect(selector: string): Rect | null {
  const el = document.querySelector(selector);
  if (!el) return null;
  const r = el.getBoundingClientRect();
  const padding = 8;
  return {
    top: r.top - padding,
    left: r.left - padding,
    width: r.width + padding * 2,
    height: r.height + padding * 2,
  };
}

// ── Popover position ──

function getPopoverStyle(
  rect: Rect,
  position: TourStep["position"],
): React.CSSProperties {
  const gap = 12;
  switch (position) {
    case "top":
      return {
        bottom: window.innerHeight - rect.top + gap,
        left: rect.left + rect.width / 2,
        transform: "translateX(-50%)",
      };
    case "bottom":
      return {
        top: rect.top + rect.height + gap,
        left: rect.left + rect.width / 2,
        transform: "translateX(-50%)",
      };
    case "left":
      return {
        top: rect.top + rect.height / 2,
        right: window.innerWidth - rect.left + gap,
        transform: "translateY(-50%)",
      };
    case "right":
      return {
        top: rect.top + rect.height / 2,
        left: rect.left + rect.width + gap,
        transform: "translateY(-50%)",
      };
  }
}

// ── Component ──

interface FeatureTourProps {
  readonly tourId: string;
  readonly onComplete: () => void;
}

export default function FeatureTour({ tourId, onComplete }: FeatureTourProps) {
  const { t } = useTranslation();
  const definitions = useTourDefinitions();
  const steps = definitions[tourId];
  const [currentStep, setCurrentStep] = useState(0);
  const [targetRect, setTargetRect] = useState<Rect | null>(null);
  const popoverRef = useRef<HTMLDivElement>(null);

  const step = steps?.[currentStep];

  // Update target rect on step change and on resize/scroll
  const updateRect = useCallback(() => {
    if (step) {
      setTargetRect(getTargetRect(step.targetSelector));
    }
  }, [step]);

  useEffect(() => {
    updateRect();
    window.addEventListener("resize", updateRect);
    window.addEventListener("scroll", updateRect, true);
    return () => {
      window.removeEventListener("resize", updateRect);
      window.removeEventListener("scroll", updateRect, true);
    };
  }, [updateRect]);

  // Close on ESC
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onComplete();
      }
    }
    globalThis.addEventListener("keydown", handleKeyDown);
    return () => globalThis.removeEventListener("keydown", handleKeyDown);
  }, [onComplete]);

  if (!steps || steps.length === 0) {
    return null;
  }

  const handleNext = () => {
    if (currentStep < steps.length - 1) {
      setCurrentStep((s) => s + 1);
    } else {
      markTourCompleted(tourId);
      onComplete();
    }
  };

  const handlePrevious = () => {
    if (currentStep > 0) {
      setCurrentStep((s) => s - 1);
    }
  };

  const handleSkip = () => {
    markTourCompleted(tourId);
    onComplete();
  };

  const isLast = currentStep === steps.length - 1;

  return (
    <div className="fixed inset-0 z-[9000]">
      {/* Semi-transparent overlay with cutout */}
      <svg className="absolute inset-0 w-full h-full pointer-events-auto" onClick={handleSkip}>
        <defs>
          <mask id="tour-spotlight-mask">
            <rect x="0" y="0" width="100%" height="100%" fill="white" />
            {targetRect && (
              <rect
                x={targetRect.left}
                y={targetRect.top}
                width={targetRect.width}
                height={targetRect.height}
                rx="8"
                fill="black"
              />
            )}
          </mask>
        </defs>
        <rect
          x="0"
          y="0"
          width="100%"
          height="100%"
          fill="rgba(0,0,0,0.55)"
          mask="url(#tour-spotlight-mask)"
        />
      </svg>

      {/* Spotlight border ring */}
      {targetRect && (
        <div
          className="tour-spotlight absolute rounded-lg border-2 border-accent-primary pointer-events-none"
          style={{
            top: targetRect.top,
            left: targetRect.left,
            width: targetRect.width,
            height: targetRect.height,
            boxShadow: "0 0 0 4px rgba(var(--accent-primary-rgb, 99, 102, 241), 0.25)",
          }}
        />
      )}

      {/* Popover card */}
      {step && (
        <div
          ref={popoverRef}
          className="tour-popover absolute w-72 bg-surface-elevated border border-border-default rounded-xl shadow-[var(--shadow-3)] p-4 pointer-events-auto"
          style={targetRect ? getPopoverStyle(targetRect, step.position) : { top: "50%", left: "50%", transform: "translate(-50%, -50%)" }}
        >
          <div className="flex items-start justify-between mb-2">
            <h3 className="text-sm font-semibold text-text-primary">{step.title}</h3>
            <button
              onClick={handleSkip}
              className="p-0.5 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors"
            >
              <X size={14} />
            </button>
          </div>
          <p className="text-xs text-text-secondary mb-4">{step.description}</p>
          <div className="flex items-center justify-between">
            <span className="text-[10px] text-text-disabled">
              {t("tour.stepOf", { current: currentStep + 1, total: steps.length })}
            </span>
            <div className="flex items-center gap-1.5">
              {currentStep > 0 && (
                <button
                  onClick={handlePrevious}
                  className="flex items-center gap-1 px-2 py-1 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary hover:text-text-primary transition-colors"
                >
                  <ChevronLeft size={12} />
                  {t("tour.previous")}
                </button>
              )}
              {!isLast && (
                <button
                  onClick={handleSkip}
                  className="px-2 py-1 text-xs text-text-disabled hover:text-text-secondary transition-colors"
                >
                  {t("tour.skip")}
                </button>
              )}
              <button
                onClick={handleNext}
                className={clsx(
                  "flex items-center gap-1 px-3 py-1 text-xs rounded-lg transition-colors",
                  "bg-interactive-default hover:bg-interactive-hover text-text-primary",
                )}
              >
                {isLast ? t("tour.finish") : t("tour.next")}
                {!isLast && <ChevronRight size={12} />}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
