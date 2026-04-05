import { create } from "zustand";
import { ConnectionStatus } from "@/types";
import type { TerminalInstance } from "@/types";

interface TerminalState {
  terminals: Map<string, TerminalInstance>;
  activePaneId: string | null;
  broadcastMode: boolean;

  createTerminal: (sessionId: string, id: string) => void;
  removeTerminal: (id: string) => void;
  updateTerminalStatus: (id: string, status: ConnectionStatus) => void;
  updateTerminalDimensions: (id: string, cols: number, rows: number) => void;
  updateTerminalTitle: (id: string, title: string) => void;
  getTerminalBySession: (sessionId: string) => TerminalInstance | undefined;
  setActivePaneId: (id: string | null) => void;
  setBroadcastMode: (mode: boolean) => void;
  toggleBroadcastMode: () => void;
}

export const useTerminalStore = create<TerminalState>((set, get) => ({
  terminals: new Map(),
  activePaneId: null,
  broadcastMode: false,

  createTerminal: (sessionId, id) =>
    set((state) => {
      const next = new Map(state.terminals);
      next.set(id, {
        id,
        sessionId,
        status: ConnectionStatus.Idle,
        cols: 80,
        rows: 24,
        title: "",
      });
      return { terminals: next };
    }),

  removeTerminal: (id) =>
    set((state) => {
      const next = new Map(state.terminals);
      next.delete(id);
      return { terminals: next };
    }),

  updateTerminalStatus: (id, status) =>
    set((state) => {
      const next = new Map(state.terminals);
      const term = next.get(id);
      if (term) next.set(id, { ...term, status });
      return { terminals: next };
    }),

  updateTerminalDimensions: (id, cols, rows) =>
    set((state) => {
      const next = new Map(state.terminals);
      const term = next.get(id);
      if (term) next.set(id, { ...term, cols, rows });
      return { terminals: next };
    }),

  updateTerminalTitle: (id, title) =>
    set((state) => {
      const next = new Map(state.terminals);
      const term = next.get(id);
      if (term) next.set(id, { ...term, title });
      return { terminals: next };
    }),

  getTerminalBySession: (sessionId) => {
    const terminals = get().terminals;
    for (const term of terminals.values()) {
      if (term.sessionId === sessionId) return term;
    }
    return undefined;
  },

  setActivePaneId: (id) => set({ activePaneId: id }),
  setBroadcastMode: (mode) => set({ broadcastMode: mode }),
  toggleBroadcastMode: () => set((state) => ({ broadcastMode: !state.broadcastMode })),
}));
