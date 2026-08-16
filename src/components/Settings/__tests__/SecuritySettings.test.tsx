import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import { invoke } from "@tauri-apps/api/core";
import SecuritySettings from "@/components/Settings/SecuritySettings";
import type { SecurityConfig } from "@/types";

const mockInvoke = vi.mocked(invoke);

function baseConfig(overrides: Partial<SecurityConfig> = {}): SecurityConfig {
  return {
    vault_timeout_secs: 300,
    clipboard_clear_secs: 30,
    audit_enabled: true,
    rate_limit: { max_attempts: 5, window_secs: 60, lockout_secs: 300 },
    ...overrides,
  };
}

describe("SecuritySettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows a loading spinner before the config resolves", () => {
    mockInvoke.mockReturnValue(new Promise(() => {}));
    render(<SecuritySettings />);
    expect(document.querySelector(".animate-spin")).toBeInTheDocument();
  });

  it("loads and renders the config and cert pins", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "security_get_config") return Promise.resolve(baseConfig());
      if (cmd === "security_cert_list_pins") {
        return Promise.resolve([
          ["example.com", { sha256: "ab:cd:ef", valid_from: "", valid_until: "", subject: "" }],
        ]);
      }
      return Promise.resolve(undefined);
    });
    render(<SecuritySettings />);

    expect(await screen.findByText("Security Settings")).toBeInTheDocument();
    expect(screen.getByText("example.com")).toBeInTheDocument();
    expect(screen.getByText("ab:cd:ef")).toBeInTheDocument();
  });

  it("shows the no-pins message when there are none", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "security_get_config") return Promise.resolve(baseConfig());
      if (cmd === "security_cert_list_pins") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<SecuritySettings />);
    expect(await screen.findByText("No certificate pins configured.")).toBeInTheDocument();
  });

  it("changes the vault timeout and clipboard-clear selects", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "security_get_config") return Promise.resolve(baseConfig());
      if (cmd === "security_cert_list_pins") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<SecuritySettings />);
    await screen.findByText("Security Settings");

    fireEvent.change(screen.getByDisplayValue("5 minutes"), { target: { value: "3600" } });
    expect(screen.getByDisplayValue("1 hour")).toBeInTheDocument();

    fireEvent.change(screen.getByDisplayValue("30 seconds"), { target: { value: "0" } });
    expect(screen.getAllByDisplayValue("Never").length).toBeGreaterThanOrEqual(1);
  });

  it("toggles the audit log switch", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "security_get_config") return Promise.resolve(baseConfig({ audit_enabled: true }));
      if (cmd === "security_cert_list_pins") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<SecuritySettings />);
    await screen.findByText("Security Settings");

    const toggle = screen.getByText("Audit Log").closest("div")!.parentElement!.querySelector("button")!;
    expect(toggle.className).toContain("bg-accent-primary");
    fireEvent.click(toggle);
    expect(toggle.className).toContain("bg-surface-sunken");
  });

  it("edits rate-limit fields", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "security_get_config") return Promise.resolve(baseConfig());
      if (cmd === "security_cert_list_pins") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<SecuritySettings />);
    await screen.findByText("Security Settings");

    fireEvent.change(screen.getByLabelText("Max Attempts"), { target: { value: "10" } });
    expect(screen.getByLabelText("Max Attempts")).toHaveValue(10);

    fireEvent.change(screen.getByLabelText("Window (s)"), { target: { value: "120" } });
    expect(screen.getByLabelText("Window (s)")).toHaveValue(120);

    fireEvent.change(screen.getByLabelText("Lockout (s)"), { target: { value: "900" } });
    expect(screen.getByLabelText("Lockout (s)")).toHaveValue(900);
  });

  it("saves the config via security_set_config", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "security_get_config") return Promise.resolve(baseConfig());
      if (cmd === "security_cert_list_pins") return Promise.resolve([]);
      if (cmd === "security_set_config") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    render(<SecuritySettings />);
    await screen.findByText("Security Settings");

    fireEvent.click(screen.getByText("Save"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("security_set_config", { config: baseConfig() }),
    );
  });

  it("does not crash the loading state if fetchConfig throws", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "security_get_config") return Promise.reject(new Error("backend unavailable"));
      return Promise.resolve(undefined);
    });
    render(<SecuritySettings />);
    // Stays on the spinner rather than crashing, since config remains null.
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("security_get_config"));
    expect(document.querySelector(".animate-spin")).toBeInTheDocument();
  });
});
