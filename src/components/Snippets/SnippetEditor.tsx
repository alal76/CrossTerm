import { useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { X } from "lucide-react";
import type { Snippet } from "@/types";

interface SnippetEditorProps {
  readonly snippet?: Snippet;
  readonly onClose: () => void;
  readonly onSave: () => void;
}

export default function SnippetEditor({ snippet, onClose, onSave }: SnippetEditorProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(snippet?.name ?? "");
  const [command, setCommand] = useState(snippet?.command ?? "");
  const [tagsInput, setTagsInput] = useState(snippet?.tags.join(", ") ?? "");
  const [saving, setSaving] = useState(false);

  const handleSave = useCallback(async () => {
    if (!name.trim() || !command.trim()) return;
    setSaving(true);
    try {
      const tags = tagsInput
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean);

      if (snippet) {
        await invoke("snippet_update", {
          id: snippet.id,
          name: name.trim(),
          command: command,
          tags,
        });
      } else {
        await invoke("snippet_create", {
          name: name.trim(),
          command: command,
          tags,
        });
      }
      onSave();
    } catch {
      // ignore
    } finally {
      setSaving(false);
    }
  }, [name, command, tagsInput, snippet, onSave]);

  return (
    <div className="fixed inset-0 z-[9000] flex items-center justify-center bg-black/40">
      <div
        className="w-full max-w-md bg-surface-elevated border border-border-default rounded-lg shadow-[var(--shadow-3)]"
        style={{ animation: "paletteIn var(--duration-short) var(--ease-decelerate)" }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-subtle">
          <h2 className="text-sm font-semibold text-text-primary">
            {snippet ? t("snippets.editSnippet") : t("snippets.newSnippet")}
          </h2>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-surface-secondary text-text-secondary hover:text-text-primary"
          >
            <X size={14} />
          </button>
        </div>

        {/* Body */}
        <div className="flex flex-col gap-3 px-4 py-4">
          <label className="flex flex-col gap-1">
            <span className="text-xs text-text-secondary">{t("snippets.name")}</span>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="px-2 py-1.5 rounded bg-surface-sunken border border-border-subtle text-xs text-text-primary placeholder:text-text-disabled outline-none focus:border-border-focus"
              autoFocus
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-xs text-text-secondary">{t("snippets.command")}</span>
            <textarea
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              rows={4}
              className="px-2 py-1.5 rounded bg-surface-sunken border border-border-subtle text-xs text-text-primary font-mono placeholder:text-text-disabled outline-none focus:border-border-focus resize-y"
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-xs text-text-secondary">{t("snippets.tags")}</span>
            <input
              type="text"
              value={tagsInput}
              onChange={(e) => setTagsInput(e.target.value)}
              placeholder={t("snippets.tagsHint")}
              className="px-2 py-1.5 rounded bg-surface-sunken border border-border-subtle text-xs text-text-primary placeholder:text-text-disabled outline-none focus:border-border-focus"
            />
          </label>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-border-subtle">
          <button
            onClick={onClose}
            className="px-3 py-1.5 rounded text-xs text-text-secondary hover:bg-surface-secondary transition-colors"
          >
            {t("snippets.cancel")}
          </button>
          <button
            onClick={handleSave}
            disabled={saving || !name.trim() || !command.trim()}
            className="px-3 py-1.5 rounded text-xs bg-accent-primary text-text-inverse hover:opacity-90 disabled:opacity-50 transition-colors"
          >
            {t("snippets.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
