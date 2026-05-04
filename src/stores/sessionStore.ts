import { create } from "zustand";
import { v4 as uuidv4 } from "uuid";
import { invoke } from "@tauri-apps/api/core";
import { ConnectionStatus } from "@/types";
import type { Session, Tab, SplitPane, SmartGroup, FilterExpr } from "@/types";

interface SessionState {
  sessions: Session[];
  sessionFolders: string[];

  openTabs: Tab[];
  activeTabId: string | null;

  splitPane: SplitPane | null;

  favorites: string[];
  recentSessions: { sessionId: string; connectedAt: string }[];

  // ── Phase 2 ──
  selectedSessionIds: string[];
  smartGroups: SmartGroup[];
  activeSmartGroupId: string | null;

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

  // ── Phase 2 actions ──
  // Multi-select
  toggleSelectSession: (id: string) => void;
  selectSessions: (ids: string[]) => void;
  clearSelection: () => void;
  selectRange: (anchorId: string, targetId: string, allIds: string[]) => void;
  isSelected: (id: string) => boolean;

  // Smart group CRUD
  createSmartGroup: (name: string, filter: FilterExpr) => SmartGroup;
  updateSmartGroup: (
    id: string,
    updates: Partial<Pick<SmartGroup, "name" | "filter" | "icon" | "color">>
  ) => void;
  deleteSmartGroup: (id: string) => void;
  setActiveSmartGroup: (id: string | null) => void;
}

export const useSessionStore = create<SessionState>((set, get) => ({
  sessions: [],
  sessionFolders: [],

  openTabs: [],
  activeTabId: null,

  splitPane: null,

  favorites: [],
  recentSessions: [],

  // ── Phase 2 initial state ──
  selectedSessionIds: [],
  smartGroups: [],
  activeSmartGroupId: null,

  loadSessions: async () => {
    try {
      const sessions = await invoke<Session[]>("session_list");
      set({ sessions: Array.isArray(sessions) ? sessions : [] });
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
      selectedSessionIds: state.selectedSessionIds.filter((sid) => sid !== id),
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

  // ── Phase 2 action implementations ──

  toggleSelectSession: (id) =>
    set((state) => ({
      selectedSessionIds: state.selectedSessionIds.includes(id)
        ? state.selectedSessionIds.filter((sid) => sid !== id)
        : [...state.selectedSessionIds, id],
    })),

  selectSessions: (ids) =>
    set(() => ({ selectedSessionIds: ids })),

  clearSelection: () =>
    set(() => ({ selectedSessionIds: [] })),

  selectRange: (anchorId, targetId, allIds) => {
    const anchorIdx = allIds.indexOf(anchorId);
    const targetIdx = allIds.indexOf(targetId);
    if (anchorIdx === -1 || targetIdx === -1) return;
    const start = Math.min(anchorIdx, targetIdx);
    const end = Math.max(anchorIdx, targetIdx);
    const rangeIds = allIds.slice(start, end + 1);
    set(() => ({ selectedSessionIds: rangeIds }));
  },

  isSelected: (id) => get().selectedSessionIds.includes(id),

  createSmartGroup: (name, filter) => {
    const now = new Date().toISOString();
    const group: SmartGroup = {
      id: uuidv4(),
      name,
      filter,
      createdAt: now,
      updatedAt: now,
    };
    set((state) => ({ smartGroups: [...state.smartGroups, group] }));
    return group;
  },

  updateSmartGroup: (id, updates) =>
    set((state) => ({
      smartGroups: state.smartGroups.map((g) =>
        g.id === id
          ? { ...g, ...updates, updatedAt: new Date().toISOString() }
          : g
      ),
    })),

  deleteSmartGroup: (id) =>
    set((state) => ({
      smartGroups: state.smartGroups.filter((g) => g.id !== id),
      activeSmartGroupId:
        state.activeSmartGroupId === id ? null : state.activeSmartGroupId,
    })),

  setActiveSmartGroup: (id) => set(() => ({ activeSmartGroupId: id })),
}));

// ── Pure utility: evaluate a FilterExpr against a Session ──────────────────

export function evaluateFilterExpr(session: Session, expr: FilterExpr): boolean {
  switch (expr.type) {
    case "tag":
      return session.tags.includes(expr.value);

    case "protocol":
      return session.type === expr.value;

    case "status": {
      // Session.connectionStatus is not part of the core Session type;
      // treat missing / undefined as Disconnected.
      const status =
        (session as Session & { connectionStatus?: string }).connectionStatus ??
        ConnectionStatus.Disconnected;
      return status === expr.value;
    }

    case "last_connected_before": {
      if (!session.lastConnectedAt) return false;
      const cutoff = new Date();
      cutoff.setDate(cutoff.getDate() - expr.days);
      return new Date(session.lastConnectedAt) < cutoff;
    }

    case "name_contains":
      return session.name.toLowerCase().includes(expr.value.toLowerCase());

    case "host_contains":
      return session.connection.host
        .toLowerCase()
        .includes(expr.value.toLowerCase());

    case "and":
      return expr.children.every((child) => evaluateFilterExpr(session, child));

    case "or":
      return expr.children.some((child) => evaluateFilterExpr(session, child));

    case "not":
      return !evaluateFilterExpr(session, expr.child);
  }
}
