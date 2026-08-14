import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import SplitPaneContainer from "@/components/Terminal/SplitPaneContainer";
import { useSessionStore } from "@/stores/sessionStore";
import { useTerminalStore } from "@/stores/terminalStore";
import { SessionType, ConnectionStatus } from "@/types";
import type { Session, Tab, SplitPaneLeaf } from "@/types";

// Same routing gap as App.tsx's SessionCanvas: before wiring these in,
// SplitPaneContainer's LeafPane only special-cased SSH — RDP/VNC/Telnet/
// Serial (fully built, never imported) fell through to the generic
// local-shell TerminalTab here too.
vi.mock("@/components/Terminal/TerminalTab", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`terminal-tab-${sessionId}`}>TerminalTab Mock</div>
  ),
}));
vi.mock("@/components/Terminal/SshTerminalTab", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`ssh-terminal-tab-${sessionId}`}>SshTerminalTab Mock</div>
  ),
}));
vi.mock("@/components/RdpViewer/RdpViewer", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`rdp-viewer-${sessionId}`}>RdpViewer Mock</div>
  ),
}));
vi.mock("@/components/VncViewer/VncViewer", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`vnc-viewer-${sessionId}`}>VncViewer Mock</div>
  ),
}));
vi.mock("@/components/Telnet/TelnetTerminal", () => ({
  default: () => <div data-testid="telnet-terminal">TelnetTerminal Mock</div>,
}));
vi.mock("@/components/Serial/SerialTerminal", () => ({
  default: () => <div data-testid="serial-terminal">SerialTerminal Mock</div>,
}));
vi.mock("@/components/Terminal/WebSocketTerminalTab", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`wsterm-tab-${sessionId}`}>WebSocketTerminalTab Mock</div>
  ),
}));
vi.mock("@/components/Redfish/RedfishExplorer", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`redfish-explorer-${sessionId}`}>RedfishExplorer Mock</div>
  ),
}));
vi.mock("@/components/WebDav/WebDavBrowser", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`webdav-browser-${sessionId}`}>WebDavBrowser Mock</div>
  ),
}));
vi.mock("@/components/Mqtt/MqttClient", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`mqtt-client-${sessionId}`}>MqttClient Mock</div>
  ),
}));

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

function baseTab(overrides: Partial<Tab> = {}): Tab {
  return {
    id: "tab-1",
    sessionId: "sess-1",
    title: "Test Tab",
    sessionType: SessionType.RDP,
    status: ConnectionStatus.Idle,
    pinned: false,
    order: 0,
    ...overrides,
  };
}

const leafPane: SplitPaneLeaf = { type: "leaf", tabId: "tab-1" };

describe("SplitPaneContainer session type routing", () => {
  beforeEach(() => {
    useTerminalStore.setState({ activePaneId: "tab-1" });
  });

  it("routes an RDP leaf pane to RdpViewer", () => {
    const session = baseSession({ type: SessionType.RDP });
    const tab = baseTab({ sessionType: SessionType.RDP });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`rdp-viewer-${tab.sessionId}`)).toBeInTheDocument();
    expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
  });

  it("routes a VNC leaf pane to VncViewer", () => {
    const session = baseSession({ type: SessionType.VNC, connection: { host: "192.168.0.11", port: 5900 } });
    const tab = baseTab({ sessionType: SessionType.VNC });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`vnc-viewer-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a Telnet leaf pane to TelnetTerminal", () => {
    const session = baseSession({ type: SessionType.Telnet });
    const tab = baseTab({ sessionType: SessionType.Telnet });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId("telnet-terminal")).toBeInTheDocument();
  });

  it("routes a Serial leaf pane to SerialTerminal", () => {
    const session = baseSession({ type: SessionType.Serial });
    const tab = baseTab({ sessionType: SessionType.Serial });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId("serial-terminal")).toBeInTheDocument();
  });

  it("still falls back to the generic TerminalTab for a session type with no dedicated component", () => {
    const session = baseSession({ type: SessionType.WSL });
    const tab = baseTab({ sessionType: SessionType.WSL });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`terminal-tab-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a WebSocket Terminal leaf pane to WebSocketTerminalTab", () => {
    const session = baseSession({ type: SessionType.WebSocketTerminal, connection: { host: "192.168.0.11", port: 7681 } });
    const tab = baseTab({ sessionType: SessionType.WebSocketTerminal });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`wsterm-tab-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a Redfish leaf pane to RedfishExplorer", () => {
    const session = baseSession({ type: SessionType.Redfish, connection: { host: "192.168.0.11", port: 443 } });
    const tab = baseTab({ sessionType: SessionType.Redfish });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`redfish-explorer-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a WebDAV leaf pane to WebDavBrowser", () => {
    const session = baseSession({ type: SessionType.WebDav, connection: { host: "192.168.0.11", port: 80 } });
    const tab = baseTab({ sessionType: SessionType.WebDav });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`webdav-browser-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes an MQTT leaf pane to MqttClient", () => {
    const session = baseSession({ type: SessionType.MqttClient, connection: { host: "192.168.0.11", port: 1883 } });
    const tab = baseTab({ sessionType: SessionType.MqttClient });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`mqtt-client-${tab.sessionId}`)).toBeInTheDocument();
  });
});
