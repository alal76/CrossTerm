// ── Session → protocol-config builders ──
// RDP/VNC viewers take a fully-shaped config object rather than raw
// session fields (unlike SSH, which reads host/port/username/auth
// straight off the session). These builders centralize that mapping so
// App.tsx and SplitPaneContainer.tsx (which both route tabs to their
// session-type component) don't duplicate it. protocolOptions currently
// only ever carries "username" (SessionEditor doesn't collect RDP/VNC-
// specific fields yet — domain, NLA, codec, etc. — so those fall back to
// sensible defaults here until the editor grows per-type fields).

import type { Session, RdpConfig, VncConfig } from '@/types';

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
