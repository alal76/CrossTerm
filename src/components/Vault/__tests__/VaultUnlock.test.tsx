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
    vaultLocked: true,
    credentials: [],
    loading: false,
    error: null,
  });
}

describe("VaultUnlock", () => {
  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  it("shows Create mode when vault does not exist", async () => {
    // vault_is_locked throws => vault doesn't exist => isNewVault = true
    mockInvoke.mockRejectedValueOnce(new Error("no vault"));

    render(<VaultUnlock />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "Create Vault" })
      ).toBeInTheDocument();
    });

    expect(
      screen.getByText("Set a master password to encrypt your credentials.")
    ).toBeInTheDocument();
    // Confirm password field should be present
    expect(
      screen.getByPlaceholderText("Confirm password")
    ).toBeInTheDocument();
  });

  it("shows Unlock mode when vault exists but is locked", async () => {
    // vault_is_locked resolves => vault exists => isNewVault = false
    mockInvoke.mockResolvedValueOnce(true);

    render(<VaultUnlock />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "Unlock Vault" })
      ).toBeInTheDocument();
    });

    expect(
      screen.getByText("Enter your master password to access saved credentials.")
    ).toBeInTheDocument();
    // No confirm password field in unlock mode
    expect(
      screen.queryByPlaceholderText("Confirm password")
    ).not.toBeInTheDocument();
  });

  it("validates password minimum length (8 chars)", async () => {
    const user = userEvent.setup();
    // vault doesn't exist => create mode
    mockInvoke.mockRejectedValueOnce(new Error("no vault"));

    render(<VaultUnlock />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "Create Vault" })
      ).toBeInTheDocument();
    });

    const passwordInput = screen.getByPlaceholderText("Master password");
    const confirmInput = screen.getByPlaceholderText("Confirm password");

    // Type a short password
    await user.type(passwordInput, "short");
    await user.type(confirmInput, "short");

    // Submit the form
    const submitButton = screen.getByRole("button", { name: "Create Vault" });
    await user.click(submitButton);

    // Validation error should appear
    expect(
      screen.getByText("Password must be at least 8 characters.")
    ).toBeInTheDocument();
  });
});
