import { describe, it, expect, beforeEach, vi } from "vitest";
import { useSessionStore, evaluateFilterExpr } from "@/stores/sessionStore";
import { SessionType, ConnectionStatus } from "@/types";
import type { Session } from "@/types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

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
    selectedSessionIds: [],
    smartGroups: [],
    activeSmartGroupId: null,
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

  // ── Recent Sessions ──

  describe("addRecentSession", () => {
    it("caps the recent list at 25 entries", () => {
      for (let i = 0; i < 30; i++) {
        useSessionStore.getState().addRecentSession(`session-${i}`);
      }

      const { recentSessions } = useSessionStore.getState();
      expect(recentSessions).toHaveLength(25);
      // Most recent entry should be the last one added
      expect(recentSessions[0].sessionId).toBe("session-29");
    });
  });

  // ── Phase 2: Multi-select ──

  describe("toggleSelectSession", () => {
    it("adds a session id when not yet selected", () => {
      useSessionStore.getState().toggleSelectSession("sess-1");
      expect(useSessionStore.getState().selectedSessionIds).toContain("sess-1");
    });

    it("removes a session id when already selected", () => {
      useSessionStore.getState().toggleSelectSession("sess-1");
      useSessionStore.getState().toggleSelectSession("sess-1");
      expect(useSessionStore.getState().selectedSessionIds).not.toContain("sess-1");
    });

    it("can toggle multiple distinct sessions independently", () => {
      useSessionStore.getState().toggleSelectSession("a");
      useSessionStore.getState().toggleSelectSession("b");
      useSessionStore.getState().toggleSelectSession("a");

      const ids = useSessionStore.getState().selectedSessionIds;
      expect(ids).not.toContain("a");
      expect(ids).toContain("b");
    });
  });

  describe("selectRange", () => {
    const allIds = ["id-0", "id-1", "id-2", "id-3", "id-4"];

    it("selects a contiguous slice when anchor comes before target", () => {
      useSessionStore.getState().selectRange("id-1", "id-3", allIds);
      expect(useSessionStore.getState().selectedSessionIds).toEqual([
        "id-1",
        "id-2",
        "id-3",
      ]);
    });

    it("selects a contiguous slice when anchor comes after target", () => {
      useSessionStore.getState().selectRange("id-3", "id-1", allIds);
      expect(useSessionStore.getState().selectedSessionIds).toEqual([
        "id-1",
        "id-2",
        "id-3",
      ]);
    });

    it("selects a single item when anchor equals target", () => {
      useSessionStore.getState().selectRange("id-2", "id-2", allIds);
      expect(useSessionStore.getState().selectedSessionIds).toEqual(["id-2"]);
    });

    it("does nothing when anchor id is not in allIds", () => {
      useSessionStore.setState({ selectedSessionIds: ["id-0"] });
      useSessionStore.getState().selectRange("not-exist", "id-2", allIds);
      // state should be unchanged
      expect(useSessionStore.getState().selectedSessionIds).toEqual(["id-0"]);
    });
  });

  describe("clearSelection", () => {
    it("empties the selectedSessionIds array", () => {
      useSessionStore.setState({ selectedSessionIds: ["a", "b", "c"] });
      useSessionStore.getState().clearSelection();
      expect(useSessionStore.getState().selectedSessionIds).toHaveLength(0);
    });
  });

  describe("isSelected", () => {
    it("returns true for a selected id", () => {
      useSessionStore.setState({ selectedSessionIds: ["x"] });
      expect(useSessionStore.getState().isSelected("x")).toBe(true);
    });

    it("returns false for an unselected id", () => {
      useSessionStore.setState({ selectedSessionIds: [] });
      expect(useSessionStore.getState().isSelected("x")).toBe(false);
    });
  });

  // ── Phase 2: Smart groups ──

  describe("createSmartGroup", () => {
    it("creates a smart group with a uuid id and ISO timestamps", () => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2026-01-01T12:00:00Z"));

      const filter = { type: "tag" as const, value: "prod" };
      const group = useSessionStore.getState().createSmartGroup("Production", filter);

      expect(group.id).toMatch(
        /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i
      );
      expect(group.name).toBe("Production");
      expect(group.filter).toEqual(filter);
      expect(group.createdAt).toBe("2026-01-01T12:00:00.000Z");
      expect(group.updatedAt).toBe("2026-01-01T12:00:00.000Z");

      vi.useRealTimers();
    });

    it("adds the group to the store", () => {
      const filter = { type: "protocol" as const, value: SessionType.RDP };
      useSessionStore.getState().createSmartGroup("RDP sessions", filter);

      expect(useSessionStore.getState().smartGroups).toHaveLength(1);
      expect(useSessionStore.getState().smartGroups[0].name).toBe("RDP sessions");
    });
  });

  describe("deleteSmartGroup", () => {
    it("removes the group by id", () => {
      const filter = { type: "tag" as const, value: "dev" };
      const group = useSessionStore.getState().createSmartGroup("Dev", filter);

      useSessionStore.getState().deleteSmartGroup(group.id);

      expect(useSessionStore.getState().smartGroups).toHaveLength(0);
    });

    it("clears activeSmartGroupId if the deleted group was active", () => {
      const filter = { type: "tag" as const, value: "dev" };
      const group = useSessionStore.getState().createSmartGroup("Dev", filter);
      useSessionStore.getState().setActiveSmartGroup(group.id);

      useSessionStore.getState().deleteSmartGroup(group.id);

      expect(useSessionStore.getState().activeSmartGroupId).toBeNull();
    });

    it("does not clear activeSmartGroupId when a different group is deleted", () => {
      const filterA = { type: "tag" as const, value: "a" };
      const filterB = { type: "tag" as const, value: "b" };
      const groupA = useSessionStore.getState().createSmartGroup("A", filterA);
      const groupB = useSessionStore.getState().createSmartGroup("B", filterB);
      useSessionStore.getState().setActiveSmartGroup(groupA.id);

      useSessionStore.getState().deleteSmartGroup(groupB.id);

      expect(useSessionStore.getState().activeSmartGroupId).toBe(groupA.id);
    });
  });

  // ── Phase 2: evaluateFilterExpr ──

  describe("evaluateFilterExpr", () => {
    const baseSession = makeSession({
      id: "eval-1",
      name: "Prod Web Server",
      type: SessionType.SSH,
      tags: ["prod", "web"],
      connection: { host: "192.168.1.100", port: 22 },
    });

    describe("tag filter", () => {
      it("returns true when session has the tag", () => {
        expect(
          evaluateFilterExpr(baseSession, { type: "tag", value: "prod" })
        ).toBe(true);
      });

      it("returns false when session does not have the tag", () => {
        expect(
          evaluateFilterExpr(baseSession, { type: "tag", value: "staging" })
        ).toBe(false);
      });
    });

    describe("protocol filter", () => {
      it("returns true for matching protocol", () => {
        expect(
          evaluateFilterExpr(baseSession, { type: "protocol", value: SessionType.SSH })
        ).toBe(true);
      });

      it("returns false for non-matching protocol", () => {
        expect(
          evaluateFilterExpr(baseSession, { type: "protocol", value: SessionType.RDP })
        ).toBe(false);
      });
    });

    describe("name_contains filter", () => {
      it("returns true for case-insensitive substring match", () => {
        expect(
          evaluateFilterExpr(baseSession, { type: "name_contains", value: "prod" })
        ).toBe(true);
      });

      it("returns true with uppercase input against lowercase name portion", () => {
        expect(
          evaluateFilterExpr(baseSession, { type: "name_contains", value: "WEB" })
        ).toBe(true);
      });

      it("returns false when name does not contain the value", () => {
        expect(
          evaluateFilterExpr(baseSession, { type: "name_contains", value: "backup" })
        ).toBe(false);
      });
    });

    describe("host_contains filter", () => {
      it("returns true when host contains the substring", () => {
        expect(
          evaluateFilterExpr(baseSession, { type: "host_contains", value: "192.168" })
        ).toBe(true);
      });

      it("returns false when host does not contain the substring", () => {
        expect(
          evaluateFilterExpr(baseSession, { type: "host_contains", value: "10.0" })
        ).toBe(false);
      });
    });

    describe("and filter", () => {
      it("returns true when all children pass", () => {
        expect(
          evaluateFilterExpr(baseSession, {
            type: "and",
            children: [
              { type: "tag", value: "prod" },
              { type: "protocol", value: SessionType.SSH },
            ],
          })
        ).toBe(true);
      });

      it("returns false when any child fails", () => {
        expect(
          evaluateFilterExpr(baseSession, {
            type: "and",
            children: [
              { type: "tag", value: "prod" },
              { type: "tag", value: "nonexistent" },
            ],
          })
        ).toBe(false);
      });

      it("returns true for empty children (vacuously true)", () => {
        expect(
          evaluateFilterExpr(baseSession, { type: "and", children: [] })
        ).toBe(true);
      });
    });

    describe("or filter", () => {
      it("returns true when at least one child passes", () => {
        expect(
          evaluateFilterExpr(baseSession, {
            type: "or",
            children: [
              { type: "tag", value: "nonexistent" },
              { type: "tag", value: "prod" },
            ],
          })
        ).toBe(true);
      });

      it("returns false when all children fail", () => {
        expect(
          evaluateFilterExpr(baseSession, {
            type: "or",
            children: [
              { type: "tag", value: "staging" },
              { type: "tag", value: "dev" },
            ],
          })
        ).toBe(false);
      });
    });

    describe("not filter", () => {
      it("returns true when child fails", () => {
        expect(
          evaluateFilterExpr(baseSession, {
            type: "not",
            child: { type: "tag", value: "nonexistent" },
          })
        ).toBe(true);
      });

      it("returns false when child passes", () => {
        expect(
          evaluateFilterExpr(baseSession, {
            type: "not",
            child: { type: "tag", value: "prod" },
          })
        ).toBe(false);
      });
    });

    describe("last_connected_before filter", () => {
      it("returns true for sessions connected 30 days ago (beyond the 7-day threshold)", () => {
        vi.useFakeTimers();
        const now = new Date("2026-05-04T12:00:00Z");
        vi.setSystemTime(now);

        const thirtyDaysAgo = new Date(now);
        thirtyDaysAgo.setDate(thirtyDaysAgo.getDate() - 30);

        const session = makeSession({
          lastConnectedAt: thirtyDaysAgo.toISOString(),
        });

        expect(
          evaluateFilterExpr(session, { type: "last_connected_before", days: 7 })
        ).toBe(true);

        vi.useRealTimers();
      });

      it("returns false for sessions connected today (within the 7-day threshold)", () => {
        vi.useFakeTimers();
        const now = new Date("2026-05-04T12:00:00Z");
        vi.setSystemTime(now);

        const session = makeSession({
          lastConnectedAt: now.toISOString(),
        });

        expect(
          evaluateFilterExpr(session, { type: "last_connected_before", days: 7 })
        ).toBe(false);

        vi.useRealTimers();
      });

      it("returns false when lastConnectedAt is absent", () => {
        const session = makeSession({ lastConnectedAt: undefined });
        expect(
          evaluateFilterExpr(session, { type: "last_connected_before", days: 7 })
        ).toBe(false);
      });
    });
  });
});
