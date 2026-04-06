import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import "@/i18n";
import CredentialManager from "@/components/Vault/CredentialManager";
import { useVaultStore } from "@/stores/vaultStore";

const mockInvoke = vi.mocked(invoke);

function resetStore() {
  useVaultStore.setState({
    vaultLocked: false,
    credentials: [],
    loading: false,
    error: null,
  });
}

describe("CredentialManager", () => {
  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
    // fetchCredentials is called on mount via useEffect; mock credential_list
    mockInvoke.mockResolvedValue([] as never);
  });

  it("FT-C-14: renders credential list from store", () => {
    useVaultStore.setState({
      credentials: [
        {
          id: "cred-1",
          name: "Production DB",
          credential_type: "password",
          username: "admin",
          tags: [],
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
        {
          id: "cred-2",
          name: "Deploy Key",
          credential_type: "ssh_key",
          username: null,
          tags: [],
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      ],
    });

    render(<CredentialManager />);

    expect(screen.getByText("Production DB")).toBeInTheDocument();
    expect(screen.getByText("admin")).toBeInTheDocument();
    expect(screen.getByText("Deploy Key")).toBeInTheDocument();
    expect(screen.getByText("SSH Key")).toBeInTheDocument();
  });

  it("FT-C-15: shows form fields for password credential type", async () => {
    const user = userEvent.setup();

    render(<CredentialManager />);

    // Click the Add button to open the form (exact match to avoid "Add Credential" empty-state btn)
    const addButton = screen.getByRole("button", { name: /^Add$/ });
    await user.click(addButton);

    // The form dialog should be open
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(screen.getByText("New Credential")).toBeInTheDocument();

    // Password type should be selected by default — password-specific fields should show
    expect(screen.getByText("Username")).toBeInTheDocument();
    // "Password" label exists for the field (may also appear as type button text)
    const passwordLabels = screen.getAllByText("Password");
    expect(passwordLabels.length).toBeGreaterThanOrEqual(1);

    // Name field should always be present
    expect(screen.getByPlaceholderText("My credential")).toBeInTheDocument();
  });
});
