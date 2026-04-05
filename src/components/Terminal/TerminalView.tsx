import { useCallback, useEffect, useRef, useState } from "react";
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

function getCssVar(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function getTerminalTheme(): Record<string, string> {
  return {
    foreground: getCssVar("--terminal-fg"),
    background: getCssVar("--terminal-bg"),
    cursor: getCssVar("--terminal-cursor"),
    selectionBackground: getCssVar("--terminal-selection"),
    black: getCssVar("--terminal-ansi-0"),
    red: getCssVar("--terminal-ansi-1"),
    green: getCssVar("--terminal-ansi-2"),
    yellow: getCssVar("--terminal-ansi-3"),
    blue: getCssVar("--terminal-ansi-4"),
    magenta: getCssVar("--terminal-ansi-5"),
    cyan: getCssVar("--terminal-ansi-6"),
    white: getCssVar("--terminal-ansi-7"),
    brightBlack: getCssVar("--terminal-ansi-8"),
    brightRed: getCssVar("--terminal-ansi-9"),
    brightGreen: getCssVar("--terminal-ansi-10"),
    brightYellow: getCssVar("--terminal-ansi-11"),
    brightBlue: getCssVar("--terminal-ansi-12"),
    brightMagenta: getCssVar("--terminal-ansi-13"),
    brightCyan: getCssVar("--terminal-ansi-14"),
    brightWhite: getCssVar("--terminal-ansi-15"),
  };
}

function playBell() {
  const context = new AudioContext();
  const oscillator = context.createOscillator();
  const gain = context.createGain();
  oscillator.connect(gain);
  gain.connect(context.destination);
  oscillator.frequency.value = 800;
  gain.gain.value = 0.08;
  oscillator.start();
  oscillator.stop(context.currentTime + 0.08);
}

export default function TerminalView({ terminalId, isActive }: TerminalViewProps) {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const searchAddonRef = useRef<SearchAddon | null>(null);

  const terminals = useTerminalStore((state) => state.terminals);
  const broadcastMode = useTerminalStore((state) => state.broadcastMode);
  const updateTerminalDimensions = useTerminalStore((state) => state.updateTerminalDimensions);
  const updateTerminalStatus = useTerminalStore((state) => state.updateTerminalStatus);
  const removeTerminal = useTerminalStore((state) => state.removeTerminal);

  const bellStyle = useAppStore((state) => state.bellStyle);
  const cursorStyle = useAppStore((state) => state.cursorStyle);
  const cursorBlink = useAppStore((state) => state.cursorBlink);

  const [searchVisible, setSearchVisible] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const [pasteConfirm, setPasteConfirm] = useState<{ text: string; lines: string[] } | null>(null);
  const [suppressPasteConfirm, setSuppressPasteConfirm] = useState(false);
  const [dontAskAgain, setDontAskAgain] = useState(false);
  const [bellFlash, setBellFlash] = useState(false);

  const flashBell = useCallback(() => {
    setBellFlash(true);
    window.setTimeout(() => setBellFlash(false), 180);
  }, []);

  const emitData = useCallback(
    (data: string) => {
      if (broadcastMode) {
        for (const [id] of terminals.entries()) {
          void invoke("terminal_write", { id, data }).catch(() => {});
        }
        return;
      }
      void invoke("terminal_write", { id: terminalId, data }).catch(() => {});
    },
    [broadcastMode, terminalId, terminals],
  );

  const handleResize = useCallback(() => {
    const fitAddon = fitAddonRef.current;
    const term = termRef.current;
    if (!fitAddon || !term) {
      return;
    }
    try {
      fitAddon.fit();
      const { cols, rows } = term;
      updateTerminalDimensions(terminalId, cols, rows);
      void invoke("terminal_resize", { id: terminalId, rows, cols }).catch(() => {});
    } catch {
      // Ignore zero-size layout races.
    }
  }, [terminalId, updateTerminalDimensions]);

  const handlePasteWithConfirmation = useCallback(
    (text: string) => {
      const lines = text.split("\n");
      if (lines.length > 1 && !suppressPasteConfirm) {
        setPasteConfirm({ text, lines });
        return;
      }
      emitData(text);
    },
    [emitData, suppressPasteConfirm],
  );

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      return;
    }

    const term = new Terminal({
      cursorBlink,
      cursorStyle,
      allowProposedApi: true,
      theme: getTerminalTheme(),
    });

    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    const webLinksAddon = new WebLinksAddon((_event, uri) => {
      shellOpen(uri).catch(() => {});
    });

    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);
    term.loadAddon(webLinksAddon);
    term.open(container);

    try {
      const webglAddon = new WebglAddon();
      webglAddon.onContextLoss(() => webglAddon.dispose());
      term.loadAddon(webglAddon);
    } catch {
      // WebGL is optional.
    }

    fitAddon.fit();
    termRef.current = term;
    fitAddonRef.current = fitAddon;
    searchAddonRef.current = searchAddon;

    updateTerminalStatus(terminalId, ConnectionStatus.Connected);
    handleResize();

    const dataDisposable = term.onData((data) => {
      emitData(data);
    });

    const outputUnlisten = listen<{ terminal_id: string; data: string }>("terminal:output", (event) => {
      if (event.payload.terminal_id === terminalId) {
        term.write(event.payload.data);
      }
    });

    const bellUnlisten = listen<{ terminal_id: string }>("terminal:bell", (event) => {
      if (event.payload.terminal_id !== terminalId) {
        return;
      }
      if (bellStyle === "visual") {
        flashBell();
      } else if (bellStyle === "audio") {
        playBell();
      }
    });

    const exitUnlisten = listen<{ terminal_id: string; code?: number | null }>("terminal:exit", (event) => {
      if (event.payload.terminal_id === terminalId) {
        const exitMsg = t("terminal.processExited", { code: event.payload.code ?? 0 });
        term.write(`\r\n\x1b[90m[${exitMsg}]\x1b[0m\r\n`);
        updateTerminalStatus(terminalId, ConnectionStatus.Disconnected);
      }
    });

    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        handleResize();
      });
    });
    resizeObserver.observe(container);

    return () => {
      resizeObserver.disconnect();
      dataDisposable.dispose();
      void outputUnlisten.then((unlisten) => unlisten());
      void bellUnlisten.then((unlisten) => unlisten());
      void exitUnlisten.then((unlisten) => unlisten());
      term.dispose();
      termRef.current = null;
      fitAddonRef.current = null;
      searchAddonRef.current = null;
      removeTerminal(terminalId);
    };
  }, [bellStyle, cursorBlink, cursorStyle, emitData, flashBell, handleResize, removeTerminal, terminalId, updateTerminalStatus]);

  useEffect(() => {
    if (!isActive) {
      return;
    }
    requestAnimationFrame(() => {
      handleResize();
      termRef.current?.focus();
    });
  }, [handleResize, isActive]);

  useEffect(() => {
    if (!isActive) {
      return;
    }
    const onPaste = (event: ClipboardEvent) => {
      const container = containerRef.current;
      if (!container) {
        return;
      }
      if (!container.contains(document.activeElement) && !container.contains(event.target as Node)) {
        return;
      }
      const text = event.clipboardData?.getData("text/plain");
      if (text && text.includes("\n")) {
        event.preventDefault();
        handlePasteWithConfirmation(text);
      }
    };
    globalThis.addEventListener("paste", onPaste, true);
    return () => globalThis.removeEventListener("paste", onPaste, true);
  }, [handlePasteWithConfirmation, isActive]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.key.toLowerCase() === "f" && isActive) {
        event.preventDefault();
        setSearchVisible((value) => !value);
      }
    };
    globalThis.addEventListener("keydown", onKeyDown);
    return () => globalThis.removeEventListener("keydown", onKeyDown);
  }, [isActive]);

  useEffect(() => {
    if (!contextMenu) {
      return;
    }
    const dismiss = () => setContextMenu(null);
    globalThis.addEventListener("click", dismiss);
    return () => globalThis.removeEventListener("click", dismiss);
  }, [contextMenu]);

  const handleContextMenu = useCallback((event: React.MouseEvent) => {
    event.preventDefault();
    setContextMenu({ x: event.clientX, y: event.clientY });
  }, []);

  const getSelectedText = useCallback(() => termRef.current?.getSelection() ?? "", []);

  const contextActions = [
    {
      label: t("actions.copy"),
      handler: () => {
        const text = getSelectedText();
        if (text) {
          void navigator.clipboard.writeText(text);
        }
        setContextMenu(null);
      },
    },
    {
      label: t("actions.paste"),
      handler: async () => {
        const text = await navigator.clipboard.readText();
        if (text) {
          handlePasteWithConfirmation(text);
        }
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
      {bellFlash ? <div className="absolute inset-0 z-20 bg-text-primary/10 pointer-events-none animate-bell-flash" /> : null}

      <TerminalSearch
        searchAddon={searchAddonRef.current}
        visible={searchVisible}
        onClose={() => setSearchVisible(false)}
      />

      {pasteConfirm ? (
        <div className="fixed inset-0 z-[9000] flex items-center justify-center bg-black/50">
          <div className="bg-surface-elevated border border-border-default rounded-xl shadow-[var(--shadow-3)] w-[420px] max-w-[90vw] overflow-hidden">
            <div className="px-4 pt-4 pb-2">
              <h3 className="text-sm font-semibold text-text-primary mb-1">
                {t("pasteConfirm.title", { count: pasteConfirm.lines.length })}
              </h3>
              <p className="text-xs text-text-secondary">{t("pasteConfirm.warning")}</p>
            </div>
            <div className="mx-4 mb-3 rounded-lg bg-surface-sunken border border-border-subtle p-2 max-h-[120px] overflow-y-auto">
              <pre className="text-[11px] text-text-secondary font-mono whitespace-pre-wrap break-all">
                {pasteConfirm.lines.slice(0, 5).join("\n")}
                {pasteConfirm.lines.length > 5 ? (
                  <span className="text-text-disabled">
                    {"\n"}
                    {t("pasteConfirm.moreLines", { count: pasteConfirm.lines.length - 5 })}
                  </span>
                ) : null}
              </pre>
            </div>
            <div className="px-4 pb-3">
              <label className="flex items-center gap-2 cursor-pointer">
                <input
                  type="checkbox"
                  checked={dontAskAgain}
                  onChange={(event) => setDontAskAgain(event.target.checked)}
                  className="rounded border-border-default"
                />
                <span className="text-xs text-text-secondary">{t("pasteConfirm.dontAskAgain")}</span>
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
                  emitData(pasteConfirm.text);
                  if (dontAskAgain) {
                    setSuppressPasteConfirm(true);
                  }
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
      ) : null}

      {contextMenu ? (
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
      ) : null}

      <div
        ref={containerRef}
        className="w-full h-full bg-[var(--terminal-bg)]"
        style={{ padding: "4px 0 0 4px" }}
        onContextMenu={handleContextMenu}
      />
    </div>
  );
}
