import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import X11ForwardPanel from "@/components/X11Forward/X11ForwardPanel";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { X11ForwardConfig } from "@/types";

Element.prototype.scrollIntoView = vi.fn();

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

const config: X11ForwardConfig = {
  host: "10.0.0.90",
  port: 22,
  username: "alal",
  auth: { type: "password", password: "hunter2" },
  remote_command: "xterm",
  local_display: "0",
};

describe("X11ForwardPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects and shows the remote command and display", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "x11_forward_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<X11ForwardPanel sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/X11 Forward — 10.0.0.90/)).toBeInTheDocument();
    expect(screen.getByText(/running “xterm” on display :0/)).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("x11_forward_connect", { config });
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "x11_forward_connect") return Promise.reject(new Error("Authentication failed"));
      return Promise.resolve(undefined);
    });

    render(<X11ForwardPanel sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Couldn't forward X11/)).toBeInTheDocument();
    expect(screen.getByText(/Authentication failed/)).toBeInTheDocument();
  });

  it("appends incoming output and error events to the log", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "x11_forward_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<X11ForwardPanel sessionId="sess-1" config={config} />);
    await screen.findByText(/X11 Forward/);

    handlers["x11_forward:output"]({ payload: { session_id: "conn-1", data: "remote stdout line" } });
    handlers["x11_forward:error"]({ payload: { session_id: "conn-1", data: "sub-connection failed" } });

    expect(await screen.findByText("remote stdout line")).toBeInTheDocument();
    expect(await screen.findByText(/\[x11\] sub-connection failed/)).toBeInTheDocument();
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "x11_forward_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<X11ForwardPanel sessionId="sess-1" config={config} />);
    await screen.findByText(/X11 Forward/);
    unmount();

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("x11_forward_disconnect", { id: "conn-1" });
    });
  });
});
