import { describe, it, expect } from "vitest";
import { buildRdpConfig, buildVncConfig, buildWsTermConfig, buildRedfishConfig, buildWebDavConfig, buildMqttConfig, buildSmbConfig, buildNetconfConfig, buildMoshConfig, buildWinRmConfig, buildIpmiConfig, buildSnmpConfig, buildGrpcConfig, buildTn3270Config, buildTn5250Config, buildRloginConfig, buildDockerLogsConfig, buildX11ForwardConfig, buildProxmoxConsoleConfig } from "@/utils/sessionConfig";
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

describe("buildWinRmConfig", () => {
  it("maps host/port/credentials and infers use_tls from the conventional TLS port 5986", () => {
    const config = buildWinRmConfig(
      baseSession({ connection: { host: "10.0.0.5", port: 5986, protocolOptions: { username: "Administrator", password: "hunter2" } } })
    );
    expect(config.host).toBe("10.0.0.5");
    expect(config.port).toBe(5986);
    expect(config.username).toBe("Administrator");
    expect(config.password).toBe("hunter2");
    expect(config.use_tls).toBe(true);
  });

  it("defaults auth to ntlm and use_tls to false on the plaintext port", () => {
    const config = buildWinRmConfig(baseSession({ connection: { host: "10.0.0.5", port: 5985 } }));
    expect(config.auth).toBe("ntlm");
    expect(config.use_tls).toBe(false);
    expect(config.username).toBe("");
    expect(config.password).toBe("");
  });

  it("honors an explicit auth override from protocolOptions", () => {
    const config = buildWinRmConfig(baseSession({ connection: { host: "10.0.0.5", port: 5985, protocolOptions: { auth: "basic" } } }));
    expect(config.auth).toBe("basic");
  });
});

describe("buildIpmiConfig", () => {
  it("maps host/port/credentials and defaults channel to 1, privilege to administrator", () => {
    const config = buildIpmiConfig(
      baseSession({ connection: { host: "10.0.0.10", port: 623, protocolOptions: { username: "admin", password: "hunter2" } } })
    );
    expect(config.host).toBe("10.0.0.10");
    expect(config.port).toBe(623);
    expect(config.username).toBe("admin");
    expect(config.password).toBe("hunter2");
    expect(config.channel).toBe(1);
    expect(config.privilege).toBe("administrator");
  });

  it("honors an explicit channel/privilege override from protocolOptions", () => {
    const config = buildIpmiConfig(baseSession({ connection: { host: "10.0.0.10", port: 623, protocolOptions: { channel: 2, privilege: "operator" } } }));
    expect(config.channel).toBe(2);
    expect(config.privilege).toBe("operator");
  });
});

describe("buildSnmpConfig", () => {
  it("defaults to v2c and a 2s timeout", () => {
    const config = buildSnmpConfig(baseSession({ connection: { host: "10.0.0.30", port: 161 } }));
    expect(config.host).toBe("10.0.0.30");
    expect(config.port).toBe(161);
    expect(config.version).toBe("v2c");
    expect(config.timeout_ms).toBe(2000);
  });

  it("maps v3 credentials from protocolOptions", () => {
    const config = buildSnmpConfig(
      baseSession({
        connection: {
          host: "10.0.0.30",
          port: 161,
          protocolOptions: {
            version: "v3",
            username: "monitor",
            auth_passphrase: "authpass123",
            auth_protocol: "sha1",
            priv_passphrase: "privpass123",
            priv_protocol: "aes128",
          },
        },
      })
    );
    expect(config.version).toBe("v3");
    expect(config.username).toBe("monitor");
    expect(config.auth_passphrase).toBe("authpass123");
    expect(config.auth_protocol).toBe("sha1");
    expect(config.priv_passphrase).toBe("privpass123");
    expect(config.priv_protocol).toBe("aes128");
  });

  it("maps the community string for v1/v2c", () => {
    const config = buildSnmpConfig(baseSession({ connection: { host: "10.0.0.30", port: 161, protocolOptions: { community: "private" } } }));
    expect(config.community).toBe("private");
  });
});

describe("buildGrpcConfig", () => {
  it("builds an http:// endpoint from host/port by default", () => {
    const config = buildGrpcConfig(baseSession({ connection: { host: "10.0.0.40", port: 50051 } }));
    expect(config.endpoint).toBe("http://10.0.0.40:50051");
    expect(config.verify_tls).toBe(false);
    expect(config.metadata).toEqual({});
  });

  it("uses https:// when protocolOptions.secure is set", () => {
    const config = buildGrpcConfig(baseSession({ connection: { host: "10.0.0.40", port: 443, protocolOptions: { secure: true } } }));
    expect(config.endpoint).toBe("https://10.0.0.40:443");
  });

  it("prefers an explicit protocolOptions.endpoint over the derived one", () => {
    const config = buildGrpcConfig(baseSession({ connection: { host: "h", port: 1, protocolOptions: { endpoint: "https://api.example.com" } } }));
    expect(config.endpoint).toBe("https://api.example.com");
  });

  it("passes through custom metadata headers", () => {
    const config = buildGrpcConfig(baseSession({ connection: { host: "10.0.0.40", port: 50051, protocolOptions: { metadata: { authorization: "Bearer abc" } } } }));
    expect(config.metadata).toEqual({ authorization: "Bearer abc" });
  });
});

