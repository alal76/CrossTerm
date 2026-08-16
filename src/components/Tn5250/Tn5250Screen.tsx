import { useEffect, useState, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Loader2, AlertTriangle } from "lucide-react";
import type { Tn5250Config, Tn5250Screen as Tn5250ScreenData, Tn5250Aid } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface Tn5250ScreenProps {
  readonly sessionId: string;
  readonly config: Tn5250Config;
}

const AID_KEYS: { aid: Tn5250Aid; label: string }[] = [
  { aid: "enter", label: "Enter" },
  { aid: "clear", label: "Clear" },
  { aid: "help", label: "Help" },
  { aid: "roll_up", label: "Roll Up" },
  { aid: "roll_down", label: "Roll Down" },
  { aid: "f1", label: "F1" },
  { aid: "f2", label: "F2" },
  { aid: "f3", label: "F3" },
  { aid: "f4", label: "F4" },
  { aid: "f5", label: "F5" },
  { aid: "f6", label: "F6" },
  { aid: "f7", label: "F7" },
  { aid: "f8", label: "F8" },
  { aid: "f9", label: "F9" },
  { aid: "f10", label: "F10" },
  { aid: "f11", label: "F11" },
  { aid: "f12", label: "F12" },
];

/// Finds the start of the field that owns (row, col) — mirrors the
/// backend's backward-scan-with-wraparound `field_start_for`.
function fieldStartFor(screen: Tn5250ScreenData, addr: number): number | null {
  const n = screen.cells.length;
  for (let step = 0; step < n; step++) {
    const a = (addr - step + n) % n;
    if (screen.cells[a].field_start) return a;
  }
  return null;
}

