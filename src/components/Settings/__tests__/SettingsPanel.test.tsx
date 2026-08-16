import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import "@/i18n";
import SettingsPanel from "@/components/Settings/SettingsPanel";
import { ToastProvider } from "@/components/Shared/Toast";
import { useAppStore } from "@/stores/appStore";
import { ThemeVariant } from "@/types";

// Mock Tauri plugin-dialog and plugin-fs used by SettingsPanel
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
  readTextFile: vi.fn(),
}));

function resetStore() {
  useAppStore.setState({
    profiles: [{ id: "default", name: "Default", authMethod: "password" as const, createdAt: new Date().toISOString() }],
    theme: ThemeVariant.Dark,
    settingsOpen: true,
    customThemeName: null,
    customThemeTokens: null,
    bellStyle: "visual",
    cursorStyle: "block",
    cursorBlink: true,
  });
}

describe("SettingsPanel", () => {
  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  it("renders all 5 categories and switches on click", async () => {
    const user = userEvent.setup();

    render(<SettingsPanel />);

    // Core category buttons should be visible in the nav sidebar
    const navButtons = screen.getAllByRole("button").filter((btn) =>
      ["General", "Appearance", "Terminal", "SSH", "Security", "Connections", "File Transfer", "Keyboard", "Notifications", "Advanced"].includes(
        btn.textContent?.trim() ?? ""
      )
    );
    expect(navButtons.length).toBeGreaterThanOrEqual(5);

    const securityBtn = screen.getByRole("button", { name: /Security/ });
    const terminalBtn = screen.getByRole("button", { name: /Terminal/ });

    expect(securityBtn).toBeInTheDocument();
    expect(terminalBtn).toBeInTheDocument();

    // Clicking a different category should switch the heading
    await user.click(securityBtn);
    expect(screen.getByRole("heading", { name: "Security" })).toBeInTheDocument();

    await user.click(terminalBtn);
    expect(screen.getByRole("heading", { name: "Terminal" })).toBeInTheDocument();
  });

  it("close button sets settingsOpen to false", async () => {
    const user = userEvent.setup();
    render(<SettingsPanel />);

    await user.click(screen.getByTitle("Close settings"));

    expect(useAppStore.getState().settingsOpen).toBe(false);
  });

  it("pressing Escape sets settingsOpen to false", async () => {
    const user = userEvent.setup();
    render(<SettingsPanel />);

    await user.keyboard("{Escape}");

    expect(useAppStore.getState().settingsOpen).toBe(false);
  });

  it("FT-C-25: toggle change calls invoke('settings_update')", async () => {
    const user = userEvent.setup();
    const mockInvoke = vi.mocked(invoke);
    // settings_get returns defaults on mount; subsequent calls (settings_update) resolve
    mockInvoke.mockResolvedValue(undefined);

    render(<SettingsPanel />);

    // Wait for loading to complete (settings_get resolves)
    const { waitFor } = await import("@testing-library/react");
    await waitFor(() => {
      expect(screen.queryByText("Loading…")).not.toBeInTheDocument();
    });

    // The General category shows by default with toggle buttons.
    // Find toggle buttons by their distinct shape (w-9 h-5 rounded-full).
    const toggleButtons = screen.getAllByRole("button").filter((btn) =>
      btn.className.includes("rounded-full")
    );
    expect(toggleButtons.length).toBeGreaterThanOrEqual(1);

    // Click the first toggle (confirm_close_tab) — it starts as false, toggling sets to true
    await user.click(toggleButtons[0]);

    // invoke should be called with "settings_update"
    expect(mockInvoke).toHaveBeenCalledWith(
      "settings_update",
      expect.objectContaining({
        settings: expect.objectContaining({ confirm_close_tab: true }),
      })
    );
  });

  // Regression coverage: PolicyPanel, TeamPanel, KeyManager, and the
  // Language section (LocaleSelector/RtlSettings) were fully built and
  // tested in isolation but never mounted anywhere in SettingsPanel —
  // clicking their would-be category never existed, so they were entirely
  // unreachable in the running app.
  it("Keys, Policy, Team, and Language categories render their real panels", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "rbac_list_members") return Promise.resolve([]);
      if (cmd === "rbac_get_team_config") {
        return Promise.resolve({ members: [], require_mfa: false, session_timeout_minutes: 60, allowed_ips: [] });
      }
      if (cmd === "keymgr_list_keys" || cmd === "keymgr_agent_list" || cmd === "keymgr_cert_list") {
        return Promise.resolve([]);
      }
      if (cmd === "l10n_list_locales") return Promise.resolve([]);
      if (cmd === "l10n_get_locale") return Promise.resolve("en");
      return Promise.resolve(undefined);
    });
    render(<ToastProvider><SettingsPanel /></ToastProvider>);

    await user.click(screen.getByRole("button", { name: /Team/ }));
    expect((await screen.findAllByText("Invite Member")).length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: /Policy/ }));
    expect(await screen.findByText("Recording Policy")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Keys/ }));
    expect(screen.getAllByText("SSH Key Manager").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: /Language/ }));
    expect(screen.getByText("Interface language")).toBeInTheDocument();
    expect(screen.getByText("Enable RTL text support")).toBeInTheDocument();
  });

  // Regression coverage: PluginManager and PluginRegistry were fully built
  // and tested but never mounted anywhere — the whole plugin system was
  // unreachable from Settings.
  it("Plugins category toggles between the installed manager and the discovery registry", async () => {
    const user = userEvent.setup();
    globalThis.fetch = vi.fn().mockRejectedValue(new Error("offline"));
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "plugin_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<ToastProvider><SettingsPanel /></ToastProvider>);

    await user.click(screen.getByRole("button", { name: /Plugins/ }));
    expect(await screen.findByText("Plugin Manager")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Discover" }));
    expect(await screen.findByText("Plugin Registry")).toBeInTheDocument();
  });
});
