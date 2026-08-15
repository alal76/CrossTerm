import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Tn5250Screen from "@/components/Tn5250/Tn5250Screen";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Tn5250Config, Tn5250Screen as Tn5250ScreenData, Tn5250CellInfo } from "@/types";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: Tn5250Config = { host: "10.0.0.60", port: 23, ssl: false };

function makeScreen(count = 1920): Tn5250ScreenData {
  const cells: Tn5250CellInfo[] = Array.from({ length: count }, () => ({
    ch: " ",
    bypass: false,
    numeric: false,
    nondisplay: false,
    mandatory: false,
    field_start: false,
  }));
  return { session_id: "conn-1", rows: 24, cols: 80, cursor_row: 0, cursor_col: 0, cells };
}

describe("Tn5250Screen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects and renders the screen grid on tn5250:screen events", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "tn5250_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    renderWithToast(<Tn5250Screen sessionId="sess-1" config={config} />);
    expect(await screen.findByTestId("tn5250-grid-sess-1")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("tn5250_connect", { config });

    const screenData = makeScreen();
    screenData.cells[5] = { ...screenData.cells[5], ch: "X" };
    handlers["tn5250:screen"]({ payload: screenData });

    expect(await screen.findByText("X")).toBeInTheDocument();
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "tn5250_connect") return Promise.reject(new Error("Connection failed"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<Tn5250Screen sessionId="sess-1" config={config} />);
    expect(await screen.findByText(/Failed to connect/)).toBeInTheDocument();
  });

  it("clicking an unprotected field loads its text, and Apply calls tn5250_type", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "tn5250_connect") return Promise.resolve("conn-1");
      if (cmd === "tn5250_type") return Promise.resolve(makeScreen());
      return Promise.resolve(undefined);
    });

    renderWithToast(<Tn5250Screen sessionId="sess-1" config={config} />);
    await screen.findByTestId("tn5250-grid-sess-1");

    const screenData = makeScreen();
    screenData.cells[0] = { ...screenData.cells[0], field_start: true, bypass: false };
    screenData.cells[1] = { ...screenData.cells[1], ch: "A" };
    screenData.cells[2] = { ...screenData.cells[2], ch: "B" };
    handlers["tn5250:screen"]({ payload: screenData });
    await screen.findByText("A");

    fireEvent.click(screen.getByText("A"));

    const input = await screen.findByDisplayValue("AB");
    fireEvent.change(input, { target: { value: "hello" } });
    fireEvent.click(screen.getByText("Apply"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("tn5250_type", { id: "conn-1", row: 0, col: 1, text: "hello" });
    });
  });

  it("sends an AID key on button click", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "tn5250_connect") return Promise.resolve("conn-1");
      if (cmd === "tn5250_aid") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    renderWithToast(<Tn5250Screen sessionId="sess-1" config={config} />);
    await screen.findByTestId("tn5250-grid-sess-1");

    fireEvent.click(screen.getByText("Enter"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("tn5250_aid", { id: "conn-1", aid: "enter" });
    });
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "tn5250_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<Tn5250Screen sessionId="sess-1" config={config} />);
    await screen.findByTestId("tn5250-grid-sess-1");
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("tn5250_disconnect", { id: "conn-1" });
  });
});
