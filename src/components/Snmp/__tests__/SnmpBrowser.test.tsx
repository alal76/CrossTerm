import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import SnmpBrowser from "@/components/Snmp/SnmpBrowser";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import type { SnmpConfig, SnmpVarBind } from "@/types";

const mockInvoke = vi.mocked(invoke);

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: SnmpConfig = { host: "10.0.0.30", port: 161, version: "v2c", community: "public", timeout_ms: 2000 };

describe("SnmpBrowser", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("sets up a session and shows the version/host", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snmp_add_session") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    renderWithToast(<SnmpBrowser sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/SNMP v2c — 10.0.0.30:161/)).toBeInTheDocument();
  });

  it("runs a GET and displays the typed result", async () => {
    const varbinds: SnmpVarBind[] = [{ oid: "1.3.6.1.2.1.1.1.0", value_type: "OctetString", value: "Linux server" }];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snmp_add_session") return Promise.resolve("conn-1");
      if (cmd === "snmp_get") return Promise.resolve(varbinds);
      return Promise.resolve(undefined);
    });

    renderWithToast(<SnmpBrowser sessionId="sess-1" config={config} />);
    await screen.findByText(/SNMP v2c/);

    fireEvent.click(screen.getByText("Get"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("snmp_get", { id: "conn-1", oid: "1.3.6.1.2.1.1.1.0" });
    });
    expect(await screen.findByText("Linux server")).toBeInTheDocument();
    expect(screen.getByText("OctetString")).toBeInTheDocument();
  });

  it("runs a WALK with the configured root OID and max vars", async () => {
    const varbinds: SnmpVarBind[] = [
      { oid: "1.3.6.1.2.1.1.1.0", value_type: "OctetString", value: "Linux server" },
      { oid: "1.3.6.1.2.1.1.3.0", value_type: "TimeTicks", value: "123456" },
    ];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snmp_add_session") return Promise.resolve("conn-1");
      if (cmd === "snmp_walk") return Promise.resolve(varbinds);
      return Promise.resolve(undefined);
    });

    renderWithToast(<SnmpBrowser sessionId="sess-1" config={config} />);
    await screen.findByText(/SNMP v2c/);

    fireEvent.click(screen.getByText("Walk"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("snmp_walk", { id: "conn-1", rootOid: "1.3.6.1.2.1", maxVars: 50 });
    });
    expect(await screen.findByText("123456")).toBeInTheDocument();
    expect(screen.getByText("TimeTicks")).toBeInTheDocument();
  });

  it("shows an error toast when GET fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snmp_add_session") return Promise.resolve("conn-1");
      if (cmd === "snmp_get") return Promise.reject(new Error("Timeout — no response from agent"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<SnmpBrowser sessionId="sess-1" config={config} />);
    await screen.findByText(/SNMP v2c/);

    fireEvent.click(screen.getByText("Get"));

    expect(await screen.findByText(/GET failed/)).toBeInTheDocument();
  });

  it("removes the session on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snmp_add_session") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<SnmpBrowser sessionId="sess-1" config={config} />);
    await screen.findByText(/SNMP v2c/);
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("snmp_remove_session", { id: "conn-1" });
  });
});
