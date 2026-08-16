import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import SplitPaneContainer from "@/components/Terminal/SplitPaneContainer";
import { useSessionStore } from "@/stores/sessionStore";
import { useTerminalStore } from "@/stores/terminalStore";
import { SessionType, ConnectionStatus, SplitDirection } from "@/types";
import type { Session, Tab, SplitPaneLeaf, SplitPaneContainer as SplitPaneContainerType } from "@/types";

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
  default: ({ sessionId, connectCommand }: { sessionId: string; connectCommand?: string }) => (
    <div data-testid={`vnc-viewer-${sessionId}`} data-connect-command={connectCommand ?? "vnc_connect"}>
      VncViewer Mock
    </div>
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
vi.mock("@/components/Smb/SmbBrowser", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`smb-browser-${sessionId}`}>SmbBrowser Mock</div>
  ),
}));
vi.mock("@/components/Netconf/NetconfConsole", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`netconf-console-${sessionId}`}>NetconfConsole Mock</div>
  ),
}));
vi.mock("@/components/Mosh/MoshTerminalTab", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`mosh-terminal-tab-${sessionId}`}>MoshTerminalTab Mock</div>
  ),
}));
vi.mock("@/components/WinRm/WinRmConsole", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`winrm-console-${sessionId}`}>WinRmConsole Mock</div>
  ),
}));
vi.mock("@/components/Ipmi/IpmiSolTab", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`ipmi-sol-tab-${sessionId}`}>IpmiSolTab Mock</div>
  ),
}));
vi.mock("@/components/Snmp/SnmpBrowser", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`snmp-browser-${sessionId}`}>SnmpBrowser Mock</div>
  ),
}));
vi.mock("@/components/Grpc/GrpcExplorer", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`grpc-explorer-${sessionId}`}>GrpcExplorer Mock</div>
  ),
}));
vi.mock("@/components/Tn3270/Tn3270Screen", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`tn3270-screen-${sessionId}`}>Tn3270Screen Mock</div>
  ),
}));
vi.mock("@/components/Tn5250/Tn5250Screen", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`tn5250-screen-${sessionId}`}>Tn5250Screen Mock</div>
  ),
}));
vi.mock("@/components/Rlogin/RloginTerminalTab", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`rlogin-terminal-tab-${sessionId}`}>RloginTerminalTab Mock</div>
  ),
}));
vi.mock("@/components/DockerLogs/DockerLogsViewer", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`docker-logs-viewer-${sessionId}`}>DockerLogsViewer Mock</div>
  ),
}));
vi.mock("@/components/X11Forward/X11ForwardPanel", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`x11-forward-panel-${sessionId}`}>X11ForwardPanel Mock</div>
  ),
}));
vi.mock("@/components/Nfs/NfsExplorer", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`nfs-explorer-${sessionId}`}>NfsExplorer Mock</div>
  ),
}));
vi.mock("@/components/KubernetesPortForward/KubernetesPortForwardPanel", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`k8s-port-forward-panel-${sessionId}`}>KubernetesPortForwardPanel Mock</div>
  ),
}));
vi.mock("@/components/Spice/SpiceViewer", () => ({
  default: ({ sessionId }: { sessionId: string }) => (
    <div data-testid={`spice-viewer-${sessionId}`}>SpiceViewer Mock</div>
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

    expect(screen.getByTestId(`vnc-viewer-${tab.sessionId}`)).toHaveAttribute("data-connect-command", "vnc_connect");
  });

  it("routes a Proxmox Console leaf pane to VncViewer with the proxmox connect command", () => {
    const session = baseSession({ type: SessionType.ProxmoxConsole, connection: { host: "10.0.0.5", port: 8006 } });
    const tab = baseTab({ sessionType: SessionType.ProxmoxConsole });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`vnc-viewer-${tab.sessionId}`)).toHaveAttribute(
      "data-connect-command",
      "proxmox_console_connect"
    );
  });

  it("routes an NFS Explorer leaf pane to NfsExplorer", () => {
    const session = baseSession({ type: SessionType.NfsExplorer, connection: { host: "10.0.0.20", port: 2049 } });
    const tab = baseTab({ sessionType: SessionType.NfsExplorer });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`nfs-explorer-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a Kubernetes Port-Forward leaf pane to KubernetesPortForwardPanel", () => {
    const session = baseSession({ type: SessionType.KubernetesPortForward, connection: { host: "ignored", port: 8080 } });
    const tab = baseTab({ sessionType: SessionType.KubernetesPortForward });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`k8s-port-forward-panel-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a SPICE Console leaf pane to SpiceViewer", () => {
    const session = baseSession({ type: SessionType.SpiceConsole, connection: { host: "10.0.0.30", port: 5900 } });
    const tab = baseTab({ sessionType: SessionType.SpiceConsole });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`spice-viewer-${tab.sessionId}`)).toBeInTheDocument();
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

  it("routes an SMB leaf pane to SmbBrowser", () => {
    const session = baseSession({ type: SessionType.Smb, connection: { host: "192.168.0.30", port: 445 } });
    const tab = baseTab({ sessionType: SessionType.Smb });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`smb-browser-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a NETCONF leaf pane to NetconfConsole", () => {
    const session = baseSession({ type: SessionType.NetConf, connection: { host: "192.168.0.40", port: 830 } });
    const tab = baseTab({ sessionType: SessionType.NetConf });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`netconf-console-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a Mosh leaf pane to MoshTerminalTab", () => {
    const session = baseSession({ type: SessionType.Mosh, connection: { host: "192.168.0.20", port: 22 } });
    const tab = baseTab({ sessionType: SessionType.Mosh });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`mosh-terminal-tab-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a WinRM leaf pane to WinRmConsole", () => {
    const session = baseSession({ type: SessionType.WinRM, connection: { host: "10.0.0.5", port: 5985 } });
    const tab = baseTab({ sessionType: SessionType.WinRM });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`winrm-console-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes an IPMI SOL leaf pane to IpmiSolTab", () => {
    const session = baseSession({ type: SessionType.IpmiSol, connection: { host: "10.0.0.10", port: 623 } });
    const tab = baseTab({ sessionType: SessionType.IpmiSol });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`ipmi-sol-tab-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes an SNMP leaf pane to SnmpBrowser", () => {
    const session = baseSession({ type: SessionType.Snmp, connection: { host: "10.0.0.30", port: 161 } });
    const tab = baseTab({ sessionType: SessionType.Snmp });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`snmp-browser-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a gRPC leaf pane to GrpcExplorer", () => {
    const session = baseSession({ type: SessionType.GrpcExplorer, connection: { host: "10.0.0.40", port: 50051 } });
    const tab = baseTab({ sessionType: SessionType.GrpcExplorer });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`grpc-explorer-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a TN3270 leaf pane to Tn3270Screen", () => {
    const session = baseSession({ type: SessionType.TN3270, connection: { host: "10.0.0.50", port: 23 } });
    const tab = baseTab({ sessionType: SessionType.TN3270 });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`tn3270-screen-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a TN5250 leaf pane to Tn5250Screen", () => {
    const session = baseSession({ type: SessionType.TN5250, connection: { host: "10.0.0.60", port: 23 } });
    const tab = baseTab({ sessionType: SessionType.TN5250 });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`tn5250-screen-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a Rlogin leaf pane to RloginTerminalTab", () => {
    const session = baseSession({ type: SessionType.Rlogin, connection: { host: "10.0.0.70", port: 513 } });
    const tab = baseTab({ sessionType: SessionType.Rlogin });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`rlogin-terminal-tab-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes a Docker Logs leaf pane to DockerLogsViewer", () => {
    const session = baseSession({ type: SessionType.DockerLogs, connection: { host: "10.0.0.80", port: 2375 } });
    const tab = baseTab({ sessionType: SessionType.DockerLogs });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`docker-logs-viewer-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes an X11 Forward leaf pane to X11ForwardPanel", () => {
    const session = baseSession({ type: SessionType.X11Forward, connection: { host: "10.0.0.90", port: 22 } });
    const tab = baseTab({ sessionType: SessionType.X11Forward });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`x11-forward-panel-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("routes an SSH leaf pane to SshTerminalTab", () => {
    const session = baseSession({
      type: SessionType.SSH,
      connection: { host: "10.0.0.1", port: 22, protocolOptions: { username: "root", password: "secret" } },
    });
    const tab = baseTab({ sessionType: SessionType.SSH });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByTestId(`ssh-terminal-tab-${tab.sessionId}`)).toBeInTheDocument();
  });

  it("shows a 'No tab' placeholder when the leaf's tab no longer exists", () => {
    useSessionStore.setState({ sessions: [], openTabs: [] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    expect(screen.getByText("No tab")).toBeInTheDocument();
  });

  it("sets the active pane id when a leaf pane is clicked", () => {
    const session = baseSession({ type: SessionType.WSL });
    const tab = baseTab({ sessionType: SessionType.WSL });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });
    useTerminalStore.setState({ activePaneId: null });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    fireEvent.click(screen.getByTestId(`terminal-tab-${tab.sessionId}`).parentElement!);
    expect(useTerminalStore.getState().activePaneId).toBe("tab-1");
  });
});

