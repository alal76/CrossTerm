import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import clsx from "clsx";
import { Globe, ChevronDown, ExternalLink } from "lucide-react";
import type { LocaleInfo } from "@/types";

export default function LocaleSelector() {
  const { t } = useTranslation();
  const [locales, setLocales] = useState<LocaleInfo[]>([]);
  const [currentLocale, setCurrentLocale] = useState("en");
  const [open, setOpen] = useState(false);

  const fetchLocales = useCallback(async () => {
    try {
      const result = await invoke<LocaleInfo[]>("l10n_list_locales");
      setLocales(result);
      const current = await invoke<string>("l10n_get_locale");
      setCurrentLocale(current);
    } catch {
      // Fallback silently
    }
  }, []);

  useEffect(() => {
    void fetchLocales();
  }, [fetchLocales]);

  const handleSelect = useCallback(
    async (code: string) => {
      try {
        await invoke("l10n_set_locale", { locale: code });
        setCurrentLocale(code);
        setOpen(false);
      } catch {
        // Handle error silently
      }
    },
    []
  );

  const selected = locales.find((l) => l.code === currentLocale);

  return (
    <div className="space-y-3">
      <label className="block text-sm font-medium text-text-secondary">
        {t("l10n.language")}
      </label>

      {/* Dropdown */}
      <div className="relative">
        <button
          onClick={() => setOpen(!open)}
          className="flex items-center justify-between w-full px-3 py-2 text-sm rounded border border-border-default bg-surface-secondary hover:bg-surface-elevated"
        >
          <span className="flex items-center gap-2">
            <Globe size={14} />
            {selected ? (
              <>
                {selected.native_name}
                {selected.rtl && (
                  <span className="px-1 py-0.5 text-[10px] rounded bg-accent-primary/10 text-accent-primary">
                    {t("l10n.rtl")}
                  </span>
                )}
              </>
            ) : (
              currentLocale
            )}
          </span>
          <ChevronDown
            size={14}
            className={clsx("transition-transform", open && "rotate-180")}
          />
        </button>

        {open && (
          <div className="absolute z-10 w-full mt-1 bg-surface-elevated border border-border-default rounded shadow-lg max-h-64 overflow-auto">
            {locales.map((locale) => (
              <button
                key={locale.code}
                onClick={() => handleSelect(locale.code)}
                className={clsx(
                  "w-full flex items-center justify-between px-3 py-2 text-sm hover:bg-interactive-hover",
                  locale.code === currentLocale && "bg-interactive-default"
                )}
              >
                <div className="flex items-center gap-2">
                  <span className="font-medium">{locale.native_name}</span>
                  <span className="text-text-secondary text-xs">
                    ({locale.name})
                  </span>
                  {locale.rtl && (
                    <span className="px-1 py-0.5 text-[10px] rounded bg-accent-primary/10 text-accent-primary">
                      RTL
                    </span>
                  )}
                </div>
                {/* Completeness bar */}
                <div className="flex items-center gap-2">
                  <div className="w-16 h-1.5 rounded-full bg-surface-sunken overflow-hidden">
                    <div
                      className="h-full rounded-full bg-accent-primary"
                      style={{ width: `${locale.completeness * 100}%` }}
                    />
                  </div>
                  <span className="text-xs text-text-secondary w-8 text-right">
                    {Math.round(locale.completeness * 100)}%
                  </span>
                </div>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Completeness info */}
      {selected && selected.completeness < 1.0 && (
        <div className="text-xs text-text-secondary">
          {t("l10n.completeness")}: {Math.round(selected.completeness * 100)}%
        </div>
      )}

      {/* Contribute link */}
      <a
        href="#"
        className="flex items-center gap-1 text-xs text-text-link hover:underline"
        onClick={(e) => e.preventDefault()}
      >
        <ExternalLink size={11} />
        {t("l10n.contribute")}
      </a>
    </div>
  );
}
