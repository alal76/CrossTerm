import React from 'react';

export interface SessionHealthCardProps {
  sessionId: string;
  sessionName: string;
  status: 'ok' | 'degraded' | 'dropped' | 'unknown';
  latencyMs?: number;
  uptimeSeconds?: number;
  reconnectCount?: number;
}

const STATUS_COLOR: Record<SessionHealthCardProps['status'], string> = {
  ok: '#22c55e',
  degraded: '#eab308',
  dropped: '#ef4444',
  unknown: '#6b7280',
};

function formatUptime(seconds?: number): string {
  if (seconds == null) return '--';
  if (seconds < 60) return `${seconds}s`;
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

export const SessionHealthCard: React.FC<SessionHealthCardProps> = ({
  sessionName,
  status,
  latencyMs,
  uptimeSeconds,
  reconnectCount = 0,
}) => {
  return (
    <div
      className="session-health-card"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 6,
        padding: '4px 8px',
        borderRadius: 4,
        fontSize: 12,
        background: 'var(--bg-secondary, #1e1e2e)',
        color: 'var(--text-primary, #cdd6f4)',
      }}
    >
      <span
        className="status-dot"
        style={{
          width: 8,
          height: 8,
          borderRadius: '50%',
          background: STATUS_COLOR[status],
          flexShrink: 0,
        }}
        aria-label={`status: ${status}`}
      />
      <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
        {sessionName.length > 20 ? `${sessionName.slice(0, 20)}…` : sessionName}
      </span>
      <span style={{ opacity: 0.7, whiteSpace: 'nowrap' }}>
        {latencyMs != null ? `${latencyMs}ms` : '--'}
      </span>
      <span style={{ opacity: 0.7, whiteSpace: 'nowrap' }}>
        {formatUptime(uptimeSeconds)}
      </span>
      {reconnectCount > 0 && (
        <span
          className="reconnect-badge"
          style={{
            background: '#f97316',
            color: '#fff',
            borderRadius: 10,
            padding: '0 5px',
            fontSize: 10,
            fontWeight: 700,
          }}
        >
          {reconnectCount}↺
        </span>
      )}
    </div>
  );
};

export default SessionHealthCard;
