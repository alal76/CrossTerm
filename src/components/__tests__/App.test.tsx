import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import App from "@/App";
import { useAppStore } from "@/stores/appStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useTerminalStore } from "@/stores/terminalStore";
import { useVaultStore } from "@/stores/vaultStore";
import { ThemeVariant } from "@/types";

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
});
