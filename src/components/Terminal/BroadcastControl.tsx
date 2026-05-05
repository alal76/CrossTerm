import { useState } from "react";

interface BroadcastControlProps {
  paneId: string;
  paneName: string;
  isBroadcastEnabled: boolean;
  onToggle: (paneId: string, enabled: boolean) => void;
}

export function BroadcastControl({
  paneId,
  paneName,
  isBroadcastEnabled,
  onToggle,
}: BroadcastControlProps) {
  const switchId = `broadcast-toggle-${paneId}`;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "6px 10px",
        borderRadius: 4,
        outline: isBroadcastEnabled ? "2px solid #f97316" : "none",
        background: "var(--bg-secondary, #1e1e2e)",
        gap: 8,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
        {isBroadcastEnabled && (
          <span
            aria-label="Broadcasting"
            style={{
              display: "inline-block",
              width: 8,
              height: 8,
              borderRadius: "50%",
              background: "#f97316",
              flexShrink: 0,
            }}
          />
        )}
        <label
          htmlFor={switchId}
          style={{
            fontSize: 13,
            cursor: "pointer",
            color: "var(--text-primary, #f9fafb)",
          }}
        >
          {paneName}
        </label>
      </div>

      {/* Pill toggle switch */}
      <label
        style={{
          position: "relative",
          display: "inline-block",
          width: 36,
          height: 20,
          flexShrink: 0,
        }}
      >
        <input
          id={switchId}
          type="checkbox"
          checked={isBroadcastEnabled}
          onChange={(e) => onToggle(paneId, e.target.checked)}
          style={{ opacity: 0, width: 0, height: 0, position: "absolute" }}
        />
        <span
          style={{
            position: "absolute",
            inset: 0,
            background: isBroadcastEnabled ? "#f97316" : "#374151",
            borderRadius: 20,
            transition: "background 0.2s",
            cursor: "pointer",
          }}
        />
        <span
          style={{
            position: "absolute",
            top: 2,
            left: isBroadcastEnabled ? 18 : 2,
            width: 16,
            height: 16,
            borderRadius: "50%",
            background: "#fff",
            transition: "left 0.2s",
            pointerEvents: "none",
          }}
        />
      </label>
    </div>
  );
}

interface BroadcastManagerProps {
  panes: Array<{ id: string; name: string }>;
  onBroadcastChange: (enabledPaneIds: string[]) => void;
}

export function BroadcastManager({ panes, onBroadcastChange }: BroadcastManagerProps) {
  const [enabled, setEnabled] = useState<Set<string>>(new Set());

  function toggle(paneId: string, value: boolean) {
    setEnabled((prev) => {
      const next = new Set(prev);
      if (value) {
        next.add(paneId);
      } else {
        next.delete(paneId);
      }
      onBroadcastChange([...next]);
      return next;
    });
  }

  function enableAll() {
    const all = new Set(panes.map((p) => p.id));
    setEnabled(all);
    onBroadcastChange([...all]);
  }

  function disableAll() {
    setEnabled(new Set());
    onBroadcastChange([]);
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <div style={{ display: "flex", gap: 6, marginBottom: 4 }}>
        <button
          onClick={enableAll}
          style={{
            fontSize: 12,
            padding: "3px 10px",
            borderRadius: 3,
            border: "1px solid #f97316",
            background: "transparent",
            color: "#f97316",
            cursor: "pointer",
          }}
        >
          Enable all
        </button>
        <button
          onClick={disableAll}
          style={{
            fontSize: 12,
            padding: "3px 10px",
            borderRadius: 3,
            border: "1px solid #6b7280",
            background: "transparent",
            color: "#6b7280",
            cursor: "pointer",
          }}
        >
          Disable all
        </button>
      </div>

      {panes.map((pane) => (
        <BroadcastControl
          key={pane.id}
          paneId={pane.id}
          paneName={pane.name}
          isBroadcastEnabled={enabled.has(pane.id)}
          onToggle={toggle}
        />
      ))}
    </div>
  );
}
