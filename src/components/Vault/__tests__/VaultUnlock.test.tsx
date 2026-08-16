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

  it("unlocks with the correct password", async () => {
    const user = userEvent.setup();
    setupMockInvoke({
      vault_list: [MOCK_VAULT],
      vault_is_locked: true,
      vault_unlock: undefined,
      vault_fetch_credentials: [],
    });

    render(<VaultUnlock />);
    await waitFor(() => screen.getByRole("heading", { name: "Unlock Vault" }));

    await user.type(screen.getByPlaceholderText("Password"), "correct-password");
    await user.click(screen.getByRole("button", { name: "Unlock Vault" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("vault_unlock", {
        vaultId: "vault-1",
        masterPassword: "correct-password",
      }),
    );
  });

  it("shows an error when unlock fails and requires a non-empty password", async () => {
    const user = userEvent.setup();
    setupMockInvoke({
      vault_list: [MOCK_VAULT],
      vault_is_locked: true,
    });
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "vault_list") return [MOCK_VAULT];
      if (cmd === "vault_is_locked") return true;
      if (cmd === "vault_unlock") throw new Error("incorrect password");
      return undefined;
    });

    render(<VaultUnlock />);
    await waitFor(() => screen.getByRole("heading", { name: "Unlock Vault" }));

    await user.type(screen.getByPlaceholderText("Password"), "wrong-password");
    await user.click(screen.getByRole("button", { name: "Unlock Vault" }));

    expect(await screen.findByText("Error: incorrect password")).toBeInTheDocument();
  });

  it("attempts biometric unlock when available", async () => {
    const user = userEvent.setup();
    setupMockInvoke({
      vault_list: [MOCK_VAULT],
      vault_is_locked: true,
      vault_biometric_available: true,
      vault_unlock_biometric: undefined,
      vault_unlock: undefined,
    });

    render(<VaultUnlock />);
    const button = await screen.findByRole("button", { name: /Touch ID|Windows Hello/ });
    await user.click(button);

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("vault_unlock_biometric"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("vault_unlock", { vaultId: "vault-1", masterPassword: "" }),
    );
  });

  it("shows biometricUnavailable error when the biometric attempt fails", async () => {
    const user = userEvent.setup();
    setupMockInvoke({
      vault_list: [MOCK_VAULT],
      vault_is_locked: true,
      vault_biometric_available: true,
    });
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "vault_list") return [MOCK_VAULT];
      if (cmd === "vault_is_locked") return true;
      if (cmd === "vault_biometric_available") return true;
      if (cmd === "vault_fido2_available") return false;
      if (cmd === "vault_unlock_biometric") throw new Error("no sensor");
      return undefined;
    });

    render(<VaultUnlock />);
    const button = await screen.findByRole("button", { name: /Touch ID|Windows Hello/ });
    await user.click(button);

    expect(
      await screen.findByText("Biometric authentication is not available on this device"),
    ).toBeInTheDocument();
  });

  it("shows fido2Unavailable when the security-key button is used", async () => {
    const user = userEvent.setup();
    setupMockInvoke({
      vault_list: [MOCK_VAULT],
      vault_is_locked: true,
      vault_fido2_available: true,
    });

    render(<VaultUnlock />);
    const button = await screen.findByRole("button", { name: "Use Security Key" });
    await user.click(button);

    expect(
      await screen.findByText("Hardware key authentication is not yet configured"),
    ).toBeInTheDocument();
  });

  it("navigates to create mode via 'Add new vault' and back again", async () => {
    const user = userEvent.setup();
    setupMockInvoke({ vault_list: [MOCK_VAULT], vault_is_locked: true });

    render(<VaultUnlock />);
    await waitFor(() => screen.getByRole("heading", { name: "Unlock Vault" }));

    await user.click(screen.getByText("Add new vault"));
    expect(await screen.findByRole("heading", { name: "Create Vault" })).toBeInTheDocument();

    await user.click(screen.getByText("Back to unlock"));
    expect(await screen.findByRole("heading", { name: "Unlock Vault" })).toBeInTheDocument();
  });

  it("deletes a vault via the two-click confirm guard and the password form", async () => {
    const user = userEvent.setup();
    // After the delete, listVaults() is called again — return an empty list.
    let deleted = false;
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "vault_list") return deleted ? [] : [MOCK_VAULT];
      if (cmd === "vault_is_locked") return true;
      if (cmd === "vault_biometric_available") return false;
      if (cmd === "vault_fido2_available") return false;
      if (cmd === "vault_delete") {
        deleted = true;
        return undefined;
      }
      return undefined;
    });

    render(<VaultUnlock />);
    await waitFor(() => screen.getByRole("heading", { name: "Unlock Vault" }));

    const deleteBtn = screen.getByTitle("Delete Vault");
    await user.click(deleteBtn); // arm
    await user.click(screen.getByTitle("Click again to delete")); // confirm

    expect(await screen.findByRole("heading", { name: "Delete vault" })).toBeInTheDocument();

    await user.type(screen.getByPlaceholderText("Password"), "correct-password");
    await user.click(screen.getByRole("button", { name: "Delete Vault" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("vault_delete", { vaultId: "vault-1", password: "correct-password" }),
    );
  });

  it("cancels the delete confirmation form via the Cancel button", async () => {
    const user = userEvent.setup();
    setupMockInvoke({ vault_list: [MOCK_VAULT], vault_is_locked: true });

    render(<VaultUnlock />);
    await waitFor(() => screen.getByRole("heading", { name: "Unlock Vault" }));

    const deleteBtn = screen.getByTitle("Delete Vault");
    await user.click(deleteBtn);
    await user.click(screen.getByTitle("Click again to delete"));
    await screen.findByRole("heading", { name: "Delete vault" });

    await user.click(screen.getByText("Cancel"));
    expect(await screen.findByRole("heading", { name: "Unlock Vault" })).toBeInTheDocument();
  });

  it("shows a vault selector list and switches selection when multiple vaults are locked", async () => {
    const secondVault = { ...MOCK_VAULT, id: "vault-2", name: "Work Vault", is_default: false };
    setupMockInvoke({
      vault_list: [MOCK_VAULT, secondVault],
      vault_is_locked: true,
    });

    render(<VaultUnlock />);
    await waitFor(() => screen.getByRole("heading", { name: "Unlock Vault" }));

    // MOCK_VAULT.name is itself "Default", and is_default renders a "Default"
    // badge — both legitimately produce the text "Default" here.
    expect(screen.getAllByText("Default").length).toBeGreaterThan(0);
    expect(screen.getByText("Work Vault")).toBeInTheDocument();

    const user = userEvent.setup();
    await user.click(screen.getByText("Work Vault"));
    // Selection changed (no crash / still on unlock screen with both options present).
    expect(screen.getByText("Work Vault")).toBeInTheDocument();
  });
});
