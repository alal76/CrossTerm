import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import SsoButton, { SsoConfigForm, SsoProviderList } from "@/components/Vault/SsoButton";
import type { OidcConfig } from "@/components/Vault/SsoButton";

const mockInvoke = vi.mocked(invoke);

describe("SsoButton", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the sign-in label for the given provider", () => {
    render(<SsoButton providerName="Okta" onSuccess={vi.fn()} onError={vi.fn()} />);
    expect(screen.getByText("Sign in with Okta")).toBeInTheDocument();
  });

  it("calls auth_oidc_begin and onSuccess with the profile on click", async () => {
    const profile = { sub: "abc123", email: "a@b.com", name: "A B" };
    mockInvoke.mockResolvedValue({
      profile,
      access_token: "tok",
      id_token: "idtok",
      callback_port: 4321,
    });
    const onSuccess = vi.fn();
    render(<SsoButton providerName="Okta" onSuccess={onSuccess} onError={vi.fn()} />);

    fireEvent.click(screen.getByRole("button"));
    expect(mockInvoke).toHaveBeenCalledWith(
      "auth_oidc_begin",
      expect.objectContaining({ config: expect.objectContaining({ provider_name: "Okta" }) }),
    );
    await waitFor(() => expect(onSuccess).toHaveBeenCalledWith(profile));
  });

  it("shows a spinner label while the flow is in progress", async () => {
    let resolveFlow: (v: unknown) => void = () => {};
    mockInvoke.mockReturnValue(
      new Promise((resolve) => {
        resolveFlow = resolve;
      }),
    );
    render(<SsoButton providerName="Okta" onSuccess={vi.fn()} onError={vi.fn()} />);

    fireEvent.click(screen.getByRole("button"));
    expect(await screen.findByText("Signing in…")).toBeInTheDocument();
    expect(screen.getByRole("button")).toHaveAttribute("aria-busy", "true");

    resolveFlow({
      profile: { sub: "x" },
      access_token: "t",
      id_token: "i",
      callback_port: 1,
    });
    await waitFor(() => expect(screen.getByText("Sign in with Okta")).toBeInTheDocument());
  });

  it("calls onError with the error message when the flow fails", async () => {
    mockInvoke.mockRejectedValue(new Error("no config found"));
    const onError = vi.fn();
    render(<SsoButton providerName="Okta" onSuccess={vi.fn()} onError={onError} />);

    fireEvent.click(screen.getByRole("button"));
    await waitFor(() => expect(onError).toHaveBeenCalledWith("no config found"));
  });

  it("calls onError with a string rejection as-is", async () => {
    mockInvoke.mockRejectedValue("plain string error");
    const onError = vi.fn();
    render(<SsoButton providerName="Okta" onSuccess={vi.fn()} onError={onError} />);

    fireEvent.click(screen.getByRole("button"));
    await waitFor(() => expect(onError).toHaveBeenCalledWith("plain string error"));
  });

  it("falls back to a generic message for a non-Error, non-string rejection", async () => {
    mockInvoke.mockRejectedValue({ weird: true });
    const onError = vi.fn();
    render(<SsoButton providerName="Okta" onSuccess={vi.fn()} onError={onError} />);

    fireEvent.click(screen.getByRole("button"));
    await waitFor(() => expect(onError).toHaveBeenCalledWith("OIDC authentication failed"));
  });

  it("does not trigger the flow when disabled", () => {
    render(<SsoButton providerName="Okta" onSuccess={vi.fn()} onError={vi.fn()} disabled />);
    fireEvent.click(screen.getByRole("button"));
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(screen.getByRole("button")).toBeDisabled();
  });

  it("ignores a second click while a flow is already in progress", async () => {
    let resolveFlow: (v: unknown) => void = () => {};
    mockInvoke.mockReturnValue(
      new Promise((resolve) => {
        resolveFlow = resolve;
      }),
    );
    render(<SsoButton providerName="Okta" onSuccess={vi.fn()} onError={vi.fn()} />);

    const button = screen.getByRole("button");
    fireEvent.click(button);
    fireEvent.click(button);
    expect(mockInvoke).toHaveBeenCalledTimes(1);
    resolveFlow({ profile: { sub: "x" }, access_token: "t", id_token: "i", callback_port: 1 });
  });
});

