import { useState, useRef, useEffect, useCallback } from "react";
import clsx from "clsx";
import { X, ChevronUp, ChevronDown, CaseSensitive, Regex } from "lucide-react";
import type { SearchAddon } from "@xterm/addon-search";

interface TerminalSearchProps {
  readonly searchAddon: SearchAddon | null;
  readonly visible: boolean;
  readonly onClose: () => void;
  readonly regexMode?: boolean;
  readonly onRegexToggle?: () => void;
}

export default function TerminalSearch({ searchAddon, visible, onClose, regexMode = false, onRegexToggle }: TerminalSearchProps) {
  const [query, setQuery] = useState("");
  const [caseSensitive, setCaseSensitive] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (visible) {
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [visible]);

  const doSearch = useCallback(
    (direction: "next" | "prev") => {
      if (!searchAddon || !query) return;
      const opts = { caseSensitive, regex: regexMode, wholeWord: false, incremental: true };
      if (direction === "next") {
        searchAddon.findNext(query, opts);
      } else {
        searchAddon.findPrevious(query, opts);
      }
    },
    [searchAddon, query, caseSensitive, regexMode],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        e.preventDefault();
        doSearch(e.shiftKey ? "prev" : "next");
      }
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    },
    [doSearch, onClose],
  );

  useEffect(() => {
    if (!visible && searchAddon) {
      searchAddon.clearDecorations();
    }
  }, [visible, searchAddon]);

  if (!visible) return null;

  return (
    <div className="absolute top-2 right-4 z-20 flex items-center gap-1 bg-surface-elevated border border-border-default rounded-md shadow-2 px-2 py-1.5 animate-fade-in">
      <input
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          if (searchAddon && e.target.value) {
            searchAddon.findNext(e.target.value, { caseSensitive, regex: regexMode, incremental: true });
          }
        }}
        onKeyDown={handleKeyDown}
        className="bg-surface-primary border border-border-subtle rounded px-2 py-1 text-xs text-text-primary w-48 outline-none focus-visible:border-border-focus"
        placeholder="Search…"
      />
      <button
        onClick={() => setCaseSensitive(!caseSensitive)}
        className={clsx(
          "p-1 rounded transition-colors",
          caseSensitive
            ? "bg-interactive-default text-text-inverse"
            : "text-text-secondary hover:text-text-primary hover:bg-surface-primary",
        )}
        title="Case sensitive"
      >
        <CaseSensitive size={14} />
      </button>
      <button
        onClick={onRegexToggle}
        className={clsx(
          "p-1 rounded transition-colors ring-offset-0",
          regexMode
            ? "bg-interactive-default text-text-inverse ring-2 ring-blue-500"
            : "text-text-secondary hover:text-text-primary hover:bg-surface-primary",
        )}
        title="Use regular expression"
      >
        <Regex size={14} />
      </button>
      <button
        onClick={() => doSearch("prev")}
        className="p-1 rounded text-text-secondary hover:text-text-primary hover:bg-surface-primary transition-colors"
        title="Previous match"
      >
        <ChevronUp size={14} />
      </button>
      <button
        onClick={() => doSearch("next")}
        className="p-1 rounded text-text-secondary hover:text-text-primary hover:bg-surface-primary transition-colors"
        title="Next match"
      >
        <ChevronDown size={14} />
      </button>
      <button
        onClick={onClose}
        className="p-1 rounded text-text-secondary hover:text-text-primary hover:bg-surface-primary transition-colors"
        title="Close"
      >
        <X size={14} />
      </button>
    </div>
  );
}
