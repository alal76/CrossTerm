// ── Session → protocol-config builders ──
// RDP/VNC viewers take a fully-shaped config object rather than raw
// session fields (unlike SSH, which reads host/port/username/auth
// straight off the session). These builders centralize that mapping so
// App.tsx and SplitPaneContainer.tsx (which both route tabs to their
// session-type component) don't duplicate it. protocolOptions currently
// only ever carries "username" (SessionEditor doesn't collect RDP/VNC-
// specific fields yet — domain, NLA, codec, etc. — so those fall back to
// sensible defaults here until the editor grows per-type fields).

import type { Session, RdpConfig, VncConfig, WsTermConfig, RedfishConfig, WebDavConfig, MqttConfig, SmbConfig, NetconfConfig, MoshConfig, WinRmConfig, WinRmAuth, IpmiConfig, IpmiPrivilege, SnmpConfig, SnmpVersion, SnmpV3AuthProtocol, SnmpV3PrivProtocol, GrpcConfig, Tn3270Config, Tn3270Model, Tn5250Config, RloginConfig, DockerLogsConfig, X11ForwardConfig, X11ForwardAuth, ProxmoxConsoleConfig, ProxmoxResourceType, NfsConfig } from '@/types';

export function buildRdpConfig(session: Session): RdpConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    username: (opts?.['username'] as string) ?? '',
    password: (opts?.['password'] as string) ?? '',
    credential_ref: session.credentialRef,
    domain: opts?.['domain'] as string | undefined,
    nla_enabled: true,
    tls_required: false,
    codec: 'auto',
    clipboard_sync: true,
    drive_paths: [],
    printer_redirect: false,
    audio_mode: 'none',
    smart_card: false,
  };
}

export function buildVncConfig(session: Session): VncConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    password: (opts?.['password'] as string) ?? undefined,
    vnc_auth: true,
    vencrypt: false,
  };
}

export function buildWsTermConfig(session: Session): WsTermConfig {
  const opts = session.connection.protocolOptions;
  const explicitUrl = opts?.['url'] as string | undefined;
  const secure = Boolean(opts?.['secure']);
  return {
    url: explicitUrl ?? `${secure ? 'wss' : 'ws'}://${session.connection.host}:${session.connection.port}`,
    token: opts?.['token'] as string | undefined,
    verify_tls: false,
  };
}

export function buildRedfishConfig(session: Session): RedfishConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    username: (opts?.['username'] as string) ?? '',
    password: (opts?.['password'] as string) ?? '',
    use_tls: true,
    verify_tls: false,
  };
}

export function buildWebDavConfig(session: Session): WebDavConfig {
  const opts = session.connection.protocolOptions;
  const explicitUrl = opts?.['url'] as string | undefined;
  const secure = Boolean(opts?.['secure']);
  return {
    url: explicitUrl ?? `${secure ? 'https' : 'http'}://${session.connection.host}:${session.connection.port}`,
    username: opts?.['username'] as string | undefined,
    password: opts?.['password'] as string | undefined,
    verify_tls: false,
  };
}

export function buildMqttConfig(session: Session): MqttConfig {
  const opts = session.connection.protocolOptions;
  const explicitClientId = opts?.['client_id'] as string | undefined;
  return {
    host: session.connection.host,
    port: session.connection.port,
    client_id: explicitClientId || `crossterm-${crypto.randomUUID().slice(0, 8)}`,
    username: opts?.['username'] as string | undefined,
    password: opts?.['password'] as string | undefined,
    keep_alive_secs: 30,
    use_tls: session.connection.port === 8883,
    clean_session: true,
  };
}

/// SMB can't actually connect without a share name, which isn't a field
/// the generic Session model has — this builder leaves `share` empty when
/// protocolOptions doesn't have one, and SmbBrowser itself detects that and
/// shows a share picker (via smb_list_shares) before ever calling
/// smb_connect, rather than failing to connect with a misleading error.
export function buildSmbConfig(session: Session): SmbConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    username: opts?.['username'] as string | undefined,
    password: opts?.['password'] as string | undefined,
    domain: opts?.['domain'] as string | undefined,
    share: (opts?.['share'] as string) ?? '',
  };
}

export function buildNetconfConfig(session: Session): NetconfConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    username: (opts?.['username'] as string) ?? '',
    password: opts?.['password'] as string | undefined,
    private_key: opts?.['private_key'] as string | undefined,
    private_key_passphrase: opts?.['private_key_passphrase'] as string | undefined,
    capabilities: [],
  };
}

export function buildMoshConfig(session: Session): MoshConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    username: (opts?.['username'] as string) ?? '',
    identity_file: opts?.['identity_file'] as string | undefined,
    udp_port_range: opts?.['udp_port_range'] as string | undefined,
    ssh_options: opts?.['ssh_options'] as string | undefined,
  };
}

