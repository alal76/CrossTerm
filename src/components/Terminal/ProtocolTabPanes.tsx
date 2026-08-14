// ── Memoized session → protocol-viewer wrappers ──
// RdpViewer/VncViewer (and now WebSocketTerminalTab/RedfishExplorer/
// WebDavBrowser/MqttClient) all take a fully-built config object as a prop
// rather than reading session fields directly the way SshTerminalTab does.
// Building that config inline at the call site (`config={buildRdpConfig(session)}`)
// creates a brand-new object every render, and since each viewer's connect
// effect depends on `config` by reference, that would silently reconnect on
// every unrelated re-render of the tab router (App.tsx's SessionCanvas /
// SplitPaneContainer's LeafPane). These wrappers exist purely to give each
// viewer a small component of its own that can `useMemo` the config keyed
// on the (referentially stable, from the session store) `session` object.
import { useMemo } from "react";
import RdpViewer from "@/components/RdpViewer/RdpViewer";
import VncViewer from "@/components/VncViewer/VncViewer";
import WebSocketTerminalTab from "@/components/Terminal/WebSocketTerminalTab";
import RedfishExplorer from "@/components/Redfish/RedfishExplorer";
import WebDavBrowser from "@/components/WebDav/WebDavBrowser";
import MqttClient from "@/components/Mqtt/MqttClient";
import SmbBrowser from "@/components/Smb/SmbBrowser";
import NetconfConsole from "@/components/Netconf/NetconfConsole";
import {
  buildRdpConfig,
  buildVncConfig,
  buildWsTermConfig,
  buildRedfishConfig,
  buildWebDavConfig,
  buildMqttConfig,
  buildSmbConfig,
  buildNetconfConfig,
} from "@/utils/sessionConfig";
import type { Session } from "@/types";

interface PaneProps {
  readonly sessionId: string;
  readonly session: Session;
}

export function RdpTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildRdpConfig(session), [session]);
  return <RdpViewer sessionId={sessionId} config={config} />;
}

export function VncTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildVncConfig(session), [session]);
  return <VncViewer sessionId={sessionId} config={config} />;
}

export function WsTermTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildWsTermConfig(session), [session]);
  return <WebSocketTerminalTab sessionId={sessionId} config={config} />;
}

export function RedfishTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildRedfishConfig(session), [session]);
  return <RedfishExplorer sessionId={sessionId} config={config} />;
}

export function WebDavTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildWebDavConfig(session), [session]);
  return <WebDavBrowser sessionId={sessionId} config={config} />;
}

export function MqttTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildMqttConfig(session), [session]);
  return <MqttClient sessionId={sessionId} config={config} />;
}

export function SmbTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildSmbConfig(session), [session]);
  return <SmbBrowser sessionId={sessionId} config={config} />;
}

export function NetconfTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildNetconfConfig(session), [session]);
  return <NetconfConsole sessionId={sessionId} config={config} />;
}
