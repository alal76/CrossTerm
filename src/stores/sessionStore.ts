import { create } from "zustand";
import { v4 as uuidv4 } from "uuid";
import { invoke } from "@tauri-apps/api/core";
import { ConnectionStatus } from "@/types";
import type { Session, Tab, SplitPane } from "@/types";

interface SessionState {
  sessions: Session[];
  sessionFolders: string[];

  openTabs: Tab[];
  activeTabId: string | null;

  splitPane: SplitPane | null;

  favorites: string[];
  recentSessions: { sessionId: string; connectedAt: string }[];

  loadSessions: () => Promise<void>;
  addSession: (session: Session) => void;
  removeSession: (id: string) => void;
  updateSession: (id: string, updates: Partial<Session>) => void;

  openTab: (session: Session) => void;
  closeTab: (tabId: string) => void;
  setActiveTab: (tabId: string) => void;
  reorderTab: (tabId: string, newOrder: number) => void;
  pinTab: (tabId: string) => void;
  unpinTab: (tabId: string) => void;
  updateTabStatus: (tabId: string, status: ConnectionStatus) => void;

  setSplitPane: (pane: SplitPane | null) => void;

  toggleFavorite: (sessionId: string) => void;
  addRecentSession: (sessionId: string) => void;

  addFolder: (folderPath: string) => void;
  removeFolder: (folderPath: string) => void;
}

export const useSessionStore = create<SessionState>((set) => ({
  sessions: [],
  sessionFolders: [],

  openTabs: [],
  activeTabId: null,

  splitPane: null,

  favorites: [],
  recentSessions: [],

  loadSessions: async () => {
    try {
      const sessions = await invoke<Session[]>("session_list");
      set({ sessions });
    } catch {
      // Profile may not be active yet
    }
  },

  addSession: (session) =>
    set((state) => ({ sessions: [...state.sessions, session] })),

  removeSession: (id) =>
    set((state) => ({
      sessions: state.sessions.filter((s) => s.id !== id),
      openTabs: state.openTabs.filter((t) => t.sessionId !== id),
      favorites: state.favorites.filter((fid) => fid !== id),
    })),

  updateSession: (id, updates) =>
    set((state) => ({
      sessions: state.sessions.map((s) =>
        s.id === id ? { ...s, ...updates, updatedAt: new Date().toISOString() } : s
      ),
    })),

  openTab: (session) =>
    set((state) => {
      const existing = state.openTabs.find((t) => t.sessionId === session.id);
      if (existing) {
        return { activeTabId: existing.id };
      }
      const tab: Tab = {
        id: uuidv4(),
        sessionId: session.id,
        title: session.name,
        sessionType: session.type,
        status: ConnectionStatus.Idle,
        pinned: false,
        order: state.openTabs.length,
      };
      return {
        openTabs: [...state.openTabs, tab],
        activeTabId: tab.id,
      };
    }),

  closeTab: (tabId) =>
    set((state) => {
      const filtered = state.openTabs.filter((t) => t.id !== tabId);
      let newActiveId = state.activeTabId;
      if (state.activeTabId === tabId) {
        const idx = state.openTabs.findIndex((t) => t.id === tabId);
        newActiveId = filtered[Math.min(idx, filtered.length - 1)]?.id ?? null;
      }
      return { openTabs: filtered, activeTabId: newActiveId };
    }),

  setActiveTab: (tabId) => set({ activeTabId: tabId }),

  reorderTab: (tabId, newOrder) =>
    set((state) => ({
      openTabs: state.openTabs.map((t) =>
        t.id === tabId ? { ...t, order: newOrder } : t
      ),
    })),

  pinTab: (tabId) =>
    set((state) => ({
      openTabs: state.openTabs.map((t) =>
        t.id === tabId ? { ...t, pinned: true } : t
      ),
    })),

  unpinTab: (tabId) =>
    set((state) => ({
      openTabs: state.openTabs.map((t) =>
        t.id === tabId ? { ...t, pinned: false } : t
      ),
    })),

  updateTabStatus: (tabId, status) =>
    set((state) => ({
      openTabs: state.openTabs.map((t) =>
        t.id === tabId ? { ...t, status } : t
      ),
    })),

  setSplitPane: (pane) => set({ splitPane: pane }),

  toggleFavorite: (sessionId) =>
    set((state) => ({
      favorites: state.favorites.includes(sessionId)
        ? state.favorites.filter((id) => id !== sessionId)
        : [...state.favorites, sessionId],
    })),

  addRecentSession: (sessionId) =>
    set((state) => {
      const entry = { sessionId, connectedAt: new Date().toISOString() };
      const filtered = state.recentSessions.filter(
        (r) => r.sessionId !== sessionId
      );
      return { recentSessions: [entry, ...filtered].slice(0, 25) };
    }),

  addFolder: (folderPath) =>
    set((state) => ({
      sessionFolders: state.sessionFolders.includes(folderPath)
        ? state.sessionFolders
        : [...state.sessionFolders, folderPath],
    })),

  removeFolder: (folderPath) =>
    set((state) => ({
      sessionFolders: state.sessionFolders.filter((f) => f !== folderPath),
    })),
}));
