import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
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
  save: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
  readTextFile: vi.fn(),
  writeTextFile: vi.fn(),
}));

function resetStore() {
  useAppStore.setState({
    profiles: [{ id: "default", name: "Default", authMethod: "password" as const, createdAt: new Date().toISOString() }],
    theme: ThemeVariant.Dark,
    settingsOpen: true,
    settingsInitialCategory: null,
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

  // Regression coverage: the native OS menu bar's Settings submenu items
  // (General, Appearance, ... Advanced) all opened to the same default
  // General tab regardless of which one was clicked — this is the panel
  // side of the fix, which reads a store-set initial category on open.
  it("opens directly to the category set via settingsInitialCategory, then clears it", async () => {
    useAppStore.setState({ settingsInitialCategory: "advanced" });

    render(<SettingsPanel />);

    expect(screen.getByRole("heading", { name: "Advanced" })).toBeInTheDocument();
    expect(useAppStore.getState().settingsInitialCategory).toBeNull();
  });

  it("falls back to General when no initial category is set", () => {
    render(<SettingsPanel />);
    expect(screen.getByRole("heading", { name: "General" })).toBeInTheDocument();
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

  // Regression coverage: a failed settings_update (e.g. a stale active
  // profile ID on the backend) used to be silently swallowed — the toggle
  // stayed showing "on" in the UI even though nothing was actually
  // persisted, with no indication anything had gone wrong.
  it("surfaces an error toast when settings_update fails instead of silently keeping the optimistic value", async () => {
    const user = userEvent.setup();
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "settings_get") return Promise.resolve(undefined);
      if (cmd === "settings_update") return Promise.reject(new Error("Profile not found"));
      return Promise.resolve(undefined);
    });

    render(<ToastProvider><SettingsPanel /></ToastProvider>);
    await waitFor(() => expect(screen.queryByText("Loading…")).not.toBeInTheDocument());

    const toggleButtons = screen.getAllByRole("button").filter((btn) =>
      btn.className.includes("rounded-full")
    );
    await user.click(toggleButtons[0]);

    expect(await screen.findByText(/Failed to save settings/)).toBeInTheDocument();
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

  // Diagnostic logging is off by default in every build (release included)
  // and is a per-profile Settings toggle rather than always-on — confirms
  // the Advanced tab actually exposes and persists it.
  it("Advanced category's diagnostic logging toggle persists via settings_update", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockResolvedValue(undefined);
    render(<ToastProvider><SettingsPanel /></ToastProvider>);

    await user.click(screen.getByRole("button", { name: /Advanced/ }));
    const label = await screen.findByText("Enable Diagnostic Logging");
    const row = label.closest("div")?.parentElement;
    const toggle = row?.querySelector("button.rounded-full");
    expect(toggle).toBeTruthy();

    await user.click(toggle as HTMLElement);

    expect(vi.mocked(invoke)).toHaveBeenCalledWith(
      "settings_update",
      expect.objectContaining({
        settings: expect.objectContaining({ enable_diagnostic_logging: true }),
      })
    );
  });

  // Regression coverage: the Export Audit Log button discarded the CSV
  // string returned by the backend instead of ever saving it — clicking it
  // produced zero visible result.
  it("Advanced category's Export Audit Log button saves the real CSV to disk", async () => {
    const user = userEvent.setup();
    const { save } = await import("@tauri-apps/plugin-dialog");
    const { writeTextFile } = await import("@tauri-apps/plugin-fs");
    vi.mocked(save).mockResolvedValue("/tmp/crossterm-audit-log.csv");
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "audit_log_export_csv") return Promise.resolve("timestamp,event\n2026-01-01,VaultUnlock\n");
      return Promise.resolve(undefined);
    });
    render(<ToastProvider><SettingsPanel /></ToastProvider>);

    await user.click(screen.getByRole("button", { name: /Advanced/ }));
    await user.click(screen.getByText("Export Audit Log"));

    await vi.waitFor(() => {
      expect(writeTextFile).toHaveBeenCalledWith(
        "/tmp/crossterm-audit-log.csv",
        "timestamp,event\n2026-01-01,VaultUnlock\n",
      );
    });
  });

  // Regression coverage: SsoConfigForm and SsoProviderList were fully built
  // and tested but never mounted anywhere in Settings, so there was no way
  // to configure an OIDC SSO provider at all.
  it("Security category lists, adds, and deletes OIDC SSO providers", async () => {
    const user = userEvent.setup();
    let configs: Array<{ provider_name: string; client_id: string }> = [];
    vi.mocked(invoke).mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "auth_list_oidc_configs") return Promise.resolve(configs);
      if (cmd === "auth_save_oidc_config") {
        const { config } = args as { config: { provider_name: string; client_id: string } };
        configs = [...configs, config];
        return Promise.resolve(undefined);
      }
      if (cmd === "auth_delete_oidc_config") {
        const { providerName } = args as { providerName: string };
        configs = configs.filter((c) => c.provider_name !== providerName);
        return Promise.resolve(undefined);
      }
      return Promise.resolve(undefined);
    });
    render(<ToastProvider><SettingsPanel /></ToastProvider>);

    await user.click(screen.getByRole("button", { name: /Security/ }));
    expect(await screen.findByText("No OIDC providers configured.")).toBeInTheDocument();

    await user.click(screen.getByText("Add provider"));
    await user.type(screen.getByPlaceholderText("e.g. Okta"), "Okta");
    await user.type(screen.getByPlaceholderText("0oa1b2c3d4e5f6g7h8i9"), "client-1");
    await user.type(screen.getByPlaceholderText("https://your-idp.example.com/oauth2/authorize"), "https://idp.example.com/authorize");
    await user.type(screen.getByPlaceholderText("https://your-idp.example.com/oauth2/token"), "https://idp.example.com/token");
    await user.click(screen.getByText("Save provider"));

    expect(await screen.findByText("Okta")).toBeInTheDocument();

    await user.click(screen.getByTitle("Remove Okta"));
    await screen.findByText("No OIDC providers configured.");
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
