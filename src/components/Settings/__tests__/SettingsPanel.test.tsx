import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import "@/i18n";
import SettingsPanel from "@/components/Settings/SettingsPanel";
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

    // All 5 category buttons should be visible in the nav sidebar
    const navButtons = screen.getAllByRole("button").filter((btn) =>
      ["General", "Appearance", "Terminal", "SSH", "Security"].includes(
        btn.textContent?.trim() ?? ""
      )
    );
    expect(navButtons).toHaveLength(5);

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

    // The General category shows by default with toggle buttons (Auto Update, GPU Acceleration).
    // Find toggle buttons by their distinct shape (w-9 h-5 rounded-full).
    const toggleButtons = screen.getAllByRole("button").filter((btn) =>
      btn.className.includes("rounded-full")
    );
    expect(toggleButtons.length).toBeGreaterThanOrEqual(1);

    // Click the first toggle (auto_update) — it starts as true, toggling sets to false
    await user.click(toggleButtons[0]);

    // invoke should be called with "settings_update"
    expect(mockInvoke).toHaveBeenCalledWith(
      "settings_update",
      expect.objectContaining({
        settings: expect.objectContaining({ auto_update: false }),
      })
    );
  });
});
