import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import KeyManager from "@/components/KeyManager/KeyManager";
import { invoke } from "@tauri-apps/api/core";
import type { SshKeyInfo, AgentKey, CertificateInfo } from "@/types";

const mockInvoke = vi.mocked(invoke);

function sshKey(overrides: Partial<SshKeyInfo> = {}): SshKeyInfo {
  return {
    id: "k1",
    name: "id_ed25519",
    key_type: "ed25519",
    fingerprint: "SHA256:abc123",
    public_key: "ssh-ed25519 AAAA...",
    private_key_path: "~/.ssh/id_ed25519",
    created_at: "2026-01-01T00:00:00Z",
    tags: ["work"],
    ...overrides,
  };
}

function agentKey(overrides: Partial<AgentKey> = {}): AgentKey {
  return { fingerprint: "SHA256:xyz", key_type: "ed25519", ...overrides };
}

function certificate(overrides: Partial<CertificateInfo> = {}): CertificateInfo {
  return {
    id: "c1",
    key_id: "k1",
    serial: 42,
    cert_type: "user",
    valid_after: "2026-01-01T00:00:00Z",
    valid_before: "2026-06-01T00:00:00Z",
    principals: ["alice"],
    extensions: [],
    ...overrides,
  };
}

function mockAll(overrides: Partial<Record<string, unknown>> = {}) {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "keymgr_list_keys") return Promise.resolve(overrides.keys ?? [sshKey()]);
    if (cmd === "keymgr_agent_list") return Promise.resolve(overrides.agentKeys ?? []);
    if (cmd === "keymgr_cert_list") return Promise.resolve(overrides.certs ?? []);
    return Promise.resolve(undefined);
  });
}

describe("KeyManager", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads and displays keys on mount", async () => {
    mockAll();
    render(<KeyManager />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("keymgr_list_keys");
      expect(mockInvoke).toHaveBeenCalledWith("keymgr_agent_list");
      expect(mockInvoke).toHaveBeenCalledWith("keymgr_cert_list");
    });
    expect(await screen.findByText("id_ed25519")).toBeInTheDocument();
    expect(screen.getByText("work")).toBeInTheDocument();
  });

  it("shows the empty state when there are no keys", async () => {
    mockAll({ keys: [] });
    render(<KeyManager />);
    expect(await screen.findByText("No SSH keys found. Import or generate a key to get started.")).toBeInTheDocument();
  });

  it("imports a key", async () => {
    mockAll({ keys: [] });
    render(<KeyManager />);
    await screen.findByText(/No SSH keys/);

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "keymgr_import_key") return Promise.resolve(sshKey());
      if (cmd === "keymgr_list_keys") return Promise.resolve([sshKey()]);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByText("Import Key"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("keymgr_import_key", expect.objectContaining({ path: "~/.ssh/id_ed25519" }));
    });
    expect(await screen.findByText("id_ed25519")).toBeInTheDocument();
  });

  it("exports and deletes a key", async () => {
    mockAll();
    render(<KeyManager />);
    await screen.findByText("id_ed25519");

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "keymgr_export_key") return Promise.resolve([1, 2, 3]);
      if (cmd === "keymgr_delete_key") return Promise.resolve(undefined);
      if (cmd === "keymgr_list_keys") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    fireEvent.click(screen.getByTitle("Export Key"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("keymgr_export_key", { keyId: "k1", format: "openssh" });
    });

    fireEvent.click(screen.getByTitle("Delete Key"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("keymgr_delete_key", { keyId: "k1" });
    });
  });

  it("adds a key to the agent from the keys tab", async () => {
    mockAll();
    render(<KeyManager />);
    await screen.findByText("id_ed25519");

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "keymgr_agent_add") return Promise.resolve(undefined);
      if (cmd === "keymgr_agent_list") return Promise.resolve([agentKey()]);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByTitle("Add to Agent"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("keymgr_agent_add", { keyId: "k1", lifetime: 3600 });
    });
  });

  it("switches to the agent tab and removes a key", async () => {
    mockAll({ agentKeys: [agentKey()] });
    render(<KeyManager />);
    await screen.findByText("id_ed25519");

    fireEvent.click(screen.getByRole("button", { name: "SSH Agent" }));
    expect(await screen.findByText("SHA256:xyz")).toBeInTheDocument();

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "keymgr_agent_remove") return Promise.resolve(undefined);
      if (cmd === "keymgr_agent_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByTitle("Remove from Agent"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("keymgr_agent_remove", { fingerprint: "SHA256:xyz" });
    });
  });

  it("removes all agent keys", async () => {
    mockAll({ agentKeys: [agentKey()] });
    render(<KeyManager />);
    await screen.findByText("id_ed25519");
    fireEvent.click(screen.getByRole("button", { name: "SSH Agent" }));
    await screen.findByText("SHA256:xyz");

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "keymgr_agent_remove_all") return Promise.resolve(undefined);
      if (cmd === "keymgr_agent_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByText("Remove All"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("keymgr_agent_remove_all");
    });
  });

  it("switches to the certificates tab and shows certs", async () => {
    mockAll({ certs: [certificate()] });
    render(<KeyManager />);
    await screen.findByText("id_ed25519");

    fireEvent.click(screen.getByRole("button", { name: "Certificates" }));
    expect(await screen.findByText("User Certificate")).toBeInTheDocument();
    expect(screen.getByText("Principals: alice")).toBeInTheDocument();
  });

  it("shows an error message when loading keys fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "keymgr_list_keys") return Promise.reject(new Error("agent unreachable"));
      return Promise.resolve([]);
    });
    render(<KeyManager />);
    expect(await screen.findByText(/agent unreachable/)).toBeInTheDocument();
  });
});
