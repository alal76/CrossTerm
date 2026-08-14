import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import App from "@/App";
import { useAppStore } from "@/stores/appStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useTerminalStore } from "@/stores/terminalStore";
import { useVaultStore } from "@/stores/vaultStore";
import { ThemeVariant, SessionType, ConnectionStatus } from "@/types";
import type { Session, Tab } from "@/types";

// Mock ResizeObserver (not available in jsdom)
class MockResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

// Mock heavy terminal components that use xterm.js / WebGL
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

vi.mock("@/components/Terminal/SplitPaneContainer", () => ({
  default: () => <div data-testid="split-pane-container">SplitPaneContainer Mock</div>,
}));

// Mock the four orphaned-viewer components (RDP/VNC/Telnet/Serial) so their
// routing branches in App.tsx can be exercised without pulling in canvas/
// xterm rendering.
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

// Phase 1: websocket_term/redfish/webdav/mqtt viewers — mocked the same way,
// via the ProtocolTabPanes wrappers they're routed through.
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

// Mock components that use localStorage directly (tested separately)
vi.mock("@/components/Help/WhatsNewPanel", () => ({
  default: () => null,
}));

vi.mock("@/components/Help/TipOfTheDay", () => ({
  default: () => null,
}));

vi.mock("@/components/Help/FeatureTour", () => ({
  default: () => null,
}));

vi.mock("@/components/Shared/FirstLaunchWizard", () => ({
  default: () => <div data-testid="first-launch-wizard">Wizard Mock</div>,
}));

// Mock useBreakpoint to return "expanded" by default (full layout)
const mockBreakpoint = vi.fn(() => "expanded");
vi.mock("@/hooks/useBreakpoint", () => ({
  useBreakpoint: () => mockBreakpoint(),
}));

function resetStores() {
  useAppStore.setState({
    firstLaunchComplete: true,
    sidebarCollapsed: false,
    bottomPanelVisible: false,
    theme: ThemeVariant.Dark,
    resolvedTheme: ThemeVariant.Dark,
    settingsOpen: false,
  });

  useSessionStore.setState({
    sessions: [],
    sessionFolders: [],
    openTabs: [],
    activeTabId: null,
    splitPane: null,
    favorites: [],
    recentSessions: [],
  });

  useTerminalStore.setState({
    terminals: new Map(),
    broadcastMode: false,
  });

  useVaultStore.setState({
    vaultLocked: true,
    credentials: [],
    loading: false,
    error: null,
  });
}