export default function Tn5250Screen({ sessionId, config }: Tn5250ScreenProps) {
  const { toast } = useToast();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const connectionIdRef = useRef<string | null>(null);
  const [status, setStatus] = useState<"connecting" | "connected" | "error">("connecting");
  const [error, setError] = useState<string | null>(null);
  const [screen, setScreen] = useState<Tn5250ScreenData | null>(null);
  const [selectedAddr, setSelectedAddr] = useState<number | null>(null);
  const [fieldText, setFieldText] = useState("");

  useEffect(() => {
    let cancelled = false;
    let unlistenScreen: (() => void) | null = null;
    setStatus("connecting");
    setError(null);

    invoke<string>("tn5250_connect", { config })
      .then(async (id) => {
        if (cancelled) {
          invoke("tn5250_disconnect", { id }).catch(() => {});
          return;
        }
        connectionIdRef.current = id;
        // Attach the listener before flipping to "connected" — doing it in a
        // separate effect keyed on connectionId left a window (one render
        // wide) where the grid was visible but tn5250:screen events would
        // be silently dropped.
        const unlisten = await listen<Tn5250ScreenData>("tn5250:screen", (event) => {
          if (event.payload.session_id === id) {
            setScreen(event.payload);
          }
        });
        if (cancelled) {
          unlisten();
          return;
        }
        unlistenScreen = unlisten;
        setConnectionId(id);
        setStatus("connected");
      })
      .catch((e) => {
        if (!cancelled) {
          setStatus("error");
          setError(String(e));
        }
      });

    return () => {
      cancelled = true;
      unlistenScreen?.();
      const id = connectionIdRef.current;
      if (id) invoke("tn5250_disconnect", { id }).catch(() => {});
      connectionIdRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  const rows = useMemo(() => {
    if (!screen) return [];
    const out: { ch: string; addr: number; row: number; col: number; bypass: boolean; fieldStart: boolean }[][] = [];
    for (let r = 0; r < screen.rows; r++) {
      const row = [];
      for (let c = 0; c < screen.cols; c++) {
        const addr = r * screen.cols + c;
        const cell = screen.cells[addr];
        row.push({ ch: cell.field_start ? " " : cell.ch, addr, row: r, col: c, bypass: cell.bypass, fieldStart: cell.field_start });
      }
      out.push(row);
    }
    return out;
  }, [screen]);

  const handleCellClick = useCallback(
    (addr: number) => {
      if (!screen || screen.cells[addr].bypass) return;
      setSelectedAddr(addr);
      const fieldStart = fieldStartFor(screen, addr);
      if (fieldStart === null) {
        setFieldText("");
        return;
      }
      let text = "";
      const size = screen.cells.length;
      let a = (fieldStart + 1) % size;
      while (a !== fieldStart && !screen.cells[a].field_start) {
        text += screen.cells[a].ch;
        a = (a + 1) % size;
      }
      setFieldText(text.trimEnd());
    },
    [screen]
  );

  const commitField = useCallback(async () => {
    if (!connectionId || selectedAddr === null || !screen) return;
    const fieldStart = fieldStartFor(screen, selectedAddr);
    const dataAddr = fieldStart === null ? selectedAddr : (fieldStart + 1) % screen.cells.length;
    const row = Math.floor(dataAddr / screen.cols);
    const col = dataAddr % screen.cols;
    try {
      const updated = await invoke<Tn5250ScreenData>("tn5250_type", { id: connectionId, row, col, text: fieldText });
      setScreen(updated);
    } catch (e) {
      toast("error", `Type failed: ${String(e)}`);
    }
  }, [connectionId, selectedAddr, fieldText, screen, toast]);

  const sendAid = useCallback(
    async (aid: Tn5250Aid) => {
      if (!connectionId) return;
      try {
        await invoke("tn5250_aid", { id: connectionId, aid });
      } catch (e) {
        toast("error", `${aid} failed: ${String(e)}`);
      }
    },
    [connectionId, toast]
  );

  if (status === "connecting") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 bg-[var(--terminal-bg)] text-text-secondary">
        <Loader2 size={20} className="animate-spin" />
        <span className="text-sm">Connecting to {config.host}…</span>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="flex h-full w-full flex-col items-center justify-center gap-2 bg-[var(--terminal-bg)] p-4 text-center text-status-disconnected">
        <AlertTriangle size={20} />
        <span className="text-sm">Failed to connect to {config.host}</span>
        <span className="max-w-md break-all text-xs text-text-disabled">{error}</span>
      </div>
    );
  }

  return (
    <div className="flex h-full w-full flex-col gap-2 overflow-auto bg-[var(--terminal-bg)] p-3">
      <div
        data-testid={`tn5250-grid-${sessionId}`}
        className="w-fit select-none whitespace-pre font-mono text-sm leading-tight text-[var(--terminal-fg)]"
      >
        {rows.map((row, r) => (
          <div key={r} className="flex">
            {row.map((cell) => (
              <span
                key={cell.addr}
                onClick={() => handleCellClick(cell.addr)}
                className={
                  cell.addr === selectedAddr
                    ? "bg-accent-primary text-text-inverse"
                    : cell.bypass
                      ? "text-text-disabled"
                      : "text-status-connected cursor-text hover:bg-surface-elevated"
                }
              >
                {cell.ch === " " ? " " : cell.ch}
              </span>
            ))}
          </div>
        ))}
      </div>

      <div className="flex shrink-0 items-center gap-2 border-t border-border-default pt-2">
        <input
          value={fieldText}
          onChange={(e) => setFieldText(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && commitField()}
          disabled={selectedAddr === null}
          placeholder={selectedAddr === null ? "Click an input field to edit it" : "Field value"}
          className="flex-1 rounded-md border border-border-default bg-surface-primary px-2 py-1 font-mono text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none disabled:opacity-50"
        />
        <button
          onClick={commitField}
          disabled={selectedAddr === null}
          className="rounded-md bg-interactive-default px-3 py-1 text-xs font-medium text-text-inverse hover:bg-interactive-hover transition-colors disabled:opacity-50"
        >
          Apply
        </button>
      </div>

      <div className="flex shrink-0 flex-wrap gap-1 border-t border-border-default pt-2">
        {AID_KEYS.map(({ aid, label }) => (
          <button
            key={aid}
            onClick={() => sendAid(aid)}
            className="rounded px-2 py-1 text-xs text-text-secondary hover:bg-surface-elevated hover:text-text-primary transition-colors"
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
