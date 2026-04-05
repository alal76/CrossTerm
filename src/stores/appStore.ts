import { create } from "zustand";
import { SidebarMode, BottomPanelMode, ThemeVariant } from "@/types";
import type { Profile } from "@/types";

function getSystemTheme(): ThemeVariant.Dark | ThemeVariant.Light {
  return globalThis.matchMedia("(prefers-color-scheme: dark)").matches
    ? ThemeVariant.Dark
    : ThemeVariant.Light;
}

interface AppState {
  activeProfileId: string | null;
  profiles: Profile[];

  sidebarMode: SidebarMode;
  sidebarCollapsed: boolean;

  bottomPanelVisible: boolean;
  bottomPanelMode: BottomPanelMode;

  theme: ThemeVariant;
  resolvedTheme: ThemeVariant.Dark | ThemeVariant.Light;

  settingsOpen: boolean;
  firstLaunchComplete: boolean;

  windowWidth: number;
  windowHeight: number;

  setSidebarMode: (mode: SidebarMode) => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  toggleSidebar: () => void;

  setBottomPanelVisible: (visible: boolean) => void;
  toggleBottomPanel: () => void;
  setBottomPanelMode: (mode: BottomPanelMode) => void;

  setTheme: (theme: ThemeVariant) => void;
  setResolvedTheme: (resolved: ThemeVariant.Dark | ThemeVariant.Light) => void;

  setSettingsOpen: (open: boolean) => void;

  setFirstLaunchComplete: (complete: boolean) => void;

  setActiveProfile: (id: string) => void;
  addProfile: (profile: Profile) => void;

  setWindowDimensions: (width: number, height: number) => void;
}

const initialTheme = ThemeVariant.Dark;

export const useAppStore = create<AppState>((set) => ({
  activeProfileId: "default",
  profiles: [
    {
      id: "default",
      name: "Default",
      authMethod: "password",
      createdAt: new Date().toISOString(),
    },
  ],

  sidebarMode: SidebarMode.Sessions,
  sidebarCollapsed: false,

  bottomPanelVisible: false,
  bottomPanelMode: BottomPanelMode.AuditLog,

  theme: initialTheme,
  resolvedTheme: getSystemTheme(),

  settingsOpen: false,
  firstLaunchComplete: false,

  windowWidth: window.innerWidth,
  windowHeight: window.innerHeight,

  setSidebarMode: (mode) => set({ sidebarMode: mode }),
  setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
  toggleSidebar: () =>
    set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),

  setBottomPanelVisible: (visible) => set({ bottomPanelVisible: visible }),
  toggleBottomPanel: () =>
    set((state) => ({ bottomPanelVisible: !state.bottomPanelVisible })),
  setBottomPanelMode: (mode) =>
    set({ bottomPanelMode: mode, bottomPanelVisible: true }),

  setTheme: (theme) => {
    const resolved: ThemeVariant.Dark | ThemeVariant.Light =
      theme === ThemeVariant.System ? getSystemTheme() : theme;
    set({ theme, resolvedTheme: resolved });
  },

  setResolvedTheme: (resolved) => set({ resolvedTheme: resolved }),

  setSettingsOpen: (open) => set({ settingsOpen: open }),

  setFirstLaunchComplete: (complete) => set({ firstLaunchComplete: complete }),

  setActiveProfile: (id) => set({ activeProfileId: id }),
  addProfile: (profile) =>
    set((state) => ({ profiles: [...state.profiles, profile] })),

  setWindowDimensions: (width, height) =>
    set({
      windowWidth: width,
      windowHeight: height,
      sidebarCollapsed: width < 900,
    }),
}));
