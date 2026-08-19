import type { Session } from "@/types";
import { SessionType } from "@/types";

/**
 * Builds an ad-hoc SSH session for a cloud instance's "Connect" action, the
 * same pattern NetworkExplorer's handleConnect uses for discovered hosts —
 * this app is fundamentally an SSH-capable terminal, so "connect" to a
 * running instance means opening an SSH tab to it, not a cloud-CLI-specific
 * mechanism.
 */
export function buildAdHocSshSession(host: string, name: string): Session {
  const now = new Date().toISOString();
  return {
    id: crypto.randomUUID(),
    name,
    type: SessionType.SSH,
    group: "Cloud",
    tags: [],
    connection: { host, port: 22 },
    createdAt: now,
    updatedAt: now,
    autoReconnect: false,
    keepAliveIntervalSeconds: 0,
  };
}
