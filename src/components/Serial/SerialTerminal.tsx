import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import clsx from "clsx";
import {
  Loader2,
  Plug,
  PlugZap,
  RefreshCw,
  Usb,
} from "lucide-react";
import type { SerialConfig, SerialPortInfo } from "@/types";

const BAUD_RATES = [300, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200];

export default function SerialTerminal() {
  const { t } = useTranslation();
  const [ports, setPorts] = useState<SerialPortInfo[]>([]);
  const [connectionId, setConnectionId] = useState<string | null>(null);
  const [output, setOutput] = useState("");
  const [inputValue, setInputValue] = useState("");
  const outputRef = useRef<HTMLPreElement>(null);

  // Config form state
  const [selectedPort, setSelectedPort] = useState("");
  const [baudRate, setBaudRate] = useState(9600);
  const [dataBits, setDataBits] = useState<SerialConfig["data_bits"]>("eight");
  const [stopBits, setStopBits] = useState<SerialConfig["stop_bits"]>("one");
  const [parity, setParity] = useState<SerialConfig["parity"]>("none");
  const [flowControl, setFlowControl] = useState<SerialConfig["flow_control"]>("none");
  const [connecting, setConnecting] = useState(false);

  const loadPorts = useCallback(async () => {
    try {
      const result = await invoke<SerialPortInfo[]>("serial_list_ports");
      setPorts(result);
      if (result.length > 0 && !selectedPort) {
        setSelectedPort(result[0].name);
      }
    } catch {
      setPorts([]);
    }
  }, [selectedPort]);

  useEffect(() => {
    loadPorts();
  }, [loadPorts]);

  // Listen for serial data
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    if (connectionId) {
      listen<{ conn_id: string; data: number[] }>("serial:data", (event) => {
        if (event.payload.conn_id === connectionId) {
          const text = new TextDecoder().decode(new Uint8Array(event.payload.data));
          setOutput((prev) => prev + text);
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
    if (!selectedPort) return;
    setConnecting(true);
    try {
      const config: SerialConfig = {
        port_name: selectedPort,
        baud_rate: baudRate,
        data_bits: dataBits,
        stop_bits: stopBits,
        parity,
        flow_control: flowControl,
      };
      const connId = await invoke<string>("serial_connect", { config });
      setConnectionId(connId);
      setOutput("");
    } catch {
      // Connection failed
    } finally {
      setConnecting(false);
    }
  }, [selectedPort, baudRate, dataBits, stopBits, parity, flowControl]);

  const disconnect = useCallback(async () => {
    if (!connectionId) return;
    try {
      await invoke("serial_disconnect", { connId: connectionId });
    } catch {
      // Disconnect failed
    } finally {
      setConnectionId(null);
    }
  }, [connectionId]);

  const sendData = useCallback(async () => {
    if (!connectionId || !inputValue) return;
    try {
      const encoder = new TextEncoder();
      const data = Array.from(encoder.encode(inputValue + "\r\n"));
      await invoke("serial_write", { connId: connectionId, data });
      setInputValue("");
    } catch {
      // Write failed
    }
  }, [connectionId, inputValue]);

  const handleBaudChange = useCallback(
    async (newBaud: number) => {
      setBaudRate(newBaud);
      if (connectionId) {
        try {
          await invoke("serial_set_baud", { connId: connectionId, baudRate: newBaud });
        } catch {
          // Baud change failed
        }
      }
    },
    [connectionId]
  );

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
              <Usb size={20} />
              <span className="text-sm">{t("serial.title")}</span>
            </div>
            <p className="text-xs text-text-secondary">
              {t("serial.notConnected")}
            </p>

            <div className="flex items-center gap-2">
              <div className="flex flex-1 flex-col gap-1">
                <label className="text-xs font-medium text-text-secondary">
                  {t("serial.port")}
                </label>
                <select
                  value={selectedPort}
                  onChange={(e) => setSelectedPort(e.target.value)}
                  className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
                >
                  {ports.map((p) => (
                    <option key={p.name} value={p.name}>
                      {p.name}
                      {p.description ? ` — ${p.description}` : ""}
                    </option>
                  ))}
                  {ports.length === 0 && (
                    <option disabled>{t("serial.noPortsFound")}</option>
                  )}
                </select>
              </div>
              <button
                onClick={loadPorts}
                className="mt-4 rounded p-1.5 text-text-secondary hover:bg-surface-secondary"
              >
                <RefreshCw size={14} />
              </button>
            </div>

            <div className="grid grid-cols-2 gap-2">
              <div className="flex flex-col gap-1">
                <label className="text-xs font-medium text-text-secondary">
                  {t("serial.baudRate")}
                </label>
                <select
                  value={baudRate}
                  onChange={(e) => setBaudRate(Number(e.target.value))}
                  className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
                >
                  {BAUD_RATES.map((b) => (
                    <option key={b} value={b}>
                      {b}
                    </option>
                  ))}
                </select>
              </div>
              <div className="flex flex-col gap-1">
                <label className="text-xs font-medium text-text-secondary">
                  {t("serial.dataBits")}
                </label>
                <select
                  value={dataBits}
                  onChange={(e) =>
                    setDataBits(e.target.value as SerialConfig["data_bits"])
                  }
                  className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
                >
                  <option value="five">5</option>
                  <option value="six">6</option>
                  <option value="seven">7</option>
                  <option value="eight">8</option>
                </select>
              </div>
              <div className="flex flex-col gap-1">
                <label className="text-xs font-medium text-text-secondary">
                  {t("serial.stopBits")}
                </label>
                <select
                  value={stopBits}
                  onChange={(e) =>
                    setStopBits(e.target.value as SerialConfig["stop_bits"])
                  }
                  className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
                >
                  <option value="one">1</option>
                  <option value="two">2</option>
                </select>
              </div>
              <div className="flex flex-col gap-1">
                <label className="text-xs font-medium text-text-secondary">
                  {t("serial.parity")}
                </label>
                <select
                  value={parity}
                  onChange={(e) =>
                    setParity(e.target.value as SerialConfig["parity"])
                  }
                  className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
                >
                  <option value="none">None</option>
                  <option value="odd">Odd</option>
                  <option value="even">Even</option>
                </select>
              </div>
            </div>

            <div className="flex flex-col gap-1">
              <label className="text-xs font-medium text-text-secondary">
                {t("serial.flowControl")}
              </label>
              <select
                value={flowControl}
                onChange={(e) =>
                  setFlowControl(e.target.value as SerialConfig["flow_control"])
                }
                className="rounded border border-border-default bg-surface-secondary px-2 py-1.5 text-xs text-text-primary"
              >
                <option value="none">None</option>
                <option value="software">Software (XON/XOFF)</option>
                <option value="hardware">Hardware (RTS/CTS)</option>
              </select>
            </div>

            <button
              onClick={connect}
              disabled={!selectedPort}
              className={clsx(
                "rounded px-3 py-1.5 text-xs font-medium",
                selectedPort
                  ? "bg-accent-primary text-text-inverse hover:bg-interactive-hover"
                  : "bg-interactive-disabled text-text-disabled"
              )}
            >
              <span className="flex items-center justify-center gap-1">
                <Plug size={14} />
                {t("serial.connect")}
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
        <PlugZap size={14} className="text-status-connected" />
        <span className="text-xs font-medium">{selectedPort}</span>
        <span className="text-xs text-text-secondary">@ {baudRate} baud</span>

        <select
          value={baudRate}
          onChange={(e) => handleBaudChange(Number(e.target.value))}
          className="ml-2 rounded border border-border-default bg-surface-secondary px-1 py-0.5 text-xs"
        >
          {BAUD_RATES.map((b) => (
            <option key={b} value={b}>
              {b}
            </option>
          ))}
        </select>

        <button
          onClick={disconnect}
          className="ml-auto flex items-center gap-1 rounded bg-status-disconnected/20 px-2 py-1 text-xs text-status-disconnected hover:bg-status-disconnected/30"
        >
          {t("serial.disconnect")}
        </button>
      </div>

      {/* Output */}
      <pre
        ref={outputRef}
        className="flex-1 overflow-auto whitespace-pre-wrap p-3 font-mono text-xs text-text-primary"
      >
        {output || t("serial.notConnected")}
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
