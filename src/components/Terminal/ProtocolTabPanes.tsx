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
import MoshTerminalTab from "@/components/Mosh/MoshTerminalTab";
import WinRmConsole from "@/components/WinRm/WinRmConsole";
import IpmiSolTab from "@/components/Ipmi/IpmiSolTab";
import SnmpBrowser from "@/components/Snmp/SnmpBrowser";
import GrpcExplorer from "@/components/Grpc/GrpcExplorer";
import Tn3270Screen from "@/components/Tn3270/Tn3270Screen";
import Tn5250Screen from "@/components/Tn5250/Tn5250Screen";
import RloginTerminalTab from "@/components/Rlogin/RloginTerminalTab";
import DockerLogsViewer from "@/components/DockerLogs/DockerLogsViewer";
import {
  buildRdpConfig,
  buildVncConfig,
  buildWsTermConfig,
  buildRedfishConfig,
  buildWebDavConfig,
  buildMqttConfig,
  buildSmbConfig,
  buildNetconfConfig,
  buildMoshConfig,
  buildWinRmConfig,
  buildIpmiConfig,
  buildSnmpConfig,
  buildGrpcConfig,
  buildTn3270Config,
  buildTn5250Config,
  buildRloginConfig,
  buildDockerLogsConfig,
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

export function MoshTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildMoshConfig(session), [session]);
  return <MoshTerminalTab sessionId={sessionId} config={config} />;
}

export function WinRmTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildWinRmConfig(session), [session]);
  return <WinRmConsole sessionId={sessionId} config={config} />;
}

export function IpmiSolTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildIpmiConfig(session), [session]);
  return <IpmiSolTab sessionId={sessionId} config={config} />;
}

export function SnmpTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildSnmpConfig(session), [session]);
  return <SnmpBrowser sessionId={sessionId} config={config} />;
}

export function GrpcTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildGrpcConfig(session), [session]);
  return <GrpcExplorer sessionId={sessionId} config={config} />;
}

export function Tn3270TabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildTn3270Config(session), [session]);
  return <Tn3270Screen sessionId={sessionId} config={config} />;
}

export function Tn5250TabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildTn5250Config(session), [session]);
  return <Tn5250Screen sessionId={sessionId} config={config} />;
}

export function RloginTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildRloginConfig(session), [session]);
  return <RloginTerminalTab sessionId={sessionId} config={config} />;
}

export function DockerLogsTabPane({ sessionId, session }: PaneProps) {
  const config = useMemo(() => buildDockerLogsConfig(session), [session]);
  return <DockerLogsViewer sessionId={sessionId} config={config} />;
}
