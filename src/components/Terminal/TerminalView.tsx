import { useEffect, useRef, useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import { useTerminalStore } from "@/stores/terminalStore";
import { useAppStore } from "@/stores/appStore";
import { ConnectionStatus } from "@/types";
import TerminalSearch from "@/components/Terminal/TerminalSearch";
import "@xterm/xterm/css/xterm.css";

interface TerminalViewProps {
  readonly terminalId: string;
  readonly isActive: boolean;
}

function getCSSVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function getTerminalTheme(): Record<string, string> {
  return {
    foreground: getCSSVar("--terminal-fg"),
    background: getCSSVar("--terminal-bg"),
    cursor: getCSSVar("--terminal-cursor"),
    selectionBackground: getCSSVar("--terminal-selection"),
    black: getCSSVar("--terminal-ansi-0"),
    red: getCSSVar("--terminal-ansi-1"),
    green: getCSSVar("--terminal-ansi-2"),
    yellow: getCSSVar("--terminal-ansi-3"),
    blue: getCSSVar("--terminal-ansi-4"),
    magenta: getCSSVar("--terminal-ansi-5"),
    cyan: getCSSVar("--terminal-ansi-6"),
    white: getCSSVar("--terminal-ansi-7"),
    brightBlack: getCSSVar("--terminal-ansi-8"),
    brightRed: getCSSVar("--terminal-ansi-9"),
    brightGreen: getCSSVar("--terminal-ansi-10"),
    brightYellow: getCSSVar("--terminal-ansi-11"),
    brightBlue: getCSSVar("--terminal-ansi-12"),
    brightMagenta: getCSSVar("--terminal-ansi-13"),
    brightCyan: getCSSVar("--terminal-ansi-14"),
    brightWhite: getCSSVar("--terminal-ansi-15"),
  };
}

export default function TerminalView({ terminalId, isActive }: TerminalViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const searchAddonRef = useRef<SearchAddon | null>(null);
  const updateTerminalDimensions = useTerminalStore((s) => s.updateTerminalDimensions);
  const updateTerminalStatus = useTerminalStore((s) => s.updateTerminalStatus);
  const removeTerminal = useTerminalStore((s) => s.removeTerminal);
  const cursorStyle = useAppStore((s) => s.cursorStyle);
  const cursorBlink = useAppStore((s) => s.cursorBlink);

  const [searchVisible, setSearchVisible] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [pasteConfirm, setPasteConfirm] = useState<{ text: string; lines: string[] } | null>(null);
  const [suppressPasteConfirm, setSuppressPasteConfirm] = useState(false);
  const [dontAskAgain, setDontAskAgain] = useState(false);
  const [bellFlash, setBellFlash] = useState(false);
  const { t } = useTranslation();

  const handleResize = useCallback(() => {
    const fitAddon = fitAddonRef.current;
    if (!fitAddon) return;
    try {
      fitAddon.fit();
      const term = termRef.current;
      if (term) {
        const { cols, rows } = term;
        updateTerminalDimensions(terminalId, cols, rows);
        invoke("terminal_resize", { id: terminalId, rows, cols }).catch(() => {});
      }
    } catch {
      // fit may fail if container has zero size
    }
  }, [terminalId, updateTerminalDimensions]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const theme = getTerminalTheme();

    const term = new Terminal({
      cursorBlink,
      cursorStyle,
      cursorStyle,
      allowProposedApi: true,
      theme,
    });

    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    const webLinksAddon = new WebLinksAddon((_event, uri) => {
      shellOpen(uri).catch(() => {});
    }
    });

    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);
    term.loadAddon(webLinksAddon);

    term.open(container);

    // Try loading WebGL renderer
    try {
      const webglAddon = new WebglAddon();
      webglAddon.onContextLoss(() => {
        webglAddon.dispose();
      });
      term.loadAddon(webglAddon);
    } catch {
      // WebGL not available, fall back to canvas
    }

    fitAddon.fit();
    termRef.current = term;
    fitAddonRef.current = fitAddon;
    searchAddonRef.current = searchAddon;

    // Send initial dimensions
    const { cols, rows } = term;
    updateTerminalDimensions(terminalId, cols, rows);
    invoke("terminal_resize", { id: terminalId, rows, cols }).catch(() => {});

    // Forward user input to backend
    const dataDisposable = term.onData((data) => {
      invoke("terminal_write", { id: terminalId, data }).catch(() => {});
    });

    // Listen for output from backend
    const outputUnlisten = listen<{ terminal_id: string; data: string }>(
      "terminal:output",
      (event) => {
        if (event.payload.terminal_id === terminalId) {
          term.write(event.payload.data);
        }
      }
    );

    // Listen for terminal bell
    const bellDisposable = term.onBell(() => {
      const currentBellStyle = useAppStore.getState().bellStyle;
      if (currentBellStyle === "visual") {
        setBellFlash(true);
        setTimeout(() => setBellFlash(false), 200);
      } else if (currentBellStyle === "audio") {
        const audioCtx = new AudioContext();
        const osc = audioCtx.createOscillator();
        const gain = audioCtx.createGain();
        osc.connect(gain);
        gain.connect(audioCtx.destination);
        osc.frequency.value = 800;
        gain.gain.value = 0.1;
       Bell event handler
    const bellDisposable = term.onBell(() => {
      if (bellStyle === "visual") {
        setBellFlash(true);
        setTimeout(() => setBellFlash(false), 200);
      } else if (bellStyle === "audio") {
        const ctx = new AudioContext();
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.connect(gain);
        gain.connect(ctx.destination);
        osc.frequency.value = 800;
        gain.gain.value = 0.1;
        osc.start();
        osc.stop(ctx.currentTime + 0.08);
      }
    });

    //  osc.start();
        osc.stop(audioCtx.currentTime + 0.1);
      }
    });

    // Listen for terminal exit
    const exitUnlisten = listen<{ terminal_id: string; exit_code: number }>(
      "terminal:exit",
      (event) => {
        if (event.payload.terminal_id === terminalId) {
          term.write(`\r\n\x1b[90m[Process exited with code ${event.payload.exit_code}]\x1b[0m\r\n`);
          updateTerminalStatus(terminalId, ConnectionStatus.Disconnected);
        }
      }
    );

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        try {
          fitAddon.fit();
          const { cols, rows } = term;
          updateTerminalDimensions(terminalId, cols, rows);
          invoke("terminal_resize", { id: terminalId, rows, cols }).catch(() => {});
        } catch {
          // ignore
        }
      });
    });
    resizeObserver.observe(container);

    return () => {
      resizeObserver.disconnect();
      dataDisposable.dispose();
      bellDisposable.dispose();
      outputUnlisten.then((fn) => fn());
      exitUnlisten.then((fn) => fn());
      term.dispose();
      termRef.current = null;
      fitAddonRef.current = null;
      searchAddonRef.current = null;
    };
  }, [terminalId, updateTerminalDimensions, updateTerminalStatus, removeTerminal, cursorStyle, cursorBlink]);

  // Re-fit when tab becomes active
  useEffect(() => {
    if (isActive) {
      requestAnimationFrame(() => {
        handleResize();
        termRef.current?.focus();
      });
    }
  }, [isActive, handleResize]);

  // Intercept paste events for multi-line confirmation
  const handlePasteWithConfirmation = useCallback(
    (text: string) => {
      const lines = text.split("\n");
      if (lines.length > 1 && !suppressPasteConfirm) {
        setPasteConfirm({ text, lines });
      } else {
        invoke("terminal_write", { id: terminalId, data: text }).catch(() => {});
      }
    },
    [terminalId, suppressPasteConfirm],
  );

  useEffect(() => {
    if (!isActive) return;
    function onPaste(e: ClipboardEvent) {
      // Only intercept if focus is within our container
      const container = containerRef.current;
      if (!container) return;
      if (!container.contains(document.activeElement) && !container.contains(e.target as Node)) return;
      const text = e.clipboardData?.getData("text/plain");
      if (text && text.includes("\n")) {
        e.preventDefault();
        handlePasteWithConfirmation(text);
      }
    }
    globalThis.addEventListener("paste", onPaste, true);
    return () => globalThis.removeEventListener("paste", onPaste, true);
  }, [isActive, handlePasteWithConfirmation]);

  // Ctrl+Shift+F to toggle search
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === "f" && isActive) {
        e.preventDefault();
        setSearchVisible((v) => !v);
      }
    }
    globalThis.addEventListener("keydown", onKeyDown);
    return () => globalThis.removeEventListener("keydown", onKeyDown);
  }, [isActive]);

  // Dismiss context menu on click anywhere
  useEffect(() => {
    if (!contextMenu) return;
    function dismiss() {
      setContextMenu(null);
    }
    globalThis.addEventListener("click", dismiss);
    return () => globalThis.removeEventListener("click", dismiss);
  }, [contextMenu]);

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      setContextMenu({ x: e.clientX, y: e.clientY });
    },
    [],
  );

  const getSelectedText = useCallback(() => {
    return termRef.current?.getSelection() ?? "";
  }, []);

  const contextActions = [
    {
      label: t("actions.copy"),
      handler: () => {
        const text = getSelectedText();
        if (text) navigator.clipboard.writeText(text);
        setContextMenu(null);
      },
    },
    {
      label: t("actions.paste"),
      handler: async () => {
        const text = await navigator.clipboard.readText();
        if (text) handlePasteWithConfirmation(text);
        setContextMenu(null);
      },
    },
    {
      label: t("actions.selectAll"),
      handler: () => {
        termRef.current?.selectAll();
        setContextMenu(null);
      },
    },
    {
      label: t("terminal.clearTerminal"),
      handler: () => {
        termRef.current?.clear();
        setContextMenu(null);
      },
    },
    {
      label: t("terminal.search"),
      handler: () => {
        setSearchVisible(true);
        setContextMenu(null);
      },
    },
  ];

  return (
    <div className="relative w-full h-full">
      {/* Visual bell flash */}
      {bellFlash && (
        <div className="absolute inset-0 z-20 bg-text-primary/10 pointer-events-none animate-bell-flash" />
      )}
      <TerminalSearch
        searchAddon={searchAddonRef.current}
        visible={searchVisible}
        onClose={() => setSearchVisible(false)}
      />

      {/* Multi-line paste confirmation dialog */}
      {pasteConfirm && (
        <div className="fixed inset-0 z-[9000] flex items-center justify-center bg-black/50">
          <div className="bg-surface-elevated border border-border-default rounded-xl shadow-[var(--shadow-3)] w-[420px] max-w-[90vw] overflow-hidden">
            <div className="px-4 pt-4 pb-2">
              <h3 className="text-sm font-semibold text-text-primary mb-1">
                {t("pasteConfirm.title", { count: pasteConfirm.lines.length })}
              </h3>
              <p className="text-xs text-text-secondary">
                {t("pasteConfirm.warning")}
              </p>
            </div>
            <div className="mx-4 mb-3 rounded-lg bg-surface-sunken border border-border-subtle p-2 max-h-[120px] overflow-y-auto">
              <pre className="text-[11px] text-text-secondary font-mono whitespace-pre-wrap break-all">
                {pasteConfirm.lines.slice(0, 5).join("\n")}
                {pasteConfirm.lines.length > 5 && (
                  <span className="text-text-disabled">
                    {"\n"}{t("pasteConfirm.moreLines", { count: pasteConfirm.lines.length - 5 })}
                  </span>
                )}
              </pre>
            </div>
            <div className="px-4 pb-3">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={dontAskAgain}
                  onChange={(e) => setDontAskAgain(e.target.checked)}
                  className="rounded border-border-default"
                />
                <span className="text-xs text-text-secondary">
                  {t("pasteConfirm.dontAskAgain")}
                </span>
              </label>
            </div>
            <div className="flex justify-end gap-2 px-4 pb-4">
              <button
                onClick={() => {
                  setPasteConfirm(null);
                  setDontAskAgain(false);
                  termRef.current?.focus();
                }}
                className="px-3 py-1.5 text-xs rounded-lg border border-border-default hover:bg-surface-secondary text-text-secondary transition-colors"
              >
                {t("actions.cancel")}
              </button>
              <button
                onClick={() => {
                  invoke("terminal_write", { id: terminalId, data: pasteConfirm.text }).catch(() => {});
                  if (dontAskAgain) setSuppressPasteConfirm(true);
                  setPasteConfirm(null);
                  setDontAskAgain(false);
                  termRef.current?.focus();
                }}
                className="px-3 py-1.5 text-xs rounded-lg bg-interactive-default hover:bg-interactive-hover text-text-inverse transition-colors"
              >
                {t("actions.paste")}
              </button>
            </div>
          </div>
        </div>
      )}

      {contextMenu && (
        <div
          className="fixed z-50 bg-surface-elevated border border-border-default rounded-md shadow-2 py-1 min-w-[140px] animate-fade-in"
          style={{ left: contextMenu.x, top: contextMenu.y }}
        >
          {contextActions.map((action) => (
            <button
              key={action.label}
              onClick={action.handler}
              className="w-full text-left px-3 py-1.5 text-xs text-text-primary hover:bg-surface-primary transition-colors"
            >
              {action.label}
            </button>
          ))}
        </div>
      )}

      {/* eslint-disable-next-line jsx-a11y/no-static-element-interactions */}
      <div
        ref={containerRef}
        className="w-full h-full bg-[var(--terminal-bg)]"
        style={{ padding: "4px 0 0 4px" }}
        onContextMenu={handleContextMenu}
      />
    </div>
  );
}