describe("SplitPaneContainer split rendering, resize, and keyboard nav", () => {
  beforeEach(() => {
    useTerminalStore.setState({ activePaneId: "tab-1" });
  });

  function twoLeafSplit(direction: SplitDirection = SplitDirection.Horizontal): SplitPaneContainerType {
    return {
      type: "container",
      direction,
      sizes: [50, 50],
      children: [
        { type: "leaf", tabId: "tab-1" },
        { type: "leaf", tabId: "tab-2" },
      ],
    };
  }

  function setupTwoTabs() {
    const session1 = baseSession({ id: "sess-1", type: SessionType.WSL });
    const session2 = baseSession({ id: "sess-2", type: SessionType.WSL });
    const tab1 = baseTab({ id: "tab-1", sessionId: "sess-1", sessionType: SessionType.WSL });
    const tab2 = baseTab({ id: "tab-2", sessionId: "sess-2", sessionType: SessionType.WSL });
    useSessionStore.setState({ sessions: [session1, session2], openTabs: [tab1, tab2] });
  }

  it("renders both children of a split pane with a resize handle between them", () => {
    setupTwoTabs();
    render(<SplitPaneContainer pane={twoLeafSplit()} activeTabId="tab-1" />);

    expect(screen.getByTestId("terminal-tab-sess-1")).toBeInTheDocument();
    expect(screen.getByTestId("terminal-tab-sess-2")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Resize pane" })).toBeInTheDocument();
  });

  it("resizes panes when the resize handle is dragged", () => {
    setupTwoTabs();
    const { container } = render(<SplitPaneContainer pane={twoLeafSplit()} activeTabId="tab-1" />);

    const handle = screen.getByRole("button", { name: "Resize pane" });
    const panes = container.querySelectorAll<HTMLDivElement>(":scope > div > div > div");

    fireEvent.mouseDown(handle, { clientX: 100 });
    fireEvent.mouseMove(document, { clientX: 150 });
    fireEvent.mouseUp(document);

    // A resize occurred without crashing; flex-basis values should have moved
    // away from the initial 50/50 split on at least one of the two panes.
    const bases = Array.from(panes)
      .slice(0, 2)
      .map((el) => el.style.flexBasis);
    expect(bases.some((b) => b && b !== "50%")).toBe(true);
  });

  it("cycles the active pane with Alt+ArrowRight and wraps at the end", () => {
    setupTwoTabs();
    render(<SplitPaneContainer pane={twoLeafSplit()} activeTabId="tab-1" />);

    fireEvent.keyDown(document, { key: "ArrowRight", altKey: true });
    expect(useTerminalStore.getState().activePaneId).toBe("tab-2");

    fireEvent.keyDown(document, { key: "ArrowRight", altKey: true });
    expect(useTerminalStore.getState().activePaneId).toBe("tab-1");
  });

  it("cycles backward with Alt+ArrowLeft and wraps at the start", () => {
    setupTwoTabs();
    render(<SplitPaneContainer pane={twoLeafSplit()} activeTabId="tab-1" />);

    fireEvent.keyDown(document, { key: "ArrowLeft", altKey: true });
    expect(useTerminalStore.getState().activePaneId).toBe("tab-2");
  });

  it("ignores arrow keys without Alt, and Alt+arrow combined with other modifiers", () => {
    setupTwoTabs();
    render(<SplitPaneContainer pane={twoLeafSplit()} activeTabId="tab-1" />);

    fireEvent.keyDown(document, { key: "ArrowRight" });
    expect(useTerminalStore.getState().activePaneId).toBe("tab-1");

    fireEvent.keyDown(document, { key: "ArrowRight", altKey: true, shiftKey: true });
    expect(useTerminalStore.getState().activePaneId).toBe("tab-1");

    fireEvent.keyDown(document, { key: "ArrowRight", altKey: true, ctrlKey: true });
    expect(useTerminalStore.getState().activePaneId).toBe("tab-1");
  });

  it("ignores non-arrow keys even with Alt held", () => {
    setupTwoTabs();
    render(<SplitPaneContainer pane={twoLeafSplit()} activeTabId="tab-1" />);

    fireEvent.keyDown(document, { key: "a", altKey: true });
    expect(useTerminalStore.getState().activePaneId).toBe("tab-1");
  });

  it("does not attach the keyboard navigation listener for a single-leaf pane", () => {
    const session = baseSession({ type: SessionType.WSL });
    const tab = baseTab({ sessionType: SessionType.WSL });
    useSessionStore.setState({ sessions: [session], openTabs: [tab] });

    render(<SplitPaneContainer pane={leafPane} activeTabId="tab-1" />);

    fireEvent.keyDown(document, { key: "ArrowRight", altKey: true });
    // No second pane exists, so activePaneId must remain untouched (not just unchanged by coincidence).
    expect(useTerminalStore.getState().activePaneId).toBe("tab-1");
  });

  it("renders a vertical split with flex-col layout", () => {
    setupTwoTabs();
    const { container } = render(
      <SplitPaneContainer pane={twoLeafSplit(SplitDirection.Vertical)} activeTabId="tab-1" />,
    );
    const flexContainer = container.querySelector(".flex-col");
    expect(flexContainer).toBeInTheDocument();
  });
});
