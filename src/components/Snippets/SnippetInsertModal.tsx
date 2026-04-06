import { useState, useMemo, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { X } from "lucide-react";
import type { Snippet } from "@/types";

const PLACEHOLDER_RE = /\{\{(\w+)\}\}/g;

interface SnippetInsertModalProps {
  readonly snippet: Snippet;
  readonly onInsert: (command: string) => void;
  readonly onCancel: () => void;
}

export default function SnippetInsertModal({ snippet, onInsert, onCancel }: SnippetInsertModalProps) {
  const { t } = useTranslation();

  const placeholders = useMemo(() => {
    const names: string[] = [];
    let match: RegExpExecArray | null;
    const re = new RegExp(PLACEHOLDER_RE.source, "g");
    while ((match = re.exec(snippet.command)) !== null) {
      if (!names.includes(match[1])) {
        names.push(match[1]);
      }
    }
    return names;
  }, [snippet.command]);

  const [values, setValues] = useState<Record<string, string>>(
    () => Object.fromEntries(placeholders.map((p) => [p, ""]))
  );

  const handleChange = useCallback((name: string, value: string) => {
    setValues((prev) => ({ ...prev, [name]: value }));
  }, []);

  const handleInsert = useCallback(() => {
    let result = snippet.command;
    for (const [name, value] of Object.entries(values)) {
      result = result.split(`{{${name}}}`).join(value);
    }
    onInsert(result);
  }, [snippet.command, values, onInsert]);

  return (
    <div className="fixed inset-0 z-[9000] flex items-center justify-center bg-black/40">
      <div
        className="w-full max-w-sm bg-surface-elevated border border-border-default rounded-lg shadow-[var(--shadow-3)]"
        style={{ animation: "paletteIn var(--duration-short) var(--ease-decelerate)" }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-subtle">
          <h2 className="text-sm font-semibold text-text-primary">
            {t("snippets.placeholderPrompt")}
          </h2>
          <button
            onClick={onCancel}
            className="p-1 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary"
          >
            <X size={14} />
          </button>
        </div>

        {/* Body */}
        <div className="flex flex-col gap-3 px-4 py-4">
          <code className="text-[11px] text-text-secondary font-mono p-2 bg-surface-sunken rounded break-all">
            {snippet.command}
          </code>

          {placeholders.map((name) => (
            <label key={name} className="flex flex-col gap-1">
              <span className="text-xs text-text-secondary font-mono">{`{{${name}}}`}</span>
              <input
                type="text"
                value={values[name] ?? ""}
                onChange={(e) => handleChange(name, e.target.value)}
                className="px-2 py-1.5 rounded bg-surface-sunken border border-border-subtle text-xs text-text-primary outline-none focus:border-border-focus"
                autoFocus={name === placeholders[0]}
              />
            </label>
          ))}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-border-subtle">
          <button
            onClick={onCancel}
            className="px-3 py-1.5 rounded text-xs text-text-secondary hover:bg-surface-secondary transition-colors"
          >
            {t("snippets.cancel")}
          </button>
          <button
            onClick={handleInsert}
            className="px-3 py-1.5 rounded text-xs bg-accent-primary text-text-inverse hover:opacity-90 transition-colors"
          >
            {t("snippets.insert")}
          </button>
        </div>
      </div>
    </div>
  );
}
