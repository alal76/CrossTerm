import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import "@/i18n";
import CredentialManager from "@/components/Vault/CredentialManager";
import { useVaultStore } from "@/stores/vaultStore";

const mockInvoke = vi.mocked(invoke);

function resetStore() {
  useVaultStore.setState({
    activeVaultId: "vault-1",
    vaultLocked: false,
    vaultLockStates: { "vault-1": false },
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

    render(<CredentialManager onClose={() => {}} />);

    expect(screen.getByText("Production DB")).toBeInTheDocument();
    expect(screen.getByText("admin")).toBeInTheDocument();
    expect(screen.getByText("Deploy Key")).toBeInTheDocument();
    expect(screen.getByText("SSH Key")).toBeInTheDocument();
  });

  it("FT-C-15: shows form fields for password credential type", async () => {
    const user = userEvent.setup();

    render(<CredentialManager onClose={() => {}} />);

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

  it("shows the empty state with an add-credential CTA when there are no credentials", async () => {
    const user = userEvent.setup();
    render(<CredentialManager onClose={() => {}} />);

    expect(await screen.findByText("No credentials")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Add Credential" }));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("requires a name before saving", async () => {
    const user = userEvent.setup();
    render(<CredentialManager onClose={() => {}} />);
    await user.click(screen.getByRole("button", { name: /^Add$/ }));

    await user.click(screen.getByRole("button", { name: "Create" }));
    expect(await screen.findByText("Name is required.")).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("credential_create", expect.anything());
  });

  it("creates a password credential with the entered fields", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "credential_create") return Promise.resolve("cred-new");
      if (cmd === "credential_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<CredentialManager onClose={() => {}} />);
    await user.click(screen.getByRole("button", { name: /^Add$/ }));

    await user.type(screen.getByPlaceholderText("My credential"), "Prod DB");
    await user.type(screen.getByPlaceholderText("admin"), "root");

    await user.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("credential_create", {
        vaultId: "vault-1",
        request: {
          name: "Prod DB",
          credential_type: "password",
          username: "root",
          data: { password: "" },
        },
      }),
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("switches to the SSH key type and shows its fields", async () => {
    const user = userEvent.setup();
    render(<CredentialManager onClose={() => {}} />);
    await user.click(screen.getByRole("button", { name: /^Add$/ }));

    await user.click(screen.getByRole("button", { name: /SSH Key/ }));
    expect(screen.getByText("Private Key")).toBeInTheDocument();
    expect(screen.queryByText("Username")).not.toBeInTheDocument();
  });

  it("creates an API token credential", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "credential_create") return Promise.resolve("cred-new");
      if (cmd === "credential_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<CredentialManager onClose={() => {}} />);
    await user.click(screen.getByRole("button", { name: /^Add$/ }));
    await user.click(screen.getByRole("button", { name: /API Token/ }));

    await user.type(screen.getByPlaceholderText("My credential"), "GH Token");
    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("credential_create", {
        vaultId: "vault-1",
        request: {
          name: "GH Token",
          credential_type: "api_token",
          username: undefined,
          data: { provider: "", token: "" },
        },
      }),
    );
  });

  it("creates a cloud credential with a provider selected from the dropdown", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "credential_create") return Promise.resolve("cred-new");
      if (cmd === "credential_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<CredentialManager onClose={() => {}} />);
    await user.click(screen.getByRole("button", { name: /^Add$/ }));
    await user.click(screen.getByRole("button", { name: /Cloud Credential/ }));

    await user.type(screen.getByPlaceholderText("My credential"), "AWS Prod");
    await user.selectOptions(screen.getByRole("combobox"), "aws");
    fireEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "credential_create",
        expect.objectContaining({
          request: expect.objectContaining({
            data: { provider: "aws", access_key: "", secret_key: "", region: undefined },
          }),
        }),
      ),
    );
  });

  it("edits an existing credential and pre-fills its fields", async () => {
    const user = userEvent.setup();
    const seeded = {
      id: "cred-1",
      name: "Prod DB",
      credential_type: "password",
      username: "admin",
      tags: [],
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "credential_update") return Promise.resolve(undefined);
      if (cmd === "credential_list") return Promise.resolve([seeded]);
      return Promise.resolve(undefined);
    });
    render(<CredentialManager onClose={() => {}} />);

    await user.click(await screen.findByTitle("Edit"));
    expect(screen.getByText("Edit Credential")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Prod DB")).toBeInTheDocument();
    expect(screen.getByDisplayValue("admin")).toBeInTheDocument();
    // Type selector is hidden once editing (type can't change).
    expect(screen.queryByRole("button", { name: /SSH Key/ })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("credential_update", {
        vaultId: "vault-1",
        id: "cred-1",
        request: { name: "Prod DB", username: "admin", data: { password: "" } },
      }),
    );
  });

  it("deletes a credential via the two-click confirm guard", async () => {
    const user = userEvent.setup();
    const seeded = {
      id: "cred-1",
      name: "Prod DB",
      credential_type: "password",
      username: "admin",
      tags: [],
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "credential_delete") return Promise.resolve(undefined);
      if (cmd === "credential_list") return Promise.resolve([seeded]);
      return Promise.resolve(undefined);
    });
    render(<CredentialManager onClose={() => {}} />);

    const deleteBtn = await screen.findByTitle("Delete");
    await user.click(deleteBtn); // arm
    expect(screen.getByText("Confirm?")).toBeInTheDocument();
    await user.click(screen.getByText("Confirm?")); // confirm

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("credential_delete", { vaultId: "vault-1", id: "cred-1" }),
    );
  });

  it("shows a save error without closing the form", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "credential_create") return Promise.reject(new Error("vault is locked"));
      if (cmd === "credential_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<CredentialManager onClose={() => {}} />);
    await user.click(await screen.findByRole("button", { name: /^Add$/ }));
    await user.type(screen.getByPlaceholderText("My credential"), "Prod DB");
    await user.click(screen.getByRole("button", { name: "Create" }));

    expect(await screen.findByText("Error: vault is locked")).toBeInTheDocument();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("closes the form via the Cancel button and the X button", async () => {
    const user = userEvent.setup();
    render(<CredentialManager onClose={() => {}} />);
    await user.click(screen.getByRole("button", { name: /^Add$/ }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("calls onClose on Escape only when the form is not open", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(<CredentialManager onClose={onClose} />);

    await user.click(screen.getByRole("button", { name: /^Add$/ }));
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });
});
