import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import SerialTerminal from "@/components/Serial/SerialTerminal";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SerialPortInfo } from "@/types";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

const ports: SerialPortInfo[] = [{ name: "/dev/ttyUSB0", description: "USB Serial" }];

describe("SerialTerminal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("lists ports on mount and auto-selects the first one", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "serial_list_ports") return Promise.resolve(ports);
      return Promise.resolve(undefined);
    });

    render(<SerialTerminal />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("serial_list_ports");
    });
    expect(await screen.findByRole("option", { name: /\/dev\/ttyUSB0/ })).toBeInTheDocument();
  });

  it("connects with the selected port/baud rate and shows the connected toolbar", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "serial_list_ports") return Promise.resolve(ports);
      if (cmd === "serial_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<SerialTerminal />);
    await screen.findByRole("option", { name: /\/dev\/ttyUSB0/ });

    fireEvent.click(screen.getByRole("button", { name: /serial\.connect/ }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("serial_connect", {
        config: {
          port_name: "/dev/ttyUSB0",
          baud_rate: 9600,
          data_bits: "eight",
          stop_bits: "one",
          parity: "none",
          flow_control: "none",
        },
      });
    });
    expect(await screen.findByText("/dev/ttyUSB0")).toBeInTheDocument();
  });

  it("accepts a custom baud rate not in the preset list before connecting", async () => {
    // Regression: the baud rate field used to be a <select> limited to a
    // fixed preset list, with no way to type a value like 230400 or 921600
    // (common for 3D-printer firmware and ESP32/ESP8266 flashing) that the
    // backend would happily accept.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "serial_list_ports") return Promise.resolve(ports);
      if (cmd === "serial_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<SerialTerminal />);
    await screen.findByRole("option", { name: /\/dev\/ttyUSB0/ });

    const baudInput = screen.getByDisplayValue("9600");
    fireEvent.change(baudInput, { target: { value: "921600" } });

    fireEvent.click(screen.getByRole("button", { name: /serial\.connect/ }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("serial_connect", {
        config: expect.objectContaining({ baud_rate: 921600 }),
      });
    });
  });

  it("only commits a custom baud rate change to the backend on blur, not per keystroke", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "serial_list_ports") return Promise.resolve(ports);
      if (cmd === "serial_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<SerialTerminal />);
    await screen.findByRole("option", { name: /\/dev\/ttyUSB0/ });
    fireEvent.click(screen.getByRole("button", { name: /serial\.connect/ }));
    await screen.findByText("/dev/ttyUSB0");

    const baudInput = screen.getByDisplayValue("9600");
    fireEvent.change(baudInput, { target: { value: "230400" } });
    expect(mockInvoke).not.toHaveBeenCalledWith("serial_set_baud", expect.anything());

    fireEvent.blur(baudInput);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("serial_set_baud", { connId: "conn-1", baudRate: 230400 });
    });
  });

  it("decodes incoming serial:data bytes and appends them to the output", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "serial_list_ports") return Promise.resolve(ports);
      if (cmd === "serial_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<SerialTerminal />);
    await screen.findByRole("option", { name: /\/dev\/ttyUSB0/ });
    fireEvent.click(screen.getByRole("button", { name: /serial\.connect/ }));
    await screen.findByText("/dev/ttyUSB0");

    const bytes = Array.from(new TextEncoder().encode("READY"));
    handlers["serial:data"]({ payload: { conn_id: "conn-1", data: bytes } });

    expect(await screen.findByText("READY")).toBeInTheDocument();
  });

  it("disconnects and returns to the connect form", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "serial_list_ports") return Promise.resolve(ports);
      if (cmd === "serial_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<SerialTerminal />);
    await screen.findByRole("option", { name: /\/dev\/ttyUSB0/ });
    fireEvent.click(screen.getByRole("button", { name: /serial\.connect/ }));
    await screen.findByText("/dev/ttyUSB0");

    fireEvent.click(screen.getByRole("button", { name: /serial\.disconnect/ }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("serial_disconnect", { connId: "conn-1" });
    });
    expect(await screen.findByRole("option", { name: /\/dev\/ttyUSB0/ })).toBeInTheDocument();
  });
});
