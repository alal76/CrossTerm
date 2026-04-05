import { useState, useEffect, useRef, useMemo, useCallback } from "react";
import clsx from "clsx";
import { Zap, ArrowRight } from "lucide-react";
import { v4 as uuidv4 } from "uuid";
import { useSessionStore } from "@/stores/sessionStore";
import { SessionType } from "@/types";
import type { Session } from "@/types";

interface QuickConnectProps {
  onConnect?: (session: Session) => void;
}

function parseInput(raw: string): { user?: string; host: string; port: number } | null {
  const trimmed = raw.trim();
  if (!trimmed) return null;

  // Strip leading "ssh " if present
  const str = trimmed.replace(/^ssh\s+/i, "");

  let user: string | undefined;
  let host: string;
  let port = 22;

  // user@host:port or user@host or host:port or host
  if (str.includes("@")) {
    const [userPart, rest] = str.split("@", 2);
    user = userPart;
    if (rest.includes(":")) {
      const [h, p] = rest.split(":", 2);
      host = h;
      const parsed = Number.parseInt(p, 10);
      if (!Number.isNaN(parsed) && parsed > 0 && parsed <= 65535) port = parsed;
    } else {
      host = rest;
    }
  } else if (str.includes(":")) {
    const [h, p] = str.split(":", 2);
    host = h;
    const parsed = Number.parseInt(p, 10);
    if (!Number.isNaN(parsed) && parsed > 0 && parsed <= 65535) port = parsed;
  } else {
    host = str;
  }

  if (!host) return null;
  return { user, host, port };
}

export default function QuickConnect({ onConnect }: QuickConnectProps) {
  const [open, setOpen] = useState(false);
  const [input, setInput] = useState("");
  const [selectedIdx, setSelectedIdx] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);

  const sessions = useSessionStore((s) => s.sessions);
  const addSession = useSessionStore((s) => s.addSession);
  const openTab = useSessionStore((s) => s.openTab);
  const addRecentSession = useSessionStore((s) => s.addRecentSession);

  const close = useCallback(() => {
    setOpen(false);
    setInput("");
    setSelectedIdx(-1);
  }, []);

  // Autocomplete from saved sessions
  const suggestions = useMemo(() => {
    if (!input.trim()) return [];
    const lower = input.toLowerCase().replace(/^ssh\s+/i, "");
    return sessions
      .filter(
        (s) =>
          s.name.toLowerCase().includes(lower) ||
          s.connection.host.toLowerCase().includes(lower)
      )
      .slice(0, 5);
  }, [input, sessions]);

  // Global shortcut: Ctrl/Cmd+Shift+N
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === "n") {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
    }
    globalThis.addEventListener("keydown", onKeyDown);
    return () => globalThis.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    if (open) requestAnimationFrame(() => inputRef.current?.focus());
  }, [open]);

  function handleConnect() {
    const parsed = parseInput(input);
    if (!parsed) return;

    const now = new Date().toISOString();
    const session: Session = {
      id: uuidv4(),
      name: parsed.user ? `${parsed.user}@${parsed.host}` : parsed.host,
      type: SessionType.SSH,
      group: "",
      tags: [],
      connection: {
        host: parsed.host,
        port: parsed.port,
        protocolOptions: {
          username: parsed.user ?? "root",
        },
      },
      createdAt: now,
      updatedAt: now,
      autoReconnect: false,
      keepAliveIntervalSeconds: 60,
    };

    addSession(session);
    openTab(session);
    addRecentSession(session.id);
    onConnect?.(session);
    close();
  }

  function handleSelectSuggestion(session: Session) {
    openTab(session);
    addRecentSession(session.id);
    onConnect?.(session);
    close();
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        close();
        break;
      case "ArrowDown":
        e.preventDefault();
        setSelectedIdx((i) => Math.min(i + 1, suggestions.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelectedIdx((i) => Math.max(i - 1, -1));
        break;
      case "Enter":
        e.preventDefault();
        if (selectedIdx >= 0 && suggestions[selectedIdx]) {
          handleSelectSuggestion(suggestions[selectedIdx]);
        } else {
          handleConnect();
        }
        break;
    }
  }

  if (!open) return null;

  const parsed = parseInput(input);

  return (
    <div className="fixed inset-0 z-[9000] flex justify-center pt-[15vh]" onClick={close} onKeyDown={(e) => e.key === "Escape" && close()} role="presentation">
      <div className="absolute inset-0 bg-surface-overlay/60 backdrop-blur-sm" />
      <div
        className="relative w-full max-w-md bg-surface-elevated rounded-xl border border-border-default shadow-[var(--shadow-3)] overflow-hidden"
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => e.key === "Escape" && close()}
        style={{ animation: "paletteIn var(--duration-medium) var(--ease-decelerate)" }}
        role="dialog"
      >
        <div className="flex items-center gap-2.5 px-4 py-3 border-b border-border-subtle">
          <Zap size={16} className="text-accent-primary shrink-0" />
          <input
            ref={inputRef}
            value={input}
            onChange={(e) => {
              setInput(e.target.value);
              setSelectedIdx(-1);
            }}
            onKeyDown={handleKeyDown}
            placeholder="ssh user@host:port"
            className="flex-1 bg-transparent text-sm text-text-primary placeholder:text-text-disabled outline-none font-mono"
            spellCheck={false}
          />
          {parsed && (
            <button
              onClick={handleConnect}
              className="shrink-0 p-1.5 rounded-lg bg-accent-primary/20 hover:bg-accent-primary/30 text-accent-primary transition-colors duration-[var(--duration-micro)]"
            >
              <ArrowRight size={14} />
            </button>
          )}
        </div>

        {suggestions.length > 0 && (
          <div className="border-b border-border-subtle">
            <div className="px-3 py-1.5 text-[10px] uppercase tracking-wider text-text-disabled">
              Saved Sessions
            </div>
            {suggestions.map((s, idx) => (
              <button
                key={s.id}
                className={clsx(
                  "flex items-center gap-2 w-full px-4 py-2 text-left text-sm",
                  "transition-colors duration-[var(--duration-micro)]",
                  idx === selectedIdx
                    ? "bg-interactive-default/20 text-text-primary"
                    : "text-text-secondary hover:bg-surface-secondary hover:text-text-primary"
                )}
                onClick={() => handleSelectSuggestion(s)}
                onMouseEnter={() => setSelectedIdx(idx)}
              >
                <span className="flex-1 truncate">{s.name}</span>
                <span className="text-xs text-text-disabled font-mono">
                  {s.connection.host}:{s.connection.port}
                </span>
              </button>
            ))}
          </div>
        )}

        {parsed && (
          <div className="px-4 py-2.5 text-xs text-text-secondary flex items-center gap-2">
            <span>Connect to</span>
            <code className="bg-surface-secondary px-1.5 py-0.5 rounded text-text-primary font-mono">
              {parsed.user ? `${parsed.user}@` : ""}
              {parsed.host}:{parsed.port}
            </code>
            <span className="text-text-disabled ml-auto">↵ Enter</span>
          </div>
        )}

        {!parsed && !input && (
          <div className="px-4 py-4 text-xs text-text-disabled text-center">
            Type a hostname, user@host, or ssh user@host:port
          </div>
        )}
      </div>
    </div>
  );
}
