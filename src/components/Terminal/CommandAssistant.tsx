import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, ChevronRight, Copy, Sparkles, X, Zap } from "lucide-react";
import clsx from "clsx";

// ── Types ──────────────────────────────────────────────────────────────────────

type RiskLevel = "safe" | "caution" | "dangerous";

interface CommandSuggestion {
  command: string;
  explanation: string;
  risk_level: RiskLevel;
}

interface AiContext {
  current_directory?: string;
  shell?: string;
  os?: string;
  recent_commands: string[];
}

export interface CommandAssistantProps {
  sessionId: string;
  currentDirectory?: string;
  shell?: string;
  recentCommands?: string[];
  onInsertCommand: (command: string) => void;
  onClose: () => void;
}

// ── Sub-components ─────────────────────────────────────────────────────────────

function RiskBadge({ level }: { level: RiskLevel }) {
  const config = {
    safe: {
      label: "Safe",
      className: "bg-green-900/50 text-green-400 border border-green-700/50",
    },
    caution: {
      label: "Caution",
      className: "bg-yellow-900/50 text-yellow-400 border border-yellow-700/50",
    },
    dangerous: {
      label: "Dangerous",
      className: "bg-red-900/50 text-red-400 border border-red-700/50",
    },
  };

  const { label, className } = config[level] ?? config.caution;

  return (
    <span className={clsx("inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-xs font-medium", className)}>
      {level === "dangerous" && <AlertTriangle size={10} />}
      {label}
    </span>
  );
}

function SkeletonCard() {
  return (
    <div className="animate-pulse rounded-md border border-gray-700 bg-gray-800/50 p-3">
      <div className="mb-2 h-4 w-3/5 rounded bg-gray-700" />
      <div className="h-3 w-4/5 rounded bg-gray-700/70" />
    </div>
  );
}

function SuggestionCard({
  suggestion,
  onInsert,
  onCopy,
}: {
  suggestion: CommandSuggestion;
  onInsert: () => void;
  onCopy: () => void;
}) {
  return (
    <div className="rounded-md border border-gray-700 bg-gray-800/60 p-3 transition-colors hover:border-gray-500 hover:bg-gray-800">
      <div className="mb-1.5 flex items-start justify-between gap-2">
        <code className="font-mono text-sm text-white break-all">{suggestion.command}</code>
        <div className="flex shrink-0 items-center gap-1">
          <RiskBadge level={suggestion.risk_level} />
          <button
            onClick={onCopy}
            title="Copy command"
            className="rounded p-1 text-gray-400 transition-colors hover:bg-gray-700 hover:text-white"
          >
            <Copy size={13} />
          </button>
          <button
            onClick={onInsert}
            title="Insert into terminal"
            className="flex items-center gap-1 rounded bg-blue-600 px-2 py-1 text-xs text-white transition-colors hover:bg-blue-500"
          >
            Insert
            <ChevronRight size={12} />
          </button>
        </div>
      </div>
      <p className="text-xs leading-relaxed text-gray-400">{suggestion.explanation}</p>
    </div>
  );
}

// ── Main Component ─────────────────────────────────────────────────────────────

