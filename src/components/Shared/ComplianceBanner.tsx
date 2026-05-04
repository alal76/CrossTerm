import { Video } from "lucide-react";
import clsx from "clsx";

export interface ComplianceBannerProps {
  /** Show the banner only when recording is active due to policy. */
  isVisible: boolean;
  /** The connected hostname shown in the banner text. */
  hostname: string;
  /** Identifier for the current session (reserved for future use / aria). */
  sessionId: string;
  /** When `true`, a "Stop recording" link is rendered if `onDisableRecording`
   *  is also provided. Sourced from `policy.recording.allow_user_disable`. */
  allowUserDisable?: boolean;
  /** Callback invoked when the user requests recording to be stopped.
   *  Only rendered when both `allowUserDisable` and this prop are present. */
  onDisableRecording?: () => void;
}

/**
 * ComplianceBanner
 *
 * A slim, non-dismissible banner displayed at the top of the terminal area
 * whenever the active session is being recorded due to an organisational
 * policy.  The banner informs the user that their activity is being captured.
 *
 * It is intentionally non-dismissible; the only way to remove it is to either
 * end the session or (if the policy permits it) stop recording via the
 * "Stop recording" link.
 */
export default function ComplianceBanner({
  isVisible,
  hostname,
  sessionId,
  allowUserDisable = false,
  onDisableRecording,
}: Readonly<ComplianceBannerProps>) {
  if (!isVisible) return null;

  const canDisable = allowUserDisable && typeof onDisableRecording === "function";

  return (
    <div
      role="status"
      aria-live="polite"
      aria-label={`Recording active for session ${sessionId}`}
      className={clsx(
        // Layout: fixed slim bar across the top of the terminal viewport
        "absolute top-0 inset-x-0 z-30",
        "h-7 flex items-center justify-between gap-2 px-3",
        // Colour: orange/amber to signal compliance state without full alarm
        "bg-orange-500/90 text-white",
        // Subtle inner shadow to visually separate from terminal content
        "shadow-[0_1px_3px_rgba(0,0,0,0.35)]",
      )}
    >
      {/* Left side — recording indicator + label */}
      <div className="flex items-center gap-1.5 min-w-0">
        {/* Pulsing red dot */}
        <span
          className="shrink-0 animate-pulse rounded-full bg-red-400 w-2 h-2"
          aria-hidden="true"
        />

        {/* Camera / video icon */}
        <Video size={12} className="shrink-0 opacity-90" aria-hidden="true" />

        {/* Recording notice text */}
        <span className="text-[11px] font-medium leading-none truncate">
          This session is being recorded
          {hostname ? (
            <>
              {" · "}
              <span className="font-mono opacity-90">{hostname}</span>
            </>
          ) : null}
        </span>
      </div>

      {/* Right side — optional "Stop recording" action */}
      {canDisable ? (
        <button
          type="button"
          onClick={onDisableRecording}
          className={clsx(
            "shrink-0 text-[10px] font-medium underline underline-offset-2",
            "opacity-90 hover:opacity-100 transition-opacity duration-[var(--duration-micro)]",
            "focus:outline-none focus-visible:ring-1 focus-visible:ring-white/70 rounded-sm",
          )}
        >
          Stop recording
        </button>
      ) : null}
    </div>
  );
}