describe("buildTn3270Config", () => {
  it("defaults to model2 and maps host/port/lu_name", () => {
    const config = buildTn3270Config(baseSession({ connection: { host: "10.0.0.50", port: 23, protocolOptions: { lu_name: "LU1" } } }));
    expect(config.host).toBe("10.0.0.50");
    expect(config.port).toBe(23);
    expect(config.model).toBe("model2");
    expect(config.lu_name).toBe("LU1");
  });

  it("honors an explicit model override", () => {
    const config = buildTn3270Config(baseSession({ connection: { host: "10.0.0.50", port: 23, protocolOptions: { model: "model5" } } }));
    expect(config.model).toBe("model5");
  });
});

describe("buildTn5250Config", () => {
  it("maps host/port/device_name/system_name/ssl", () => {
    const config = buildTn5250Config(
      baseSession({ connection: { host: "10.0.0.60", port: 23, protocolOptions: { device_name: "QPADEV0001", system_name: "SYS1", ssl: true } } })
    );
    expect(config.host).toBe("10.0.0.60");
    expect(config.device_name).toBe("QPADEV0001");
    expect(config.system_name).toBe("SYS1");
    expect(config.ssl).toBe(true);
  });

  it("defaults ssl to false without protocolOptions", () => {
    const config = buildTn5250Config(baseSession({ connection: { host: "10.0.0.60", port: 23 } }));
    expect(config.ssl).toBe(false);
  });
});

describe("buildRloginConfig", () => {
  it("maps host/port and defaults terminal_type/speed", () => {
    const config = buildRloginConfig(baseSession({ connection: { host: "10.0.0.70", port: 513, protocolOptions: { username: "alal" } } }));
    expect(config.host).toBe("10.0.0.70");
    expect(config.port).toBe(513);
    expect(config.local_username).toBe("alal");
    expect(config.remote_username).toBe("alal");
    expect(config.terminal_type).toBe("xterm");
    expect(config.terminal_speed).toBe(38400);
  });

  it("prefers explicit local_username/remote_username over the shared username fallback", () => {
    const config = buildRloginConfig(
      baseSession({ connection: { host: "10.0.0.70", port: 513, protocolOptions: { username: "shared", local_username: "alal", remote_username: "root" } } })
    );
    expect(config.local_username).toBe("alal");
    expect(config.remote_username).toBe("root");
  });
});

describe("buildDockerLogsConfig", () => {
  it("uses TCP host/port when no socket_path is configured", () => {
    const config = buildDockerLogsConfig(baseSession({ connection: { host: "10.0.0.80", port: 2375, protocolOptions: { container_id: "abc123" } } }));
    expect(config.host).toBe("10.0.0.80");
    expect(config.port).toBe(2375);
    expect(config.socket_path).toBeUndefined();
    expect(config.container_id).toBe("abc123");
  });

  it("prefers socket_path over host/port when both could apply", () => {
    const config = buildDockerLogsConfig(
      baseSession({ connection: { host: "10.0.0.80", port: 2375, protocolOptions: { socket_path: "/var/run/docker.sock", container_id: "abc123" } } })
    );
    expect(config.socket_path).toBe("/var/run/docker.sock");
    expect(config.host).toBeUndefined();
    expect(config.port).toBeUndefined();
  });
});

describe("buildX11ForwardConfig", () => {
  it("uses password auth when a password is present, and defaults remote_command/local_display", () => {
    const config = buildX11ForwardConfig(baseSession({ connection: { host: "10.0.0.90", port: 22, protocolOptions: { username: "alal", password: "hunter2" } } }));
    expect(config.host).toBe("10.0.0.90");
    expect(config.username).toBe("alal");
    expect(config.auth).toEqual({ type: "password", password: "hunter2" });
    expect(config.remote_command).toBe("xterm");
    expect(config.local_display).toBe("0");
  });

  it("falls back to private_key auth when no password is set", () => {
    const config = buildX11ForwardConfig(
      baseSession({ connection: { host: "10.0.0.90", port: 22, protocolOptions: { username: "alal", key_data: "PEM", passphrase: "pass", remote_command: "xclock", local_display: "1" } } })
    );
    expect(config.auth).toEqual({ type: "private_key", key_data: "PEM", passphrase: "pass" });
    expect(config.remote_command).toBe("xclock");
    expect(config.local_display).toBe("1");
  });
});

describe("buildProxmoxConsoleConfig", () => {
  it("maps host/port/node/vmid and defaults realm to pam, resource_type to qemu", () => {
    const config = buildProxmoxConsoleConfig(
      baseSession({ connection: { host: "10.0.0.5", port: 8006, protocolOptions: { node: "pve1", vmid: "101", username: "root", password: "hunter2" } } })
    );
    expect(config.host).toBe("10.0.0.5");
    expect(config.port).toBe(8006);
    expect(config.node).toBe("pve1");
    expect(config.vmid).toBe("101");
    expect(config.resource_type).toBe("qemu");
    expect(config.realm).toBe("pam");
    expect(config.username).toBe("root");
    expect(config.password).toBe("hunter2");
    expect(config.verify_tls).toBe(false);
  });

  it("falls back to port 8006 when the session has no port, and honors explicit lxc/realm/verify_tls", () => {
    const config = buildProxmoxConsoleConfig(
      baseSession({
        connection: {
          host: "10.0.0.5",
          port: 0,
          protocolOptions: { node: "pve1", vmid: "200", resource_type: "lxc", realm: "pve", verify_tls: true },
        },
      })
    );
    expect(config.port).toBe(8006);
    expect(config.resource_type).toBe("lxc");
    expect(config.realm).toBe("pve");
    expect(config.verify_tls).toBe(true);
  });
});
