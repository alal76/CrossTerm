import { describe, it, expect } from "vitest";
import { buildRdpConfig, buildVncConfig, buildWsTermConfig, buildRedfishConfig, buildWebDavConfig, buildMqttConfig, buildSmbConfig, buildNetconfConfig, buildMoshConfig } from "@/utils/sessionConfig";
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

describe("buildWsTermConfig", () => {
  it("builds a ws:// URL from host/port by default", () => {
    const config = buildWsTermConfig(baseSession({ connection: { host: "192.168.0.5", port: 7681 } }));
    expect(config.url).toBe("ws://192.168.0.5:7681");
    expect(config.verify_tls).toBe(false);
  });

  it("uses wss:// when protocolOptions.secure is set", () => {
    const config = buildWsTermConfig(baseSession({ connection: { host: "192.168.0.5", port: 7681, protocolOptions: { secure: true } } }));
    expect(config.url).toBe("wss://192.168.0.5:7681");
  });

  it("prefers an explicit protocolOptions.url over the derived one", () => {
    const config = buildWsTermConfig(baseSession({ connection: { host: "192.168.0.5", port: 7681, protocolOptions: { url: "wss://example.com/term" } } }));
    expect(config.url).toBe("wss://example.com/term");
  });

  it("pulls the bearer token from protocolOptions", () => {
    const config = buildWsTermConfig(baseSession({ connection: { host: "h", port: 1, protocolOptions: { token: "abc123" } } }));
    expect(config.token).toBe("abc123");
  });
});

describe("buildRedfishConfig", () => {
  it("maps host/port and defaults use_tls true, verify_tls false", () => {
    const config = buildRedfishConfig(baseSession({ connection: { host: "192.168.0.99", port: 443 } }));
    expect(config.host).toBe("192.168.0.99");
    expect(config.port).toBe(443);
    expect(config.use_tls).toBe(true);
    expect(config.verify_tls).toBe(false);
  });

  it("defaults username/password to empty strings without protocolOptions", () => {
    const config = buildRedfishConfig(baseSession());
    expect(config.username).toBe("");
    expect(config.password).toBe("");
  });
});

describe("buildWebDavConfig", () => {
  it("builds an http:// URL from host/port by default", () => {
    const config = buildWebDavConfig(baseSession({ connection: { host: "192.168.0.8", port: 80 } }));
    expect(config.url).toBe("http://192.168.0.8:80");
  });

  it("uses https:// when protocolOptions.secure is set", () => {
    const config = buildWebDavConfig(baseSession({ connection: { host: "192.168.0.8", port: 443, protocolOptions: { secure: true } } }));
    expect(config.url).toBe("https://192.168.0.8:443");
  });

  it("prefers an explicit protocolOptions.url over the derived one", () => {
    const config = buildWebDavConfig(baseSession({ connection: { host: "h", port: 1, protocolOptions: { url: "https://dav.example.com/remote.php/dav" } } }));
    expect(config.url).toBe("https://dav.example.com/remote.php/dav");
  });
});

describe("buildMqttConfig", () => {
  it("maps host/port and defaults keep_alive/clean_session", () => {
    const config = buildMqttConfig(baseSession({ connection: { host: "192.168.0.8", port: 1883 } }));
    expect(config.host).toBe("192.168.0.8");
    expect(config.port).toBe(1883);
    expect(config.keep_alive_secs).toBe(30);
    expect(config.clean_session).toBe(true);
  });

  it("infers use_tls from the conventional TLS port 8883", () => {
    expect(buildMqttConfig(baseSession({ connection: { host: "h", port: 8883 } })).use_tls).toBe(true);
    expect(buildMqttConfig(baseSession({ connection: { host: "h", port: 1883 } })).use_tls).toBe(false);
  });

  it("generates a unique client_id when none is provided, else uses protocolOptions.client_id", () => {
    const generated1 = buildMqttConfig(baseSession({ connection: { host: "h", port: 1883 } }));
    const generated2 = buildMqttConfig(baseSession({ connection: { host: "h", port: 1883 } }));
    expect(generated1.client_id).toMatch(/^crossterm-/);
    expect(generated1.client_id).not.toBe(generated2.client_id);

    const explicit = buildMqttConfig(baseSession({ connection: { host: "h", port: 1883, protocolOptions: { client_id: "my-client" } } }));
    expect(explicit.client_id).toBe("my-client");
  });
});

describe("buildSmbConfig", () => {
  it("maps host/port and credentials from protocolOptions", () => {
    const config = buildSmbConfig(
      baseSession({
        connection: { host: "192.168.0.30", port: 445, protocolOptions: { username: "alal", password: "hunter2", domain: "WORKGROUP", share: "public" } },
      })
    );
    expect(config.host).toBe("192.168.0.30");
    expect(config.port).toBe(445);
    expect(config.username).toBe("alal");
    expect(config.password).toBe("hunter2");
    expect(config.domain).toBe("WORKGROUP");
    expect(config.share).toBe("public");
  });

  it("defaults share to an empty string when protocolOptions doesn't have one", () => {
    const config = buildSmbConfig(baseSession({ connection: { host: "192.168.0.30", port: 445 } }));
    expect(config.share).toBe("");
    expect(config.username).toBeUndefined();
    expect(config.domain).toBeUndefined();
  });
});

describe("buildNetconfConfig", () => {
  it("maps host/port/username and credentials from protocolOptions", () => {
    const config = buildNetconfConfig(
      baseSession({
        connection: {
          host: "192.168.0.40",
          port: 830,
          protocolOptions: { username: "admin", password: "hunter2", private_key: "PEM", private_key_passphrase: "pass" },
        },
      })
    );
    expect(config.host).toBe("192.168.0.40");
    expect(config.port).toBe(830);
    expect(config.username).toBe("admin");
    expect(config.password).toBe("hunter2");
    expect(config.private_key).toBe("PEM");
    expect(config.private_key_passphrase).toBe("pass");
  });

  it("defaults username to an empty string and leaves credentials undefined without protocolOptions", () => {
    const config = buildNetconfConfig(baseSession({ connection: { host: "192.168.0.40", port: 830 } }));
    expect(config.username).toBe("");
    expect(config.password).toBeUndefined();
    expect(config.private_key).toBeUndefined();
    expect(config.capabilities).toEqual([]);
  });
});

describe("buildMoshConfig", () => {
  it("maps host/port/username and mosh-specific options from protocolOptions", () => {
    const config = buildMoshConfig(
      baseSession({
        connection: {
          host: "192.168.0.20",
          port: 22,
          protocolOptions: { username: "alal", identity_file: "/home/alal/.ssh/id_ed25519", udp_port_range: "60001:60010", ssh_options: "-o StrictHostKeyChecking=no" },
        },
      })
    );
    expect(config.host).toBe("192.168.0.20");
    expect(config.port).toBe(22);
    expect(config.username).toBe("alal");
    expect(config.identity_file).toBe("/home/alal/.ssh/id_ed25519");
    expect(config.udp_port_range).toBe("60001:60010");
    expect(config.ssh_options).toBe("-o StrictHostKeyChecking=no");
  });

  it("defaults username to an empty string and leaves optional fields undefined without protocolOptions", () => {
    const config = buildMoshConfig(baseSession({ connection: { host: "192.168.0.20", port: 22 } }));
    expect(config.username).toBe("");
    expect(config.identity_file).toBeUndefined();
    expect(config.udp_port_range).toBeUndefined();
  });
});
