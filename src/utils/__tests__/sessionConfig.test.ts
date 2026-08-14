import { describe, it, expect } from "vitest";
import { buildRdpConfig, buildVncConfig } from "@/utils/sessionConfig";
import type { Session } from "@/types";
import { SessionType } from "@/types";

function baseSession(overrides: Partial<Session> = {}): Session {
  return {
    id: "sess-1",
    name: "Test Session",
    type: SessionType.RDP,
    tags: [],
    connection: { host: "192.168.0.11", port: 3389 },
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    autoReconnect: false,
    keepAliveIntervalSeconds: 30,
    ...overrides,
  };
}

describe("buildRdpConfig", () => {
  it("maps host/port and credentialRef straight from the session", () => {
    const session = baseSession({ credentialRef: "cred-42" });
    const config = buildRdpConfig(session);
    expect(config.host).toBe("192.168.0.11");
    expect(config.port).toBe(3389);
    expect(config.credential_ref).toBe("cred-42");
  });

  it("pulls username/password/domain from protocolOptions when present", () => {
    const session = baseSession({
      connection: { host: "10.0.0.5", port: 3389, protocolOptions: { username: "alal", password: "hunter2", domain: "CORP" } },
    });
    const config = buildRdpConfig(session);
    expect(config.username).toBe("alal");
    expect(config.password).toBe("hunter2");
    expect(config.domain).toBe("CORP");
  });

  it("defaults username/password to empty strings and domain to undefined when protocolOptions is absent", () => {
    const config = buildRdpConfig(baseSession());
    expect(config.username).toBe("");
    expect(config.password).toBe("");
    expect(config.domain).toBeUndefined();
  });

  it("fills in sensible defaults for fields the generic Session model doesn't carry", () => {
    const config = buildRdpConfig(baseSession());
    expect(config.nla_enabled).toBe(true);
    expect(config.tls_required).toBe(false);
    expect(config.codec).toBe("auto");
    expect(config.clipboard_sync).toBe(true);
    expect(config.drive_paths).toEqual([]);
    expect(config.printer_redirect).toBe(false);
    expect(config.audio_mode).toBe("none");
    expect(config.smart_card).toBe(false);
  });
});

describe("buildVncConfig", () => {
  it("maps host/port from the session", () => {
    const session = baseSession({ type: SessionType.VNC, connection: { host: "192.168.0.11", port: 5900 } });
    const config = buildVncConfig(session);
    expect(config.host).toBe("192.168.0.11");
    expect(config.port).toBe(5900);
  });

  it("pulls password from protocolOptions when present, else leaves it undefined", () => {
    const withPassword = buildVncConfig(
      baseSession({ connection: { host: "192.168.0.11", port: 5900, protocolOptions: { password: "secret" } } })
    );
    expect(withPassword.password).toBe("secret");

    const withoutPassword = buildVncConfig(baseSession({ connection: { host: "192.168.0.11", port: 5900 } }));
    expect(withoutPassword.password).toBeUndefined();
  });

  it("defaults vnc_auth to true and vencrypt to false", () => {
    const config = buildVncConfig(baseSession());
    expect(config.vnc_auth).toBe(true);
    expect(config.vencrypt).toBe(false);
  });
});
