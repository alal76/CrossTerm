import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import WinRmConsole from "@/components/WinRm/WinRmConsole";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import type { WinRmConfig, WinRmCommandResult } from "@/types";

Element.prototype.scrollIntoView = vi.fn();

const mockInvoke = vi.mocked(invoke);

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: WinRmConfig = {
  host: "10.0.0.5",
  port: 5985,
  username: "Administrator",
  password: "hunter2",
  use_tls: false,
  auth: "ntlm",
  verify_tls: false,
};

describe("WinRmConsole", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("connects and shows the host and auth method", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "winrm_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    renderWithToast(<WinRmConsole sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/WinRM — 10.0.0.5:5985/)).toBeInTheDocument();
    expect(screen.getByText("ntlm")).toBeInTheDocument();
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "winrm_connect") return Promise.reject(new Error("Authentication failed"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<WinRmConsole sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Couldn't connect/)).toBeInTheDocument();
    expect(screen.getByText(/Authentication failed/)).toBeInTheDocument();
  });

  it("runs a command and logs stdout with its exit code", async () => {
    const result: WinRmCommandResult = { stdout: "hello\n", stderr: "", exit_code: 0 };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "winrm_connect") return Promise.resolve("conn-1");
      if (cmd === "winrm_run_command") return Promise.resolve(result);
      return Promise.resolve(undefined);
    });

    renderWithToast(<WinRmConsole sessionId="sess-1" config={config} />);
    await screen.findByText(/WinRM — /);

    fireEvent.change(screen.getByPlaceholderText(/ipconfig/), { target: { value: "echo hello" } });
    fireEvent.click(screen.getByText("Run"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("winrm_run_command", { id: "conn-1", command: "echo hello" });
    });
    expect(await screen.findByText("hello")).toBeInTheDocument();
    expect(screen.getByText("exit 0")).toBeInTheDocument();
  });

  it("surfaces a nonzero exit code via toast and shows stderr", async () => {
    const result: WinRmCommandResult = { stdout: "", stderr: "not found\n", exit_code: 1 };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "winrm_connect") return Promise.resolve("conn-1");
      if (cmd === "winrm_run_command") return Promise.resolve(result);
      return Promise.resolve(undefined);
    });

    renderWithToast(<WinRmConsole sessionId="sess-1" config={config} />);
    await screen.findByText(/WinRM — /);

    fireEvent.change(screen.getByPlaceholderText(/ipconfig/), { target: { value: "bogus" } });
    fireEvent.click(screen.getByText("Run"));

    expect(await screen.findByText("exit 1")).toBeInTheDocument();
    expect(screen.getByText("not found")).toBeInTheDocument();
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "winrm_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<WinRmConsole sessionId="sess-1" config={config} />);
    await screen.findByText(/WinRM — /);
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("winrm_disconnect", { id: "conn-1" });
  });
});
