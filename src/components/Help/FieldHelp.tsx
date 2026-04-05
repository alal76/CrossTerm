import { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { HelpCircle } from "lucide-react";

interface FieldHelpProps {
  readonly description: string;
  readonly articleSlug?: string;
}

export default function FieldHelp({ description, articleSlug }: FieldHelpProps) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [open]);

  return (
    <div className="relative inline-flex items-center" ref={ref}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="ml-1 text-text-disabled hover:text-text-secondary transition-colors duration-[var(--duration-micro)]"
        aria-label={t("fieldHelp.toggle")}
      >
        <HelpCircle size={14} />
      </button>
      {open && (
        <div
          className="absolute left-1/2 bottom-full mb-1.5 -translate-x-1/2 w-56 bg-surface-elevated border border-border-default rounded-lg shadow-[var(--shadow-2)] p-2.5 z-[9000] text-xs text-text-secondary"
          style={{ animation: "paletteIn var(--duration-short) var(--ease-decelerate)" }}
        >
          <p>{description}</p>
          {articleSlug && (
            <button
              type="button"
              onClick={() => {
                setOpen(false);
                // Dispatch a custom event that can be picked up to open help panel
                globalThis.dispatchEvent(
                  new CustomEvent("crossterm:open-help", { detail: { slug: articleSlug } })
                );
              }}
              className="mt-1.5 text-text-link hover:underline text-[11px]"
            >
              {t("fieldHelp.learnMore")}
            </button>
          )}
        </div>
      )}
    </div>
  );
}
