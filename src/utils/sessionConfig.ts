// ── Session → protocol-config builders ──
// RDP/VNC viewers take a fully-shaped config object rather than raw
// session fields (unlike SSH, which reads host/port/username/auth
// straight off the session). These builders centralize that mapping so
// App.tsx and SplitPaneContainer.tsx (which both route tabs to their
// session-type component) don't duplicate it. protocolOptions currently
// only ever carries "username" (SessionEditor doesn't collect RDP/VNC-
// specific fields yet — domain, NLA, codec, etc. — so those fall back to
// sensible defaults here until the editor grows per-type fields).

import type { Session, RdpConfig, VncConfig, WsTermConfig, RedfishConfig, WebDavConfig, MqttConfig } from '@/types';

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
