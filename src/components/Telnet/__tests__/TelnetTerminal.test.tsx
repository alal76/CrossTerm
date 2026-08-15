import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import TelnetTerminal from "@/components/Telnet/TelnetTerminal";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

describe("TelnetTerminal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("renders a connect form with no live connection until the user connects", () => {
    render(<TelnetTerminal />);
    expect(screen.getByPlaceholderText("telnet.example.com")).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("connects with the entered host/port/terminal type and shows the connected view", async () => {
    mockInvoke.mockResolvedValue("conn-1");

    render(<TelnetTerminal />);
    fireEvent.change(screen.getByPlaceholderText("telnet.example.com"), { target: { value: "bbs.example.org" } });

    const connectButton = screen.getByRole("button");
    fireEvent.click(connectButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("telnet_connect", {
        config: { host: "bbs.example.org", port: 23, terminal_type: "xterm-256color" },
      });
    });
    expect(await screen.findByText("bbs.example.org:23")).toBeInTheDocument();
  });

  it("appends incoming telnet:data for the active connection to the output", async () => {
    mockInvoke.mockResolvedValue("conn-1");

    render(<TelnetTerminal />);
    fireEvent.change(screen.getByPlaceholderText("telnet.example.com"), { target: { value: "bbs.example.org" } });
    fireEvent.click(screen.getByRole("button"));
    await screen.findByText("bbs.example.org:23");

    handlers["telnet:data"]({ payload: { conn_id: "conn-1", data: "Welcome!" } });

    expect(await screen.findByText("Welcome!")).toBeInTheDocument();
  });

  it("disconnects and returns to the connect form", async () => {
    mockInvoke.mockResolvedValue("conn-1");

    render(<TelnetTerminal />);
    fireEvent.change(screen.getByPlaceholderText("telnet.example.com"), { target: { value: "bbs.example.org" } });
    fireEvent.click(screen.getByRole("button"));
    await screen.findByText("bbs.example.org:23");

    fireEvent.click(screen.getByRole("button", { name: "telnet.disconnect" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("telnet_disconnect", { connId: "conn-1" });
    });
    expect(await screen.findByPlaceholderText("telnet.example.com")).toBeInTheDocument();
  });
});
