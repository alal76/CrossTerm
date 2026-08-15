import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import IpmiSolTab from "@/components/Ipmi/IpmiSolTab";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { IpmiConfig, IpmiPowerStatus } from "@/types";

const mockTermWrite = vi.fn();
const mockTermOnData = vi.fn((_cb: (data: string) => void) => ({ dispose: vi.fn() }));
const mockTermOpen = vi.fn();
const mockTermDispose = vi.fn();
vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn().mockImplementation(function (this: Record<string, unknown>) {
    this.loadAddon = vi.fn();
    this.open = mockTermOpen;
    this.onData = mockTermOnData;
    this.write = mockTermWrite;
    this.dispose = mockTermDispose;
    this.focus = vi.fn();
  }),
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn().mockImplementation(function (this: Record<string, unknown>) {
    this.fit = vi.fn();
  }),
}));
vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: vi.fn().mockImplementation(function () {}),
}));

class MockResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: IpmiConfig = { host: "10.0.0.10", port: 623, username: "admin", password: "hunter2", channel: 1, privilege: "administrator" };
const powerOn: IpmiPowerStatus = { session_id: "conn-1", powered_on: true };

describe("IpmiSolTab", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects, mounts the terminal, and shows power status", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ipmi_sol_connect") return Promise.resolve("conn-1");
      if (cmd === "ipmi_power_status") return Promise.resolve(powerOn);
      return Promise.resolve(undefined);
    });

    renderWithToast(<IpmiSolTab sessionId="sess-1" config={config} />);

    expect(await screen.findByTestId("ipmi-sol-container-sess-1")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("ipmi_sol_connect", { config });
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());
    expect(await screen.findByText("powered on")).toBeInTheDocument();
  });

  it("shows an error state when the RAKP+ handshake fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ipmi_sol_connect") return Promise.reject(new Error("Authentication failed"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<IpmiSolTab sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Failed to connect/)).toBeInTheDocument();
    expect(screen.getByText(/Authentication failed/)).toBeInTheDocument();
  });

  it("forwards keystrokes to ipmi_sol_send and incoming ipmi:sol_data to the terminal", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ipmi_sol_connect") return Promise.resolve("conn-1");
      if (cmd === "ipmi_power_status") return Promise.resolve(powerOn);
      return Promise.resolve(undefined);
    });

    renderWithToast(<IpmiSolTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOnData).toHaveBeenCalled());

    const onDataCallback = mockTermOnData.mock.calls[0][0] as (data: string) => void;
    onDataCallback("boot\n");
    expect(mockInvoke).toHaveBeenCalledWith("ipmi_sol_send", { id: "conn-1", data: "boot\n" });

    handlers["ipmi:sol_data"]({ payload: { session_id: "conn-1", data: "login: " } });
    expect(mockTermWrite).toHaveBeenCalledWith("login: ");
  });

  it("sends a power action and refreshes power status", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ipmi_sol_connect") return Promise.resolve("conn-1");
      if (cmd === "ipmi_power_status") return Promise.resolve(powerOn);
      if (cmd === "ipmi_power_control") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    renderWithToast(<IpmiSolTab sessionId="sess-1" config={config} />);
    await screen.findByText("powered on");

    fireEvent.click(screen.getByTitle("Power On"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("ipmi_power_control", { id: "conn-1", action: "up" });
    });
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ipmi_sol_connect") return Promise.resolve("conn-1");
      if (cmd === "ipmi_power_status") return Promise.resolve(powerOn);
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<IpmiSolTab sessionId="sess-1" config={config} />);
    await waitFor(() => expect(mockTermOpen).toHaveBeenCalled());
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("ipmi_sol_disconnect", { id: "conn-1" });
  });
});
