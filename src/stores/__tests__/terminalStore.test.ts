import { describe, it, expect, beforeEach } from "vitest";
import { useTerminalStore } from "@/stores/terminalStore";
import { ConnectionStatus } from "@/types";

function resetStore() {
  useTerminalStore.setState({ terminals: new Map() });
}

describe("terminalStore", () => {
  beforeEach(() => {
    resetStore();
  });

  // ── Create Terminal ──

  describe("createTerminal", () => {
    it("creates a terminal with correct defaults", () => {
      useTerminalStore.getState().createTerminal("sess-1", "term-1");

      const term = useTerminalStore.getState().terminals.get("term-1");
      expect(term).toBeDefined();
      expect(term!.sessionId).toBe("sess-1");
      expect(term!.status).toBe(ConnectionStatus.Idle);
      expect(term!.cols).toBe(80);
      expect(term!.rows).toBe(24);
      expect(term!.title).toBe("");
    });

    it("creates multiple terminals", () => {
      useTerminalStore.getState().createTerminal("sess-1", "term-1");
      useTerminalStore.getState().createTerminal("sess-2", "term-2");

      expect(useTerminalStore.getState().terminals.size).toBe(2);
    });
  });

  // ── Remove Terminal ──

  describe("removeTerminal", () => {
    it("removes an existing terminal", () => {
      useTerminalStore.getState().createTerminal("sess-1", "term-1");
      useTerminalStore.getState().removeTerminal("term-1");

      expect(useTerminalStore.getState().terminals.size).toBe(0);
    });

    it("does nothing for non-existent terminal", () => {
      useTerminalStore.getState().createTerminal("sess-1", "term-1");
      useTerminalStore.getState().removeTerminal("non-existent");

      expect(useTerminalStore.getState().terminals.size).toBe(1);
    });
  });

  // ── Update Status ──

  describe("updateTerminalStatus", () => {
    it("updates the terminal status", () => {
      useTerminalStore.getState().createTerminal("sess-1", "term-1");
      useTerminalStore
        .getState()
        .updateTerminalStatus("term-1", ConnectionStatus.Connected);

      const term = useTerminalStore.getState().terminals.get("term-1");
      expect(term!.status).toBe(ConnectionStatus.Connected);
    });
  });

  // ── Update Dimensions ──

  describe("updateTerminalDimensions", () => {
    it("updates cols and rows", () => {
      useTerminalStore.getState().createTerminal("sess-1", "term-1");
      useTerminalStore.getState().updateTerminalDimensions("term-1", 120, 40);

      const term = useTerminalStore.getState().terminals.get("term-1");
      expect(term!.cols).toBe(120);
      expect(term!.rows).toBe(40);
    });
  });

  // ── Update Title ──

  describe("updateTerminalTitle", () => {
    it("sets terminal title", () => {
      useTerminalStore.getState().createTerminal("sess-1", "term-1");
      useTerminalStore.getState().updateTerminalTitle("term-1", "root@server");

      const term = useTerminalStore.getState().terminals.get("term-1");
      expect(term!.title).toBe("root@server");
    });
  });

  // ── Get By Session ──

  describe("getTerminalBySession", () => {
    it("finds terminal by session id", () => {
      useTerminalStore.getState().createTerminal("sess-1", "term-1");

      const term = useTerminalStore.getState().getTerminalBySession("sess-1");
      expect(term).toBeDefined();
      expect(term!.id).toBe("term-1");
    });

    it("returns undefined for unknown session", () => {
      const term = useTerminalStore.getState().getTerminalBySession("nope");
      expect(term).toBeUndefined();
    });
  });
});
