export interface HostKeyChange {
  host: string;
  port: number;
  oldFingerprint: string;
  newFingerprint: string;
  oldKeyType: string;
  newKeyType: string;
  detectedAt: string;
}

interface KnownHostsDiffProps {
  change: HostKeyChange;
  onAccept: (change: HostKeyChange) => void;
  onReject: () => void;
  onForget: (host: string) => void;
}

export default function KnownHostsDiff({
  change,
  onAccept,
  onReject,
  onForget,
}: KnownHostsDiffProps) {
  return (
    <div
      style={{
        border: "1px solid #ef4444",
        borderRadius: 6,
        overflow: "hidden",
        fontFamily: "var(--font-mono, monospace)",
        fontSize: 13,
      }}
    >
      {/* Warning banner */}
      <div
        style={{
          background: "#ef4444",
          color: "#fff",
          padding: "8px 12px",
          fontWeight: 600,
        }}
      >
        &#9888; Host key mismatch detected for {change.host}:{change.port}
      </div>

      {/* Diff table */}
      <table
        style={{
          width: "100%",
          borderCollapse: "collapse",
          tableLayout: "fixed",
        }}
      >
        <thead>
          <tr>
            <th
              style={{
                padding: "6px 12px",
                textAlign: "left",
                background: "#fef2f2",
                color: "#b91c1c",
                borderBottom: "1px solid #fca5a5",
                width: "50%",
              }}
            >
              Previous key
            </th>
            <th
              style={{
                padding: "6px 12px",
                textAlign: "left",
                background: "#f0fdf4",
                color: "#15803d",
                borderBottom: "1px solid #86efac",
                width: "50%",
              }}
            >
              New key
            </th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td
              style={{
                padding: "8px 12px",
                background: "#fef2f2",
                color: "#b91c1c",
                verticalAlign: "top",
                borderRight: "1px solid #fca5a5",
              }}
            >
              <div style={{ fontWeight: 600, marginBottom: 4 }}>
                {change.oldKeyType}
              </div>
              <div style={{ wordBreak: "break-all" }}>
                {change.oldFingerprint}
              </div>
            </td>
            <td
              style={{
                padding: "8px 12px",
                background: "#f0fdf4",
                color: "#15803d",
                verticalAlign: "top",
              }}
            >
              <div style={{ fontWeight: 600, marginBottom: 4 }}>
                {change.newKeyType}
              </div>
              <div style={{ wordBreak: "break-all" }}>
                {change.newFingerprint}
              </div>
            </td>
          </tr>
        </tbody>
      </table>

      {/* Risk warning */}
      <div
        style={{
          padding: "8px 12px",
          background: "#fffbeb",
          color: "#92400e",
          fontSize: 12,
          borderTop: "1px solid #fde68a",
        }}
      >
        This could indicate a man-in-the-middle attack. Only accept if you
        manually changed the server&apos;s host key.
      </div>

      {/* Action buttons */}
      <div
        style={{
          display: "flex",
          gap: 8,
          padding: "10px 12px",
          background: "var(--bg-secondary, #f9fafb)",
          borderTop: "1px solid #e5e7eb",
        }}
      >
        <button
          onClick={() => onAccept(change)}
          style={{
            padding: "6px 14px",
            background: "#16a34a",
            color: "#fff",
            border: "none",
            borderRadius: 4,
            cursor: "pointer",
            fontSize: 13,
          }}
        >
          Accept new key
        </button>
        <button
          onClick={onReject}
          style={{
            padding: "6px 14px",
            background: "#dc2626",
            color: "#fff",
            border: "none",
            borderRadius: 4,
            cursor: "pointer",
            fontSize: 13,
          }}
        >
          Reject connection
        </button>
        <button
          onClick={() => onForget(change.host)}
          style={{
            padding: "6px 14px",
            background: "transparent",
            color: "var(--text-secondary, #6b7280)",
            border: "1px solid #d1d5db",
            borderRadius: 4,
            cursor: "pointer",
            fontSize: 13,
          }}
        >
          Forget old key
        </button>
      </div>
    </div>
  );
}