describe("SsoConfigForm", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  function fillRequiredFields() {
    fireEvent.change(screen.getByPlaceholderText("e.g. Okta"), { target: { value: "Okta" } });
    fireEvent.change(screen.getByPlaceholderText("0oa1b2c3d4e5f6g7h8i9"), {
      target: { value: "client-1" },
    });
    fireEvent.change(screen.getByPlaceholderText("https://your-idp.example.com/oauth2/authorize"), {
      target: { value: "https://idp.example.com/authorize" },
    });
    fireEvent.change(screen.getByPlaceholderText("https://your-idp.example.com/oauth2/token"), {
      target: { value: "https://idp.example.com/token" },
    });
  }

  it("shows validation errors for missing required fields", () => {
    render(<SsoConfigForm />);
    fireEvent.click(screen.getByText("Save provider"));
    expect(screen.getByText("Provider name is required.")).toBeInTheDocument();
  });

  it("validates fields in order: client id, then auth endpoint, then token endpoint", () => {
    render(<SsoConfigForm />);
    fireEvent.change(screen.getByPlaceholderText("e.g. Okta"), { target: { value: "Okta" } });
    fireEvent.click(screen.getByText("Save provider"));
    expect(screen.getByText("Client ID is required.")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("0oa1b2c3d4e5f6g7h8i9"), {
      target: { value: "client-1" },
    });
    fireEvent.click(screen.getByText("Save provider"));
    expect(screen.getByText("Authorization endpoint is required.")).toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("https://your-idp.example.com/oauth2/authorize"), {
      target: { value: "https://idp.example.com/authorize" },
    });
    fireEvent.click(screen.getByText("Save provider"));
    expect(screen.getByText("Token endpoint is required.")).toBeInTheDocument();
  });

  it("submits a well-formed config and calls onSaved", async () => {
    mockInvoke.mockResolvedValue(undefined);
    const onSaved = vi.fn();
    render(<SsoConfigForm onSaved={onSaved} />);
    fillRequiredFields();

    fireEvent.click(screen.getByText("Save provider"));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "auth_save_oidc_config",
        expect.objectContaining({
          config: expect.objectContaining({
            provider_name: "Okta",
            client_id: "client-1",
            scopes: ["openid", "email", "profile"],
          }),
        }),
      ),
    );
    await waitFor(() => expect(screen.getByText("Provider configuration saved.")).toBeInTheDocument());
    expect(onSaved).toHaveBeenCalled();
  });

  it("pre-fills fields from initialConfig", () => {
    const initialConfig: Partial<OidcConfig> = {
      provider_name: "Google",
      client_id: "gid",
      scopes: ["openid", "email"],
    };
    render(<SsoConfigForm initialConfig={initialConfig} />);
    expect(screen.getByDisplayValue("Google")).toBeInTheDocument();
    expect(screen.getByDisplayValue("gid")).toBeInTheDocument();
    expect(screen.getByDisplayValue("openid email")).toBeInTheDocument();
  });

  it("shows an error message when saving fails", async () => {
    mockInvoke.mockRejectedValue(new Error("network down"));
    render(<SsoConfigForm />);
    fillRequiredFields();
    fireEvent.click(screen.getByText("Save provider"));
    expect(await screen.findByText("network down")).toBeInTheDocument();
  });

  it("omits userinfo_endpoint when left blank", async () => {
    mockInvoke.mockResolvedValue(undefined);
    render(<SsoConfigForm />);
    fillRequiredFields();
    fireEvent.click(screen.getByText("Save provider"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "auth_save_oidc_config",
        expect.objectContaining({
          config: expect.objectContaining({ userinfo_endpoint: undefined }),
        }),
      ),
    );
  });
});

describe("SsoProviderList", () => {
  it("shows the empty state when there are no configs", () => {
    render(<SsoProviderList configs={[]} onDelete={vi.fn()} onAdd={vi.fn()} />);
    expect(screen.getByText("No OIDC providers configured.")).toBeInTheDocument();
  });

  it("renders configured providers and calls onDelete", () => {
    const onDelete = vi.fn();
    const configs: OidcConfig[] = [
      {
        provider_name: "Okta",
        client_id: "client-1",
        authorization_endpoint: "https://a",
        token_endpoint: "https://b",
        scopes: ["openid"],
      },
    ];
    render(<SsoProviderList configs={configs} onDelete={onDelete} onAdd={vi.fn()} />);
    expect(screen.getByText("Okta")).toBeInTheDocument();
    expect(screen.getByText("client-1")).toBeInTheDocument();

    fireEvent.click(screen.getByTitle("Remove Okta"));
    expect(onDelete).toHaveBeenCalledWith("Okta");
  });

  it("shows a spinner on the entry being deleted", () => {
    const configs: OidcConfig[] = [
      {
        provider_name: "Okta",
        client_id: "client-1",
        authorization_endpoint: "https://a",
        token_endpoint: "https://b",
        scopes: ["openid"],
      },
    ];
    render(<SsoProviderList configs={configs} onDelete={vi.fn()} onAdd={vi.fn()} deleting="Okta" />);
    expect(screen.getByTitle("Remove Okta")).toBeDisabled();
  });

  it("calls onAdd when the add-provider button is clicked", () => {
    const onAdd = vi.fn();
    render(<SsoProviderList configs={[]} onDelete={vi.fn()} onAdd={onAdd} />);
    fireEvent.click(screen.getByText("Add provider"));
    expect(onAdd).toHaveBeenCalled();
  });
});
