import { useRef, useMemo } from "react";

interface TimestampJumperProps {
  onJump: (targetTime: Date) => void;
  onClose: () => void;
}

export default function TimestampJumper({ onJump, onClose }: TimestampJumperProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  function handleJump() {
    const value = inputRef.current?.value;
    if (!value) return;
    const date = new Date(value);
    if (!isNaN(date.getTime())) {
      onJump(date);
    }
  }

  return (
    <div
      style={{
        position: "absolute",
        bottom: 8,
        right: 8,
        background: "var(--bg-secondary, #1e1e2e)",
        padding: 6,
        borderRadius: 4,
        display: "flex",
        gap: 4,
        alignItems: "center",
      }}
    >
      <label
        htmlFor="timestamp-jumper-input"
        style={{ fontSize: 12, color: "var(--text-secondary, #9ca3af)", whiteSpace: "nowrap" }}
      >
        Jump to:
      </label>
      <input
        id="timestamp-jumper-input"
        ref={inputRef}
        type="datetime-local"
        style={{
          fontSize: 12,
          padding: "2px 6px",
          borderRadius: 3,
          border: "1px solid var(--border-default, #374151)",
          background: "var(--bg-primary, #111827)",
          color: "var(--text-primary, #f9fafb)",
        }}
      />
      <button
        onClick={handleJump}
        style={{
          fontSize: 12,
          padding: "2px 8px",
          borderRadius: 3,
          border: "none",
          background: "var(--accent-primary, #3b82f6)",
          color: "#fff",
          cursor: "pointer",
        }}
      >
        Jump
      </button>
      <button
        onClick={onClose}
        aria-label="Close"
        style={{
          fontSize: 14,
          lineHeight: 1,
          padding: "2px 6px",
          borderRadius: 3,
          border: "none",
          background: "transparent",
          color: "var(--text-secondary, #9ca3af)",
          cursor: "pointer",
        }}
      >
        &times;
      </button>
    </div>
  );
}

const TIMESTAMP_RE = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/;

export function useTimestampIndex(lines: string[]): Map<number, number> {
  return useMemo(() => {
    const map = new Map<number, number>();
    for (let i = 0; i < lines.length; i++) {
      const prefix = lines[i].slice(0, 25);
      const match = TIMESTAMP_RE.exec(prefix);
      if (match) {
        const ts = new Date(match[0]).getTime();
        if (!isNaN(ts)) {
          map.set(ts, i);
        }
      }
    }
    return map;
  }, [lines]);
}