export function buildWinRmConfig(session: Session): WinRmConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    username: (opts?.['username'] as string) ?? '',
    password: (opts?.['password'] as string) ?? '',
    use_tls: session.connection.port === 5986,
    auth: (opts?.['auth'] as WinRmAuth) ?? 'ntlm',
    verify_tls: false,
  };
}

export function buildIpmiConfig(session: Session): IpmiConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    username: (opts?.['username'] as string) ?? '',
    password: (opts?.['password'] as string) ?? '',
    channel: (opts?.['channel'] as number) ?? 1,
    privilege: (opts?.['privilege'] as IpmiPrivilege) ?? 'administrator',
  };
}

export function buildSnmpConfig(session: Session): SnmpConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    version: (opts?.['version'] as SnmpVersion) ?? 'v2c',
    community: opts?.['community'] as string | undefined,
    username: opts?.['username'] as string | undefined,
    auth_passphrase: opts?.['auth_passphrase'] as string | undefined,
    auth_protocol: opts?.['auth_protocol'] as SnmpV3AuthProtocol | undefined,
    priv_passphrase: opts?.['priv_passphrase'] as string | undefined,
    priv_protocol: opts?.['priv_protocol'] as SnmpV3PrivProtocol | undefined,
    timeout_ms: 2000,
  };
}

export function buildGrpcConfig(session: Session): GrpcConfig {
  const opts = session.connection.protocolOptions;
  const explicitEndpoint = opts?.['endpoint'] as string | undefined;
  const secure = Boolean(opts?.['secure']);
  return {
    endpoint: explicitEndpoint ?? `${secure ? 'https' : 'http'}://${session.connection.host}:${session.connection.port}`,
    verify_tls: false,
    metadata: (opts?.['metadata'] as Record<string, string>) ?? {},
  };
}

export function buildTn3270Config(session: Session): Tn3270Config {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    model: (opts?.['model'] as Tn3270Model) ?? 'model2',
    lu_name: opts?.['lu_name'] as string | undefined,
  };
}

export function buildTn5250Config(session: Session): Tn5250Config {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    device_name: opts?.['device_name'] as string | undefined,
    system_name: opts?.['system_name'] as string | undefined,
    ssl: Boolean(opts?.['ssl']),
  };
}

export function buildRloginConfig(session: Session): RloginConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port,
    local_username: (opts?.['local_username'] as string) ?? (opts?.['username'] as string) ?? '',
    remote_username: (opts?.['remote_username'] as string) ?? (opts?.['username'] as string) ?? '',
    terminal_type: (opts?.['terminal_type'] as string) ?? 'xterm',
    terminal_speed: (opts?.['terminal_speed'] as number) ?? 38400,
  };
}

export function buildDockerLogsConfig(session: Session): DockerLogsConfig {
  const opts = session.connection.protocolOptions;
  const socketPath = opts?.['socket_path'] as string | undefined;
  return {
    socket_path: socketPath,
    host: socketPath ? undefined : session.connection.host,
    port: socketPath ? undefined : session.connection.port,
    container_id: (opts?.['container_id'] as string) ?? '',
    tty: Boolean(opts?.['tty']),
    tail: opts?.['tail'] as number | undefined,
    timestamps: Boolean(opts?.['timestamps']),
  };
}

export function buildX11ForwardConfig(session: Session): X11ForwardConfig {
  const opts = session.connection.protocolOptions;
  const password = opts?.['password'] as string | undefined;
  const auth: X11ForwardAuth = password
    ? { type: 'password', password }
    : { type: 'private_key', key_data: (opts?.['key_data'] as string) ?? '', passphrase: opts?.['passphrase'] as string | undefined };
  return {
    host: session.connection.host,
    port: session.connection.port,
    username: (opts?.['username'] as string) ?? '',
    auth,
    remote_command: (opts?.['remote_command'] as string) ?? 'xterm',
    local_display: (opts?.['local_display'] as string) ?? '0',
  };
}

export function buildProxmoxConsoleConfig(session: Session): ProxmoxConsoleConfig {
  const opts = session.connection.protocolOptions;
  return {
    host: session.connection.host,
    port: session.connection.port || 8006,
    node: (opts?.['node'] as string) ?? '',
    vmid: (opts?.['vmid'] as string) ?? '',
    resource_type: ((opts?.['resource_type'] as ProxmoxResourceType) ?? 'qemu'),
    username: (opts?.['username'] as string) ?? '',
    realm: (opts?.['realm'] as string) ?? 'pam',
    password: (opts?.['password'] as string) ?? '',
    verify_tls: Boolean(opts?.['verify_tls']),
  };
}

export function buildNfsConfig(session: Session): NfsConfig {
  const opts = session.connection.protocolOptions;
  const uid = opts?.['uid'] as number | undefined;
  const gid = opts?.['gid'] as number | undefined;
  return {
    host: session.connection.host,
    export_path: (opts?.['export_path'] as string) ?? '/',
    uid,
    gid,
    mount_port: opts?.['mount_port'] as number | undefined,
    nfs_port: opts?.['nfs_port'] as number | undefined,
  };
}
