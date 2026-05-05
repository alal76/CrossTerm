import { useEffect } from "react";

type Direction = "auto" | "ltr" | "rtl";

interface RtlSettingsProps {
  enabled: boolean;
  direction: Direction;
  onChange: (enabled: boolean, direction: Direction) => void;
}

export default function RtlSettings({ enabled, direction, onChange }: RtlSettingsProps) {
  useEffect(() => {
    if (enabled) {
      document.documentElement.dir = direction;
    }
  }, [enabled, direction]);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
      <p style={{ fontSize: 12, color: "var(--text-secondary, #9ca3af)", margin: 0 }}>
        RTL support enables proper display of Arabic, Hebrew, and other
        right-to-left languages.
      </p>

      {/* Enable toggle */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
        <label
          htmlFor="rtl-enabled-toggle"
          style={{ fontSize: 13, cursor: "pointer", color: "var(--text-primary, #f9fafb)" }}
        >
          Enable RTL text support
        </label>
        <input
          id="rtl-enabled-toggle"
          type="checkbox"
          checked={enabled}
          onChange={(e) => onChange(e.target.checked, direction)}
          style={{ width: 16, height: 16, cursor: "pointer" }}
        />
      </div>

      {/* Direction dropdown */}
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <label
          htmlFor="rtl-direction-select"
          style={{ fontSize: 13, color: "var(--text-primary, #f9fafb)" }}
        >
          Text direction
        </label>
        <select
          id="rtl-direction-select"
          value={direction}
          disabled={!enabled}
          onChange={(e) => onChange(enabled, e.target.value as Direction)}
          style={{
            fontSize: 13,
            padding: "3px 8px",
            borderRadius: 3,
            border: "1px solid var(--border-default, #374151)",
            background: "var(--bg-secondary, #1e1e2e)",
            color: "var(--text-primary, #f9fafb)",
            cursor: enabled ? "pointer" : "not-allowed",
            opacity: enabled ? 1 : 0.5,
          }}
        >
          <option value="auto">auto</option>
          <option value="ltr">ltr</option>
          <option value="rtl">rtl</option>
        </select>
      </div>
    </div>
  );
}
