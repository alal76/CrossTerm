import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { Search, Plus, Code2, Tag, Copy, Play, Trash2, Pencil } from "lucide-react";
import type { Snippet } from "@/types";
import SnippetEditor from "@/components/Snippets/SnippetEditor";
import SnippetInsertModal from "@/components/Snippets/SnippetInsertModal";

const PLACEHOLDER_RE = /\{\{(\w+)\}\}/g;

function hasPlaceholders(command: string): boolean {
  return PLACEHOLDER_RE.test(command);
}

export default function SnippetListPanel() {
  const { t } = useTranslation();
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [editorSnippet, setEditorSnippet] = useState<Snippet | undefined>(undefined);
  const [showEditor, setShowEditor] = useState(false);
  const [insertSnippet, setInsertSnippet] = useState<Snippet | null>(null);
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const loadSnippets = useCallback(async () => {
    try {
      if (searchQuery.trim()) {
        const results = await invoke<Snippet[]>("snippet_search", { query: searchQuery });
        setSnippets(Array.isArray(results) ? results : []);
      } else {
        const results = await invoke<Snippet[]>("snippet_list");
        setSnippets(Array.isArray(results) ? results : []);
      }
    } catch {
      // Backend unavailable during dev
    }
  }, [searchQuery]);

  useEffect(() => {
    loadSnippets();
  }, [loadSnippets]);

  const handleCopy = useCallback(async (command: string) => {
    await navigator.clipboard.writeText(command);
  }, []);

  const handleInsert = useCallback((snippet: Snippet) => {
    if (hasPlaceholders(snippet.command)) {
      setInsertSnippet(snippet);
    } else {
      handleCopy(snippet.command);
    }
  }, [handleCopy]);

  const handleDelete = useCallback(async (id: string) => {
    try {
      await invoke("snippet_delete", { id });
      setConfirmDeleteId(null);
      loadSnippets();
    } catch {
      // ignore
    }
  }, [loadSnippets]);

  const handleEditorClose = useCallback(() => {
    setShowEditor(false);
    setEditorSnippet(undefined);
  }, []);

  const handleEditorSave = useCallback(() => {
    setShowEditor(false);
    setEditorSnippet(undefined);
    loadSnippets();
  }, [loadSnippets]);

  const handleInsertComplete = useCallback(async (command: string) => {
    await navigator.clipboard.writeText(command);
    setInsertSnippet(null);
  }, []);

  return (
    <div className="flex flex-col h-full">
      {/* Search + New */}
      <div className="flex items-center gap-2 px-2 py-2 border-b border-border-subtle">
        <div className="flex items-center flex-1 gap-1.5 px-2 py-1 rounded bg-surface-sunken border border-border-subtle">
          <Search size={13} className="text-text-disabled shrink-0" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t("snippets.searchPlaceholder")}
            className="flex-1 bg-transparent text-xs text-text-primary placeholder:text-text-disabled outline-none"
          />
        </div>
        <button
          onClick={() => { setEditorSnippet(undefined); setShowEditor(true); }}
          className="flex items-center gap-1 px-2 py-1 rounded text-xs text-accent-primary hover:bg-surface-elevated transition-colors"
          title={t("snippets.newSnippet")}
        >
          <Plus size={14} />
        </button>
      </div>

      {/* Snippet list */}
      <div className="flex-1 overflow-y-auto">
        {snippets.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-2 text-text-disabled">
            <Code2 size={32} />
            <span className="text-xs">{t("snippets.emptyState")}</span>
          </div>
        ) : (
          <ul className="divide-y divide-border-subtle">
            {snippets.map((snippet) => (
              <li
                key={snippet.id}
                className="group flex flex-col gap-1 px-3 py-2 hover:bg-surface-secondary transition-colors"
              >
                <div className="flex items-center justify-between">
                  <span className="text-xs font-medium text-text-primary truncate">
                    {snippet.name}
                  </span>
                  <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                    <button
                      onClick={() => handleInsert(snippet)}
                      className="p-0.5 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary"
                      title={t("snippets.insert")}
                    >
                      <Play size={12} />
                    </button>
                    <button
                      onClick={() => handleCopy(snippet.command)}
                      className="p-0.5 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary"
                      title={t("snippets.copyToClipboard")}
                    >
                      <Copy size={12} />
                    </button>
                    <button
                      onClick={() => { setEditorSnippet(snippet); setShowEditor(true); }}
                      className="p-0.5 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary"
                      title={t("snippets.editSnippet")}
                    >
                      <Pencil size={12} />
                    </button>
                    <button
                      onClick={() => setConfirmDeleteId(snippet.id)}
                      className="p-0.5 rounded hover:bg-surface-elevated text-status-disconnected hover:text-status-disconnected"
                      title={t("snippets.deleteSnippet")}
                    >
                      <Trash2 size={12} />
                    </button>
                  </div>
                </div>
                <code className="text-[11px] text-text-secondary truncate font-mono">
                  {snippet.command}
                </code>
                {snippet.tags.length > 0 && (
                  <div className="flex items-center gap-1 flex-wrap">
                    {snippet.tags.map((tag) => (
                      <span
                        key={tag}
                        className="inline-flex items-center gap-0.5 px-1.5 py-0.5 text-[10px] rounded bg-surface-elevated text-text-secondary"
                      >
                        <Tag size={9} />
                        {tag}
                      </span>
                    ))}
                  </div>
                )}

                {/* Delete confirmation */}
                {confirmDeleteId === snippet.id && (
                  <div className="flex items-center gap-2 mt-1">
                    <span className="text-[11px] text-status-disconnected">
                      {t("snippets.confirmDelete")}
                    </span>
                    <button
                      onClick={() => handleDelete(snippet.id)}
                      className="text-[11px] px-1.5 py-0.5 rounded bg-status-disconnected text-text-inverse hover:opacity-90"
                    >
                      {t("actions.delete")}
                    </button>
                    <button
                      onClick={() => setConfirmDeleteId(null)}
                      className="text-[11px] px-1.5 py-0.5 rounded bg-surface-elevated text-text-secondary hover:text-text-primary"
                    >
                      {t("snippets.cancel")}
                    </button>
                  </div>
                )}
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Editor modal */}
      {showEditor && (
        <SnippetEditor
          snippet={editorSnippet}
          onClose={handleEditorClose}
          onSave={handleEditorSave}
        />
      )}

      {/* Insert modal */}
      {insertSnippet && (
        <SnippetInsertModal
          snippet={insertSnippet}
          onInsert={handleInsertComplete}
          onCancel={() => setInsertSnippet(null)}
        />
      )}
    </div>
  );
}
