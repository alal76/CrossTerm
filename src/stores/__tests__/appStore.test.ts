import { describe, it, expect, beforeEach } from "vitest";
import { useAppStore } from "@/stores/appStore";
import { SidebarMode, BottomPanelMode, ThemeVariant } from "@/types";

function resetStore() {
  useAppStore.setState({
    sidebarMode: SidebarMode.Sessions,
    sidebarCollapsed: false,
    bottomPanelVisible: false,
    bottomPanelMode: BottomPanelMode.AuditLog,
    theme: ThemeVariant.Dark,
    resolvedTheme: ThemeVariant.Dark,
    settingsOpen: false,
    windowWidth: 1920,
    windowHeight: 1080,
  });
}

describe("appStore", () => {
  beforeEach(() => {
    resetStore();
  });

  // ── Theme ──

  describe("setTheme", () => {
    it("switches to light theme", () => {
      useAppStore.getState().setTheme(ThemeVariant.Light);

      const state = useAppStore.getState();
      expect(state.theme).toBe(ThemeVariant.Light);
      expect(state.resolvedTheme).toBe(ThemeVariant.Light);
    });

    it("switches to dark theme", () => {
      useAppStore.getState().setTheme(ThemeVariant.Light);
      useAppStore.getState().setTheme(ThemeVariant.Dark);

      expect(useAppStore.getState().theme).toBe(ThemeVariant.Dark);
      expect(useAppStore.getState().resolvedTheme).toBe(ThemeVariant.Dark);
    });

    it("resolves system theme via matchMedia", () => {
      useAppStore.getState().setTheme(ThemeVariant.System);

      const state = useAppStore.getState();
      expect(state.theme).toBe(ThemeVariant.System);
      // matchMedia mock returns true for dark
      expect(state.resolvedTheme).toBe(ThemeVariant.Dark);
    });
  });

  // ── Sidebar ──

  describe("setSidebarMode", () => {
    it("switches sidebar mode", () => {
      useAppStore.getState().setSidebarMode(SidebarMode.Snippets);
      expect(useAppStore.getState().sidebarMode).toBe(SidebarMode.Snippets);
    });

    it("switches to tunnels mode", () => {
      useAppStore.getState().setSidebarMode(SidebarMode.Tunnels);
      expect(useAppStore.getState().sidebarMode).toBe(SidebarMode.Tunnels);
    });
  });

  describe("toggleSidebar", () => {
    it("toggles sidebar collapsed state", () => {
      expect(useAppStore.getState().sidebarCollapsed).toBe(false);

      useAppStore.getState().toggleSidebar();
      expect(useAppStore.getState().sidebarCollapsed).toBe(true);

      useAppStore.getState().toggleSidebar();
      expect(useAppStore.getState().sidebarCollapsed).toBe(false);
    });
  });

  // ── Bottom Panel ──

  describe("setBottomPanelMode", () => {
    it("sets mode and makes panel visible", () => {
      useAppStore.getState().setBottomPanelMode(BottomPanelMode.SFTP);

      const state = useAppStore.getState();
      expect(state.bottomPanelMode).toBe(BottomPanelMode.SFTP);
      expect(state.bottomPanelVisible).toBe(true);
    });
  });

  describe("toggleBottomPanel", () => {
    it("toggles bottom panel visibility", () => {
      useAppStore.getState().toggleBottomPanel();
      expect(useAppStore.getState().bottomPanelVisible).toBe(true);

      useAppStore.getState().toggleBottomPanel();
      expect(useAppStore.getState().bottomPanelVisible).toBe(false);
    });
  });

  // ── Settings ──

  describe("setSettingsOpen", () => {
    it("opens and closes settings", () => {
      useAppStore.getState().setSettingsOpen(true);
      expect(useAppStore.getState().settingsOpen).toBe(true);

      useAppStore.getState().setSettingsOpen(false);
      expect(useAppStore.getState().settingsOpen).toBe(false);
    });
  });

  // ── Window Dimensions ──

  describe("setWindowDimensions", () => {
    it("updates dimensions", () => {
      useAppStore.getState().setWindowDimensions(1024, 768);

      const state = useAppStore.getState();
      expect(state.windowWidth).toBe(1024);
      expect(state.windowHeight).toBe(768);
    });

    it("collapses sidebar when width < 900", () => {
      useAppStore.getState().setWindowDimensions(800, 600);
      expect(useAppStore.getState().sidebarCollapsed).toBe(true);
    });

    it("does not collapse sidebar when width >= 900", () => {
      useAppStore.getState().setWindowDimensions(1200, 800);
      expect(useAppStore.getState().sidebarCollapsed).toBe(false);
    });
  });

  // ── Profiles ──

  describe("addProfile", () => {
    it("adds a profile", () => {
      useAppStore.getState().addProfile({
        id: "prof-2",
        name: "Work",
        authMethod: "password",
        createdAt: new Date().toISOString(),
      });

      const profiles = useAppStore.getState().profiles;
      expect(profiles.find((p) => p.id === "prof-2")).toBeDefined();
    });
  });
});
