import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import "@/i18n";
import VaultUnlock from "@/components/Vault/VaultUnlock";
import { useVaultStore } from "@/stores/vaultStore";

const mockInvoke = vi.mocked(invoke);

function resetStore() {
  useVaultStore.setState({
    vaults: [],
    activeVaultId: null,
    vaultLockStates: {},
    vaultLocked: true,
    credentials: [],
    loading: false,
    error: null,
  });
}

const MOCK_VAULT = {
  id: "vault-1",
  name: "Default",
  is_default: true,
  owner_profile_id: "default",
  shared_with: [] as string[],
  created_at: new Date().toISOString(),
};

/** Route invoke calls by command name instead of sequential mocking */
function setupMockInvoke(overrides: Record<string, unknown> = {}) {
  const defaults: Record<string, unknown> = {
    vault_list: [],
    vault_is_locked: true,
    vault_biometric_available: false,
    vault_fido2_available: false,
    ...overrides,
  };
  mockInvoke.mockImplementation(async (cmd: string) => {
    if (cmd in defaults) return defaults[cmd];
    return undefined;
  });
}

describe("VaultUnlock", () => {
  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  it("shows Create mode when no vaults exist for profile", async () => {
    setupMockInvoke({ vault_list: [] });

    render(<VaultUnlock />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "Create Vault" })
      ).toBeInTheDocument();
    });

    expect(
      screen.getByText("Set a master password to encrypt your credentials.")
    ).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText("Confirm password")
    ).toBeInTheDocument();
  });

  it("shows Unlock mode when vaults exist but are locked", async () => {
    setupMockInvoke({
      vault_list: [MOCK_VAULT],
      vault_is_locked: true,
    });

    render(<VaultUnlock />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "Unlock Vault" })
      ).toBeInTheDocument();
    });

    expect(
      screen.getByText("Enter your master password to access saved credentials.")
    ).toBeInTheDocument();
    expect(
      screen.queryByPlaceholderText("Confirm password")
    ).not.toBeInTheDocument();
  });

  it("validates password minimum length (8 chars)", async () => {
    const user = userEvent.setup();
    setupMockInvoke({ vault_list: [] });

    render(<VaultUnlock />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "Create Vault" })
      ).toBeInTheDocument();
    });

    const passwordInput = screen.getByPlaceholderText("Master password");
    const confirmInput = screen.getByPlaceholderText("Confirm password");

    await user.type(passwordInput, "short");
    await user.type(confirmInput, "short");

    const submitButton = screen.getByRole("button", { name: "Create Vault" });
    await user.click(submitButton);

    expect(
      screen.getByText("Password must be at least 8 characters.")
    ).toBeInTheDocument();
  });

  it("FT-C-12: password confirmation mismatch shows error", async () => {
    const user = userEvent.setup();
    setupMockInvoke({ vault_list: [] });

    render(<VaultUnlock />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "Create Vault" })
      ).toBeInTheDocument();
    });

    const passwordInput = screen.getByPlaceholderText("Master password");
    const confirmInput = screen.getByPlaceholderText("Confirm password");

    await user.type(passwordInput, "securepassword123");
    await user.type(confirmInput, "differentpassword");

    const submitButton = screen.getByRole("button", { name: "Create Vault" });
    await user.click(submitButton);

    expect(
      screen.getByText("Passwords do not match.")
    ).toBeInTheDocument();
  });

  it("FT-C-13: submit calls invoke('vault_create') and unlockVault", async () => {
    const user = userEvent.setup();
    setupMockInvoke({
      vault_list: [],
      vault_create: MOCK_VAULT,
    });

    render(<VaultUnlock />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "Create Vault" })
      ).toBeInTheDocument();
    });

    const passwordInput = screen.getByPlaceholderText("Master password");
    const confirmInput = screen.getByPlaceholderText("Confirm password");

    await user.type(passwordInput, "strongpassword123");
    await user.type(confirmInput, "strongpassword123");

    const submitButton = screen.getByRole("button", { name: "Create Vault" });
    await user.click(submitButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vault_create", {
        profileId: "default",
        masterPassword: "strongpassword123",
        name: "My Vault",
        isDefault: true,
      });
    });
  });
});
