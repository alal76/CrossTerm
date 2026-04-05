import { describe, it, expect, beforeEach, vi } from "vitest";
import { useSessionStore } from "@/stores/sessionStore";
import { SessionType, ConnectionStatus } from "@/types";
import type { Session } from "@/types";

function makeSession(overrides: Partial<Session> = {}): Session {
  return {
    id: overrides.id ?? "sess-1",
    name: overrides.name ?? "Test Server",
    type: SessionType.SSH,
    group: "default",
    tags: [],
    connection: { host: "10.0.0.1", port: 22 },
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    autoReconnect: false,
    keepAliveIntervalSeconds: 60,
    ...overrides,
  };
}

function resetStore() {
  useSessionStore.setState({
    sessions: [],
    sessionFolders: [],
    openTabs: [],
    activeTabId: null,
    splitPane: null,
    favorites: [],
    recentSessions: [],
  });
}

describe("sessionStore", () => {
  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  // ── Sessions ──

  describe("addSession", () => {
    it("adds a session to the list", () => {
      const session = makeSession();
      useSessionStore.getState().addSession(session);

      const { sessions } = useSessionStore.getState();
      expect(sessions).toHaveLength(1);
      expect(sessions[0].id).toBe("sess-1");
    });

    it("appends multiple sessions", () => {
      useSessionStore.getState().addSession(makeSession({ id: "a" }));
      useSessionStore.getState().addSession(makeSession({ id: "b" }));

      expect(useSessionStore.getState().sessions).toHaveLength(2);
    });
  });

  describe("removeSession", () => {
    it("removes a session and cleans up tabs and favorites", () => {
      const session = makeSession({ id: "rem-1" });
      const store = useSessionStore.getState();
      store.addSession(session);
      store.openTab(session);
      store.toggleFavorite("rem-1");

      useSessionStore.getState().removeSession("rem-1");

      const state = useSessionStore.getState();
      expect(state.sessions).toHaveLength(0);
      expect(state.openTabs).toHaveLength(0);
      expect(state.favorites).not.toContain("rem-1");
    });

    it("does nothing when removing non-existent id", () => {
      useSessionStore.getState().addSession(makeSession());
      useSessionStore.getState().removeSession("non-existent");

      expect(useSessionStore.getState().sessions).toHaveLength(1);
    });
  });

  // ── Tabs ──

  describe("openTab", () => {
    it("opens a new tab and sets it active", () => {
      const session = makeSession();
      useSessionStore.getState().addSession(session);
      useSessionStore.getState().openTab(session);

      const state = useSessionStore.getState();
      expect(state.openTabs).toHaveLength(1);
      expect(state.activeTabId).toBe(state.openTabs[0].id);
      expect(state.openTabs[0].sessionId).toBe("sess-1");
      expect(state.openTabs[0].status).toBe(ConnectionStatus.Idle);
    });

    it("reuses existing tab for same session", () => {
      const session = makeSession();
      useSessionStore.getState().openTab(session);
      useSessionStore.getState().openTab(session);

      expect(useSessionStore.getState().openTabs).toHaveLength(1);
    });
  });

  describe("closeTab", () => {
    it("removes the tab", () => {
      const session = makeSession();
      useSessionStore.getState().openTab(session);
      const tabId = useSessionStore.getState().openTabs[0].id;

      useSessionStore.getState().closeTab(tabId);

      expect(useSessionStore.getState().openTabs).toHaveLength(0);
      expect(useSessionStore.getState().activeTabId).toBeNull();
    });

    it("activates adjacent tab when closing active tab", () => {
      const s1 = makeSession({ id: "s1", name: "S1" });
      const s2 = makeSession({ id: "s2", name: "S2" });
      useSessionStore.getState().openTab(s1);
      useSessionStore.getState().openTab(s2);

      const tabs = useSessionStore.getState().openTabs;
      // s2 is active; close it, s1 should become active
      useSessionStore.getState().closeTab(tabs[1].id);

      const state = useSessionStore.getState();
      expect(state.openTabs).toHaveLength(1);
      expect(state.activeTabId).toBe(tabs[0].id);
    });
  });

  describe("reorderTab", () => {
    it("updates the order of a tab", () => {
      const s1 = makeSession({ id: "r1", name: "R1" });
      const s2 = makeSession({ id: "r2", name: "R2" });
      const s3 = makeSession({ id: "r3", name: "R3" });
      useSessionStore.getState().openTab(s1);
      useSessionStore.getState().openTab(s2);
      useSessionStore.getState().openTab(s3);

      const tabs = useSessionStore.getState().openTabs;
      // Move last tab to position 0
      useSessionStore.getState().reorderTab(tabs[2].id, 0);

      const updated = useSessionStore.getState().openTabs;
      const moved = updated.find((t) => t.sessionId === "r3");
      expect(moved?.order).toBe(0);
    });
  });

  describe("pinTab", () => {
    it("pins a tab", () => {
      const session = makeSession();
      useSessionStore.getState().openTab(session);
      const tabId = useSessionStore.getState().openTabs[0].id;

      useSessionStore.getState().pinTab(tabId);

      const tab = useSessionStore.getState().openTabs[0];
      expect(tab.pinned).toBe(true);
    });

    it("unpins a tab", () => {
      const session = makeSession();
      useSessionStore.getState().openTab(session);
      const tabId = useSessionStore.getState().openTabs[0].id;

      useSessionStore.getState().pinTab(tabId);
      useSessionStore.getState().unpinTab(tabId);

      expect(useSessionStore.getState().openTabs[0].pinned).toBe(false);
    });

    it("pinned tabs can be sorted before unpinned", () => {
      const s1 = makeSession({ id: "p1", name: "P1" });
      const s2 = makeSession({ id: "p2", name: "P2" });
      useSessionStore.getState().openTab(s1);
      useSessionStore.getState().openTab(s2);

      const tabs = useSessionStore.getState().openTabs;
      useSessionStore.getState().pinTab(tabs[1].id);

      const sorted = [...useSessionStore.getState().openTabs].sort(
        (a, b) => (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0)
      );
      expect(sorted[0].pinned).toBe(true);
      expect(sorted[0].sessionId).toBe("p2");
    });
  });

  // ── Favorites ──

  describe("toggleFavorite", () => {
    it("adds a session to favorites", () => {
      useSessionStore.getState().toggleFavorite("fav-1");
      expect(useSessionStore.getState().favorites).toContain("fav-1");
    });

    it("removes a session from favorites on second toggle", () => {
      useSessionStore.getState().toggleFavorite("fav-1");
      useSessionStore.getState().toggleFavorite("fav-1");
      expect(useSessionStore.getState().favorites).not.toContain("fav-1");
    });
  });

  // ── Update Session ──

  describe("updateSession", () => {
    it("updates session fields and sets updatedAt", () => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2025-01-01T00:00:00Z"));

      const session = makeSession({ id: "upd-1" });
      useSessionStore.getState().addSession(session);

      vi.setSystemTime(new Date("2025-01-01T00:01:00Z"));
      useSessionStore.getState().updateSession("upd-1", { name: "Renamed" });

      const updated = useSessionStore.getState().sessions[0];
      expect(updated.name).toBe("Renamed");
      expect(updated.updatedAt).not.toBe(session.updatedAt);

      vi.useRealTimers();
    });
  });
});