describe("App", () => {
  beforeEach(() => {
    resetStores();
    mockBreakpoint.mockReturnValue("expanded");
    vi.clearAllMocks();
    // Clear theme classes
    document.documentElement.classList.remove("light", "dark");
  });

  // FT-C-35: Renders all 6 layout regions
  it("FT-C-35: renders all 6 layout regions", () => {
    useAppStore.setState({ bottomPanelVisible: true });

    render(<App />);

    // Region A: TitleBar - contains "CrossTerm" branding
    expect(screen.getByText("CrossTerm")).toBeInTheDocument();

    // Region B: TabBar - has the new tab button and tablist
    expect(screen.getByRole("tablist")).toBeInTheDocument();

    // Region C: Sidebar - has the sidebar nav with mode buttons
    const nav = document.querySelector("nav");
    expect(nav).toBeInTheDocument();

    // Region D: SessionCanvas - empty canvas state when no tabs
    expect(screen.getByText("Welcome to CrossTerm")).toBeInTheDocument();

    // Region E: BottomPanel - visible when bottomPanelVisible is true
    expect(screen.getByLabelText("Bottom Panel")).toBeInTheDocument();

    // Region F: StatusBar - has the status footer
    expect(screen.getByRole("status")).toBeInTheDocument();
  });

  // FT-C-36: Ctrl+J toggles bottom panel
  it("FT-C-36: Ctrl+J toggles bottom panel", () => {
    render(<App />);

    // Bottom panel should not be visible initially
    expect(screen.queryByLabelText("Bottom Panel")).not.toBeInTheDocument();

    // Press Ctrl+J to toggle
    fireEvent.keyDown(document, { key: "j", ctrlKey: true });

    // Bottom panel should now be visible
    expect(screen.getByLabelText("Bottom Panel")).toBeInTheDocument();

    // Press Ctrl+J again to hide
    fireEvent.keyDown(document, { key: "j", ctrlKey: true });

    // Bottom panel should be hidden again
    expect(screen.queryByLabelText("Bottom Panel")).not.toBeInTheDocument();
  });

  // FT-C-37: Sidebar collapses at narrow width
  it("FT-C-37: sidebar collapses at narrow breakpoint", () => {
    // Start with compact breakpoint - sidebar should be hidden
    mockBreakpoint.mockReturnValue("compact");

    render(<App />);

    // On "compact" breakpoint, the sidebar nav is not rendered
    // The sidebar component returns null for compact

    // At compact, the bottom nav bar appears instead
    // Verify that sidebar session panel content is not visible
    expect(screen.queryByText("SESSIONS")).not.toBeInTheDocument();
  });

  // FT-C-38: Theme toggle switches dark/light and applies CSS class
  it("FT-C-38: theme toggle switches and applies CSS class", async () => {
    render(<App />);

    // Start in dark mode
    expect(document.documentElement.classList.contains("dark")).toBe(true);

    // Find the theme toggle button in the title bar
    // The button cycles Dark → Light → System
    const themeButton = screen.getByTitle("Dark");
    fireEvent.click(themeButton);

    // After click, theme should cycle to Light
    await waitFor(() => {
      expect(document.documentElement.classList.contains("light")).toBe(true);
      expect(document.documentElement.classList.contains("dark")).toBe(false);
    });
  });

  // Regression coverage for wiring RDP/VNC/Telnet/Serial into the tab
  // router: before this, SessionType.SSH was the only type with a real
  // component — everything else (including these four, whose components
  // were fully built but never imported anywhere) silently fell through to
  // the generic local-shell TerminalTab fallback.
  describe("session type routing (previously-orphaned viewers)", () => {
    function baseSession(overrides: Partial<Session>): Session {
      return {
        id: "sess-1",
        name: "Test Session",
        type: SessionType.SSH,
        tags: [],
        connection: { host: "192.168.0.11", port: 22 },
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
        autoReconnect: false,
        keepAliveIntervalSeconds: 30,
        ...overrides,
      };
    }

    function baseTab(overrides: Partial<Tab>): Tab {
      return {
        id: "tab-1",
        sessionId: "sess-1",
        title: "Test Tab",
        sessionType: SessionType.SSH,
        status: ConnectionStatus.Idle,
        pinned: false,
        order: 0,
        ...overrides,
      };
    }

    it("routes an RDP tab to RdpViewer, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.RDP, connection: { host: "192.168.0.11", port: 3389 } });
      const tab = baseTab({ sessionType: SessionType.RDP });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`rdp-viewer-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a VNC tab to VncViewer, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.VNC, connection: { host: "192.168.0.11", port: 5900 } });
      const tab = baseTab({ sessionType: SessionType.VNC });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`vnc-viewer-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a Telnet tab to TelnetTerminal, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.Telnet });
      const tab = baseTab({ sessionType: SessionType.Telnet });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId("telnet-terminal")).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a Serial tab to SerialTerminal, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.Serial });
      const tab = baseTab({ sessionType: SessionType.Serial });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId("serial-terminal")).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("still falls back to the generic TerminalTab for a session type with no dedicated component", () => {
      const session = baseSession({ type: SessionType.WSL });
      const tab = baseTab({ sessionType: SessionType.WSL });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`terminal-tab-${tab.sessionId}`)).toBeInTheDocument();
    });

    it("routes a WebSocket Terminal tab to WebSocketTerminalTab, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.WebSocketTerminal, connection: { host: "192.168.0.11", port: 7681 } });
      const tab = baseTab({ sessionType: SessionType.WebSocketTerminal });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`wsterm-tab-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a Redfish tab to RedfishExplorer, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.Redfish, connection: { host: "192.168.0.11", port: 443 } });
      const tab = baseTab({ sessionType: SessionType.Redfish });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`redfish-explorer-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a WebDAV tab to WebDavBrowser, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.WebDav, connection: { host: "192.168.0.11", port: 80 } });
      const tab = baseTab({ sessionType: SessionType.WebDav });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`webdav-browser-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes an MQTT tab to MqttClient, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.MqttClient, connection: { host: "192.168.0.11", port: 1883 } });
      const tab = baseTab({ sessionType: SessionType.MqttClient });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`mqtt-client-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes an SMB tab to SmbBrowser, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.Smb, connection: { host: "192.168.0.30", port: 445 } });
      const tab = baseTab({ sessionType: SessionType.Smb });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`smb-browser-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });
  });
});
