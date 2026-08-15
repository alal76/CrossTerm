import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import Tn3270Screen from "@/components/Tn3270/Tn3270Screen";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Tn3270Config, Tn3270Screen as Tn3270ScreenData, Tn3270CellInfo } from "@/types";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: Tn3270Config = { host: "10.0.0.50", port: 23, model: "model2" };

function makeScreen(overrides: Partial<Tn3270CellInfo> = {}, count = 1920): Tn3270ScreenData {
  const cells: Tn3270CellInfo[] = Array.from({ length: count }, () => ({
    ch: " ",
    protected: false,
    numeric: false,
    displayable: true,
    intensified: false,
    field_start: false,
    ...overrides,
  }));
  return { session_id: "conn-1", rows: 24, cols: 80, cursor_addr: 0, cells };
}

describe("Tn3270Screen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects and renders the screen grid on tn3270:screen events", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "tn3270_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    renderWithToast(<Tn3270Screen sessionId="sess-1" config={config} />);
    expect(await screen.findByTestId("tn3270-grid-sess-1")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("tn3270_connect", { config });

    const screenData = makeScreen();
    screenData.cells[5] = { ...screenData.cells[5], ch: "X" };
    handlers["tn3270:screen"]({ payload: screenData });

    expect(await screen.findByText("X")).toBeInTheDocument();
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "tn3270_connect") return Promise.reject(new Error("Connection failed"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<Tn3270Screen sessionId="sess-1" config={config} />);
    expect(await screen.findByText(/Failed to connect/)).toBeInTheDocument();
  });

  it("clicking an unprotected field loads its text, and Apply calls tn3270_type", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "tn3270_connect") return Promise.resolve("conn-1");
      if (cmd === "tn3270_type") return Promise.resolve(makeScreen());
      return Promise.resolve(undefined);
    });

    renderWithToast(<Tn3270Screen sessionId="sess-1" config={config} />);
    await screen.findByTestId("tn3270-grid-sess-1");

    const screenData = makeScreen();
    screenData.cells[0] = { ...screenData.cells[0], field_start: true, protected: false };
    screenData.cells[1] = { ...screenData.cells[1], ch: "A" };
    screenData.cells[2] = { ...screenData.cells[2], ch: "B" };
    handlers["tn3270:screen"]({ payload: screenData });
    await screen.findByText("A");

    // Click the unprotected data cell right after the field-attribute cell.
    fireEvent.click(screen.getByText("A"));

    const input = await screen.findByDisplayValue("AB");
    fireEvent.change(input, { target: { value: "hello" } });
    fireEvent.click(screen.getByText("Apply"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("tn3270_type", { id: "conn-1", addr: 1, text: "hello" });
    });
  });

  it("sends an AID key on button click", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "tn3270_connect") return Promise.resolve("conn-1");
      if (cmd === "tn3270_aid") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    renderWithToast(<Tn3270Screen sessionId="sess-1" config={config} />);
    await screen.findByTestId("tn3270-grid-sess-1");

    fireEvent.click(screen.getByText("Enter"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("tn3270_aid", { id: "conn-1", aid: "enter" });
    });
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "tn3270_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<Tn3270Screen sessionId="sess-1" config={config} />);
    await screen.findByTestId("tn3270-grid-sess-1");
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("tn3270_disconnect", { id: "conn-1" });
  });
});
