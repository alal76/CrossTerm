import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import "@/i18n";
import App from "@/App";
import { useAppStore } from "@/stores/appStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useTerminalStore } from "@/stores/terminalStore";
import { useVaultStore } from "@/stores/vaultStore";
import { ThemeVariant, SessionType, ConnectionStatus, SidebarMode } from "@/types";
import type { Session, Tab } from "@/types";
import { SESSION_TYPE_OPTIONS } from "@/components/SessionTree/SessionEditor";

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

  // Regression coverage: `activeProfileId` defaults to the literal string
  // "default" (see appStore.ts) — never a real profile UUID — until
  // FirstLaunchWizard's setActiveProfile() call replaces it. If that never
  // ran (or the profile it pointed to was since removed), `profile_switch`
  // fails on every subsequent launch and every settings/session read/write
  // silently fails right along with it, forever, with no recovery path.
  it("recovers from a stale/missing active profile ID by switching to the most recently used real profile", async () => {
    useAppStore.setState({ activeProfileId: "default" });
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "profile_switch") {
        const { id } = args as { id: string };
        if (id === "default") return Promise.reject(new Error("Profile not found"));
        return Promise.resolve({ id, name: "Recovered" });
      }
      if (cmd === "profile_list") {
        return Promise.resolve([
          { id: "old-profile", updated_at: "2026-01-01T00:00:00Z" },
          { id: "recent-profile", updated_at: "2026-08-16T22:22:00Z" },
        ]);
      }
      if (cmd === "settings_get") return Promise.resolve({ theme: ThemeVariant.Dark });
      return Promise.resolve(undefined);
    });

    render(<App />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("profile_switch", { id: "recent-profile" });
    });
    expect(useAppStore.getState().activeProfileId).toBe("recent-profile");
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

  // Regression coverage for the "+" new-tab menu: it offers direct buttons
  // for only the handful of most-common protocols, but must always provide
  // a path to the rest via "More Session Types…" — now an inline submenu
  // (not a modal) so every remaining protocol is reachable in one extra
  // click rather than opening a full editor and hunting through a <select>.
  // Before the original version of this test, the menu silently drifted out
  // of sync as protocols were added — SESSION_TYPE_OPTIONS in
  // SessionEditor.tsx grew to cover every SessionType, but the "+" menu had
  // no escape hatch to reach it, so ~27 of the ~33 connectable session
  // types (everything past Local Shell/SSH/Telnet/RDP/VNC/SFTP) were
  // unreachable from the tab bar.
  it("every connectable SessionType is reachable from the '+' new-tab menu", () => {
    render(<App />);

    fireEvent.click(screen.getByTitle("New Tab"));
    fireEvent.click(screen.getByText("More Session Types…"));

    const nonConnectableTypes: SessionType[] = [
      SessionType.NetworkExplorer,
      SessionType.CloudDashboard,
      SessionType.CodeEditor,
      SessionType.DiffViewer,
      SessionType.Macros,
      SessionType.Recordings,
      SessionType.Ftp,
    ];
    const inlineTypes: SessionType[] = [
      SessionType.LocalShell,
      SessionType.SSH,
      SessionType.Telnet,
      SessionType.RDP,
      SessionType.VNC,
      SessionType.SFTP,
    ];
    const allTypes = Object.values(SessionType).filter((t) => !nonConnectableTypes.includes(t));
    const submenuTypes = allTypes.filter((t) => !inlineTypes.includes(t));

    // Every non-inline connectable type must appear as its own submenu item.
    expect(submenuTypes.length).toBeGreaterThan(0);
    for (const type of submenuTypes) {
      const option = SESSION_TYPE_OPTIONS.find((o) => o.value === type);
      expect(option, `SESSION_TYPE_OPTIONS is missing a label for ${type}`).toBeDefined();
      expect(screen.getByText(option!.label)).toBeInTheDocument();
    }
  });

  it("clicking a submenu session type opens the session editor with that type pre-selected", async () => {
    render(<App />);

    fireEvent.click(screen.getByTitle("New Tab"));
    fireEvent.click(screen.getByText("More Session Types…"));
    fireEvent.click(screen.getByText("Redfish (BMC REST)"));

    const typeSelect = await screen.findByRole("combobox");
    expect((typeSelect as HTMLSelectElement).value).toBe(SessionType.Redfish);
  });

  // Regression coverage: the Tunnels sidebar tab used to hardcode a
  // permanent "No tunnels" placeholder instead of ever rendering the fully
  // built PortForwardManager component.
  it("Tunnels sidebar tab renders PortForwardManager, not a static placeholder", () => {
    render(<App />);
    fireEvent.click(screen.getByTitle("Tunnels"));
    expect(screen.queryByText("No tunnels")).not.toBeInTheDocument();
    expect(screen.getByText("Port Forwards")).toBeInTheDocument();
  });

  // Regression coverage: PluginSidebar.tsx was fully built and tested but
  // had no sidebar mode to render it under.
  it("Plugins sidebar tab renders PluginSidebar", () => {
    render(<App />);
    fireEvent.click(screen.getByTitle("Plugins"));
    expect(screen.getByText("Sidebar Panels")).toBeInTheDocument();
  });

  // Regression coverage: the Sessions sidebar tab used to render a bare-bones
  // inline flat list (no folders, search, favorites, context menu, or
  // "Import" action) instead of the fully built SessionTree component, and
  // its "Import" button was wired to nothing (onImport was never passed
  // down from App.tsx).
  it("Sessions sidebar tab's Import button (empty state) opens the wizard", async () => {
    useAppStore.setState({ sidebarMode: SidebarMode.Sessions });
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "import_detect_sources") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<App />);
    fireEvent.click(screen.getByText("Import"));
    expect(await screen.findByText("Import Sessions")).toBeInTheDocument();
  });

  it("Sessions sidebar tab renders the real SessionTree, not the old flat-list fallback", () => {
    useAppStore.setState({ sidebarMode: SidebarMode.Sessions });
    useSessionStore.setState({
      sessions: [
        {
          id: "s1",
          name: "Prod Server",
          type: SessionType.SSH,
          group: "",
          tags: [],
          connection: { host: "10.0.0.1", port: 22 },
          createdAt: new Date().toISOString(),
          updatedAt: new Date().toISOString(),
          autoReconnect: false,
          keepAliveIntervalSeconds: 60,
        },
      ],
    });
    render(<App />);
    // The search bar only exists on the real SessionTree component, not the
    // old inline SessionsPanel fallback it replaced.
    expect(screen.getByPlaceholderText("Search sessions…")).toBeInTheDocument();
  });

  // Regression coverage: CloudDashboard.tsx (which wires together AWS/Azure/
  // GCP panels and the resource-explorer asset tree) was fully built and
  // tested but never reachable from anywhere in the app.
  it("'Cloud' in the + menu opens a tab rendering the real CloudDashboard", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "cloud_detect_clis") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<App />);

    fireEvent.click(screen.getByTitle("New Tab"));
    fireEvent.click(screen.getByText("Cloud"));

    expect(await screen.findByText("Cloud Dashboard")).toBeInTheDocument();
  });

  // Regression coverage: CodeEditor.tsx and DiffViewer.tsx were fully built
  // and tested but had no way to open them — they're local-file tools (the
  // backend's editor_open/editor_diff commands read the local filesystem,
  // not SFTP), so they're reachable as their own tabs rather than wired
  // into the SFTP browser's remote-file flow.
  it("'Code Editor' and 'Diff Viewer' in the + menu open their real components", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "editor_list_open") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<App />);

    fireEvent.click(screen.getByTitle("New Tab"));
    fireEvent.click(screen.getByText("Code Editor"));
    expect(await screen.findByText("No files open. Open a file to start editing.")).toBeInTheDocument();

    fireEvent.click(screen.getByTitle("New Tab"));
    fireEvent.click(screen.getByText("Diff Viewer"));
    expect(await screen.findByText("Compare Files")).toBeInTheDocument();
  });

  // Regression coverage: MacroEditor.tsx and ExpectRuleList.tsx were fully
  // built and tested but had no mount point anywhere in the app.
  it("Macro Editor in the + menu opens a tab with the real MacroEditor and ExpectRuleList", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "macro_list") return Promise.resolve([]);
      if (cmd === "expect_rule_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<App />);

    fireEvent.click(screen.getByTitle("New Tab"));
    fireEvent.click(screen.getByText("Macro Editor"));

    expect(await screen.findByText("No macros yet. Create a macro to automate terminal tasks.")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Expect Rules"));
    expect(await screen.findByText("No expect rules defined. Add a rule to auto-respond to patterns.")).toBeInTheDocument();
  });

  // Regression coverage: RecordingPlayer.tsx was fully built and tested but
  // had no way to reach it — recordings could be created but never browsed
  // or played back.
  it("Recordings in the + menu opens a tab listing and playing back real recordings", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "recording_list") {
        return Promise.resolve([
          { id: "rec-1", path: "/tmp/rec-1.cast", title: "demo", duration_secs: 12, size_bytes: 100, width: 80, height: 24, created_at: "2026-01-01T00:00:00Z" },
        ]);
      }
      return Promise.resolve(undefined);
    });
    render(<App />);

    fireEvent.click(screen.getByTitle("New Tab"));
    fireEvent.click(screen.getByText("Recordings"));

    expect(await screen.findByText("demo")).toBeInTheDocument();
    fireEvent.click(screen.getByText("demo"));
    expect(await screen.findByText("Speed:")).toBeInTheDocument();
  });

  // Regression coverage: FtpBrowser was fully built and tested, and FTP was
  // even a fully implemented backend protocol, but there was no
  // SessionType.Ftp at all — FTP wasn't reachable or even selectable
  // anywhere in the app.
  it("FTP in the + menu opens a tab with the real FtpBrowser", async () => {
    render(<App />);

    fireEvent.click(screen.getByTitle("New Tab"));
    fireEvent.click(screen.getByText("FTP"));

    expect(await screen.findByPlaceholderText("ftp.example.com")).toBeInTheDocument();
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

    it("routes a Proxmox Console tab to VncViewer with the proxmox connect command", () => {
      const session = baseSession({ type: SessionType.ProxmoxConsole, connection: { host: "10.0.0.5", port: 8006 } });
      const tab = baseTab({ sessionType: SessionType.ProxmoxConsole });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`vnc-viewer-${tab.sessionId}`)).toHaveAttribute(
        "data-connect-command",
        "proxmox_console_connect"
      );
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes an NFS Explorer tab to NfsExplorer, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.NfsExplorer, connection: { host: "10.0.0.20", port: 2049 } });
      const tab = baseTab({ sessionType: SessionType.NfsExplorer });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`nfs-explorer-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a Kubernetes Port-Forward tab to KubernetesPortForwardPanel, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.KubernetesPortForward, connection: { host: "ignored", port: 8080 } });
      const tab = baseTab({ sessionType: SessionType.KubernetesPortForward });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`k8s-port-forward-panel-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a SPICE Console tab to SpiceViewer, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.SpiceConsole, connection: { host: "10.0.0.30", port: 5900 } });
      const tab = baseTab({ sessionType: SessionType.SpiceConsole });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`spice-viewer-${tab.sessionId}`)).toBeInTheDocument();
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

    it("routes a NETCONF tab to NetconfConsole, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.NetConf, connection: { host: "192.168.0.40", port: 830 } });
      const tab = baseTab({ sessionType: SessionType.NetConf });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`netconf-console-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a Mosh tab to MoshTerminalTab, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.Mosh, connection: { host: "192.168.0.20", port: 22 } });
      const tab = baseTab({ sessionType: SessionType.Mosh });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`mosh-terminal-tab-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a WinRM tab to WinRmConsole, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.WinRM, connection: { host: "10.0.0.5", port: 5985 } });
      const tab = baseTab({ sessionType: SessionType.WinRM });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`winrm-console-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes an IPMI SOL tab to IpmiSolTab, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.IpmiSol, connection: { host: "10.0.0.10", port: 623 } });
      const tab = baseTab({ sessionType: SessionType.IpmiSol });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`ipmi-sol-tab-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes an SNMP tab to SnmpBrowser, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.Snmp, connection: { host: "10.0.0.30", port: 161 } });
      const tab = baseTab({ sessionType: SessionType.Snmp });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`snmp-browser-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a gRPC tab to GrpcExplorer, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.GrpcExplorer, connection: { host: "10.0.0.40", port: 50051 } });
      const tab = baseTab({ sessionType: SessionType.GrpcExplorer });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`grpc-explorer-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a TN3270 tab to Tn3270Screen, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.TN3270, connection: { host: "10.0.0.50", port: 23 } });
      const tab = baseTab({ sessionType: SessionType.TN3270 });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`tn3270-screen-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a TN5250 tab to Tn5250Screen, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.TN5250, connection: { host: "10.0.0.60", port: 23 } });
      const tab = baseTab({ sessionType: SessionType.TN5250 });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`tn5250-screen-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a Rlogin tab to RloginTerminalTab, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.Rlogin, connection: { host: "10.0.0.70", port: 513 } });
      const tab = baseTab({ sessionType: SessionType.Rlogin });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`rlogin-terminal-tab-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes a Docker Logs tab to DockerLogsViewer, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.DockerLogs, connection: { host: "10.0.0.80", port: 2375 } });
      const tab = baseTab({ sessionType: SessionType.DockerLogs });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`docker-logs-viewer-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });

    it("routes an X11 Forward tab to X11ForwardPanel, not the generic local-shell fallback", () => {
      const session = baseSession({ type: SessionType.X11Forward, connection: { host: "10.0.0.90", port: 22 } });
      const tab = baseTab({ sessionType: SessionType.X11Forward });
      useSessionStore.setState({ sessions: [session], openTabs: [tab], activeTabId: tab.id });

      render(<App />);

      expect(screen.getByTestId(`x11-forward-panel-${tab.sessionId}`)).toBeInTheDocument();
      expect(screen.queryByTestId(`terminal-tab-${tab.sessionId}`)).not.toBeInTheDocument();
    });
  });
});
