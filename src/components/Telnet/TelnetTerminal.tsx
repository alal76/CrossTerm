import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import clsx from "clsx";
import {
  Loader2,
  LogIn,
  Monitor,
} from "lucide-react";
import type { TelnetConfig } from "@/types";

export default function TelnetTerminal() {
  const { t } = useTranslation();
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [output, setOutput] = useState("");
  const [inputValue, setInputValue] = useState("");
  const outputRef = useRef<HTMLPreElement>(null);

  // Connection form state
  const [host, setHost] = useState("");
  const [port, setPort] = useState(23);
  const [terminalType, setTerminalType] = useState("xterm-256color");
  const [connecting, setConnecting] = useState(false);

  // Listen for telnet data
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    if (connectionId) {
      listen<{ conn_id: string; data: string }>("telnet:data", (event) => {
        if (event.payload.conn_id === connectionId) {
          setOutput((prev) => prev + event.payload.data);
        }
      }).then((fn) => {
        unlisten = fn;
      });
    }
    return () => {
      unlisten?.();
    };
  }, [connectionId]);

  // Auto-scroll
  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [output]);

  const connect = useCallback(async () => {
    if (!host) return;
    setConnecting(true);
    try {
      const config: TelnetConfig = {
        host,
        port,
        terminal_type: terminalType,
      };
      const connId = await invoke<string>("telnet_connect", { config });
      setConnectionId(connId);
      setOutput("");
    } catch {
      // Connection failed
    } finally {
      setConnecting(false);
    }
  }, [host, port, terminalType]);

  const disconnect = useCallback(async () => {
    if (!connectionId) return;
    try {
      await invoke("telnet_disconnect", { connId: connectionId });
    } catch {
      // Disconnect failed
    } finally {
      setConnectionId(null);
    }
  }, [connectionId]);

  const sendData = useCallback(async () => {
    if (!connectionId || !inputValue) return;
    try {
      await invoke("telnet_write", { connId: connectionId, data: inputValue + "\r\n" });
      setInputValue("");
    } catch {
      // Write failed
    }
  }, [connectionId, inputValue]);

  if (!connectionId) {
    return (
      <div className="flex h-full flex-col">
        {connecting ? (
          <div className="flex items-center justify-center py-16">
            <Loader2 size={24} className="animate-spin text-text-disabled" />
          </div>
        ) : (
          <div className="mx-auto flex w-full max-w-md flex-col gap-3 p-6">
            <div className="flex items-center gap-2 text-text-disabled">
              <Monitor size={20} />
              <span className="text-sm">{t("telnet.title")}</span>
            </div>
            <p className="text-xs text-text-secondary">
              {t("telnet.notConnected")}
            </p>

            <div className="flex flex-col gap-2">
              <label className="text-xs font-medium text-text-secondary">
                {t("telnet.host")}
              </label>
              <input
                type="text"
                value={host}
                onChange={(e) => setHost(e.target.value)}
                placeholder="telnet.example.com"
                className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
              />
            </div>

            <div className="flex gap-2">
              <div className="flex flex-1 flex-col gap-2">
                <label className="text-xs font-medium text-text-secondary">
                  {t("telnet.port")}
                </label>
                <input
                  type="number"
                  value={port}
                  onChange={(e) => setPort(Number(e.target.value))}
                  className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
                />
              </div>
              <div className="flex flex-1 flex-col gap-2">
                <label className="text-xs font-medium text-text-secondary">
                  {t("telnet.terminalType")}
                </label>
                <input
                  type="text"
                  value={terminalType}
                  onChange={(e) => setTerminalType(e.target.value)}
                  className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
                />
              </div>
            </div>

            <button
              onClick={connect}
              disabled={!host}
              className={clsx(
                "rounded px-3 py-1.5 text-xs font-medium",
                host
                  ? "bg-accent-primary text-text-inverse hover:bg-interactive-hover"
                  : "bg-interactive-disabled text-text-disabled"
              )}
            >
              <span className="flex items-center justify-center gap-1">
                <LogIn size={14} />
                {t("telnet.connect")}
              </span>
            </button>
          </div>
        )}
      </div>
    );
  }

  // Connected — show terminal
  return (
    <div className="flex h-full flex-col bg-surface-sunken">
      {/* Toolbar */}
      <div className="flex items-center gap-2 border-b border-border-default bg-surface-primary px-3 py-2">
        <Monitor size={14} className="text-status-connected" />
        <span className="text-xs font-medium">
          {host}:{port}
        </span>
        <button
          onClick={disconnect}
          className="ml-auto flex items-center gap-1 rounded bg-status-disconnected/20 px-2 py-1 text-xs text-status-disconnected hover:bg-status-disconnected/30"
        >
          {t("telnet.disconnect")}
        </button>
      </div>

      {/* Output */}
      <pre
        ref={outputRef}
        className="flex-1 overflow-auto whitespace-pre-wrap p-3 font-mono text-xs text-text-primary"
      >
        {output || t("telnet.notConnected")}
      </pre>

      {/* Input */}
      <div className="flex items-center gap-2 border-t border-border-default bg-surface-primary px-3 py-2">
        <input
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") sendData();
          }}
          placeholder="Type command..."
          className="flex-1 rounded border border-border-default bg-surface-secondary px-2 py-1 font-mono text-xs text-text-primary"
        />
        <button
          onClick={sendData}
          disabled={!inputValue}
          className="rounded bg-accent-primary px-2 py-1 text-xs text-text-inverse hover:bg-interactive-hover disabled:opacity-50"
        >
          Send
        </button>
      </div>
    </div>
  );
}