export default function CommandAssistant({
  sessionId: _sessionId,
  currentDirectory,
  shell,
  recentCommands = [],
  onInsertCommand,
  onClose,
}: CommandAssistantProps) {
  const [query, setQuery] = useState("");
  const [suggestions, setSuggestions] = useState<CommandSuggestion[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ollamaAvailable, setOllamaAvailable] = useState<boolean | null>(null);
  const [copiedIndex, setCopiedIndex] = useState<number | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Auto-focus when mounted
  useEffect(() => {
    requestAnimationFrame(() => inputRef.current?.focus());
  }, []);

  // Check Ollama availability on mount
  useEffect(() => {
    invoke<boolean>("ai_check_available")
      .then((available) => setOllamaAvailable(available))
      .catch(() => setOllamaAvailable(false));
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const trimmed = query.trim();
      if (!trimmed || loading) return;

      setLoading(true);
      setError(null);
      setSuggestions([]);

      const os = (() => {
        const ua = navigator.userAgent.toLowerCase();
        if (ua.includes("mac")) return "macos";
        if (ua.includes("win")) return "windows";
        return "linux";
      })();

      const context: AiContext = {
        current_directory: currentDirectory,
        shell,
        os,
        recent_commands: recentCommands.slice(-5),
      };

      try {
        const results = await invoke<CommandSuggestion[]>("ai_suggest_command", {
          userRequest: trimmed,
          context,
        });
        setSuggestions(results);
      } catch (err) {
        const message = typeof err === "string" ? err : "Failed to get suggestions.";
        setError(message);
      } finally {
        setLoading(false);
      }
    },
    [query, loading, currentDirectory, shell, recentCommands],
  );

  const handleInsert = useCallback(
    (command: string) => {
      onInsertCommand(command);
      onClose();
    },
    [onInsertCommand, onClose],
  );

  const handleCopy = useCallback(async (command: string, index: number) => {
    try {
      await navigator.clipboard.writeText(command);
      setCopiedIndex(index);
      setTimeout(() => setCopiedIndex(null), 1500);
    } catch {
      // clipboard may be unavailable in some contexts — silently ignore
    }
  }, []);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    },
    [onClose],
  );

  return (
    <div
      className="flex flex-col border-t border-gray-700 bg-gray-900"
      onKeyDown={handleKeyDown}
    >
      {/* Header bar */}
      <div className="flex items-center justify-between border-b border-gray-700/60 px-3 py-2">
        <div className="flex items-center gap-2 text-gray-300">
          <Sparkles size={14} className="text-blue-400" />
          <span className="text-xs font-medium">AI Command Assistant</span>
          <span className="flex items-center gap-1 text-xs text-gray-500">
            <Zap size={10} />
            Powered by Ollama (local AI)
          </span>
        </div>
        <button
          onClick={onClose}
          title="Close AI assistant"
          className="rounded p-1 text-gray-500 transition-colors hover:bg-gray-800 hover:text-gray-300"
        >
          <X size={14} />
        </button>
      </div>

      {/* Scrollable content area */}
      <div className="max-h-64 overflow-y-auto px-3 py-2">
        {/* Ollama unavailable banner */}
        {ollamaAvailable === false && (
          <div className="mb-2 flex items-center gap-2 rounded-md border border-yellow-700/50 bg-yellow-900/20 px-3 py-2 text-xs text-yellow-400">
            <AlertTriangle size={13} />
            <span>
              Ollama not running. Start Ollama to use AI features.{" "}
              <a
                href="https://ollama.ai"
                target="_blank"
                rel="noopener noreferrer"
                className="underline hover:text-yellow-300"
              >
                ollama.ai
              </a>
            </span>
          </div>
        )}

        {/* Query input */}
        <form onSubmit={handleSubmit} className="mb-2 flex gap-2">
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Ask AI... (e.g. find large files, kill process on port 3000)"
            disabled={loading || ollamaAvailable === false}
            className={clsx(
              "flex-1 rounded border px-3 py-1.5 text-sm text-white placeholder-gray-400 outline-none transition-colors",
              "bg-gray-800 border-gray-600 focus:border-blue-500",
              (loading || ollamaAvailable === false) && "cursor-not-allowed opacity-50",
            )}
          />
          <button
            type="submit"
            disabled={!query.trim() || loading || ollamaAvailable === false}
            className={clsx(
              "flex items-center gap-1.5 rounded px-3 py-1.5 text-xs font-medium transition-colors",
              "bg-blue-600 text-white hover:bg-blue-500",
              "disabled:cursor-not-allowed disabled:opacity-40",
            )}
          >
            <Sparkles size={12} />
            {loading ? "Thinking…" : "Suggest"}
          </button>
        </form>

        {/* Loading skeletons */}
        {loading && (
          <div className="flex flex-col gap-2">
            <SkeletonCard />
            <SkeletonCard />
          </div>
        )}

        {/* Error state */}
        {error && !loading && (
          <div className="flex items-start gap-2 rounded-md border border-red-700/50 bg-red-900/20 px-3 py-2 text-xs text-red-400">
            <AlertTriangle size={13} className="mt-0.5 shrink-0" />
            <span>{error}</span>
          </div>
        )}

        {/* Suggestion cards */}
        {!loading && suggestions.length > 0 && (
          <div className="flex flex-col gap-2">
            {suggestions.map((suggestion, index) => (
              <SuggestionCard
                key={`${suggestion.command}-${index}`}
                suggestion={
                  copiedIndex === index
                    ? { ...suggestion, command: "Copied!" }
                    : suggestion
                }
                onInsert={() => handleInsert(suggestion.command)}
                onCopy={() => handleCopy(suggestion.command, index)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
