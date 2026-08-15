import { useEffect, useState, useCallback, useMemo, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Loader2, AlertTriangle } from "lucide-react";
import type { Tn3270Config, Tn3270Screen as Tn3270ScreenData, Tn3270Aid } from "@/types";
import { useToast } from "@/components/Shared/Toast";

interface Tn3270ScreenProps {
  readonly sessionId: string;
  readonly config: Tn3270Config;
}

const AID_KEYS: { aid: Tn3270Aid; label: string }[] = [
  { aid: "enter", label: "Enter" },
  { aid: "clear", label: "Clear" },
  { aid: "pf1", label: "PF1" },
  { aid: "pf2", label: "PF2" },
  { aid: "pf3", label: "PF3" },
  { aid: "pf4", label: "PF4" },
  { aid: "pf5", label: "PF5" },
  { aid: "pf6", label: "PF6" },
  { aid: "pf7", label: "PF7" },
  { aid: "pf8", label: "PF8" },
  { aid: "pf9", label: "PF9" },
  { aid: "pf10", label: "PF10" },
  { aid: "pf11", label: "PF11" },
  { aid: "pf12", label: "PF12" },
];

/// Finds the start of the field that owns `addr` — mirrors the backend's
/// backward-scan-with-wraparound `field_attr_for`, since fields wrap
/// circularly around the buffer just like the real 3270 semantics.
function fieldStartFor(screen: Tn3270ScreenData, addr: number): number | null {
  const n = screen.cells.length;
  for (let step = 0; step < n; step++) {
    const a = (addr - step + n) % n;
    if (screen.cells[a].field_start) return a;
  }
  return null;
}

export default function Tn3270Screen({ sessionId, config }: Tn3270ScreenProps) {
  const { toast } = useToast();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const connectionIdRef = useRef<string | null>(null);
  const [status, setStatus] = useState<"connecting" | "connected" | "error">("connecting");
  const [error, setError] = useState<string | null>(null);
  const [screen, setScreen] = useState<Tn3270ScreenData | null>(null);
  const [selectedAddr, setSelectedAddr] = useState<number | null>(null);
  const [fieldText, setFieldText] = useState("");

  useEffect(() => {
    let cancelled = false;
    setStatus("connecting");
    setError(null);

    invoke<string>("tn3270_connect", { config })
      .then((id) => {
        if (cancelled) {
          invoke("tn3270_disconnect", { id }).catch(() => {});
          return;
        }
        connectionIdRef.current = id;
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
      const id = connectionIdRef.current;
      if (id) invoke("tn3270_disconnect", { id }).catch(() => {});
      connectionIdRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId, config]);

  useEffect(() => {
    if (!connectionId) return;
    const unlisten = listen<Tn3270ScreenData>("tn3270:screen", (event) => {
      if (event.payload.session_id === connectionId) {
        setScreen(event.payload);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [connectionId]);

  const rows = useMemo(() => {
    if (!screen) return [];
    const out: { ch: string; addr: number; protected: boolean; fieldStart: boolean }[][] = [];
    for (let r = 0; r < screen.rows; r++) {
      const row = [];
      for (let c = 0; c < screen.cols; c++) {
        const addr = r * screen.cols + c;
        const cell = screen.cells[addr];
        row.push({ ch: cell.field_start ? " " : cell.ch, addr, protected: cell.protected, fieldStart: cell.field_start });
      }
      out.push(row);
    }
    return out;
  }, [screen]);

  const handleCellClick = useCallback(
    (addr: number) => {
      if (!screen || screen.cells[addr].protected) return;
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
    try {
      const updated = await invoke<Tn3270ScreenData>("tn3270_type", { id: connectionId, addr: dataAddr, text: fieldText });
      setScreen(updated);
    } catch (e) {
      toast("error", `Type failed: ${String(e)}`);
    }
  }, [connectionId, selectedAddr, fieldText, screen, toast]);

  const sendAid = useCallback(
    async (aid: Tn3270Aid) => {
      if (!connectionId) return;
      try {
        await invoke("tn3270_aid", { id: connectionId, aid });
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
        data-testid={`tn3270-grid-${sessionId}`}
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
                    : cell.protected
                      ? "text-text-disabled"
                      : "text-status-connected cursor-text hover:bg-surface-elevated"
                }
              >
                {cell.ch === " " ? " " : cell.ch}
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
          placeholder={selectedAddr === null ? "Click an unprotected field to edit it" : "Field value"}
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
