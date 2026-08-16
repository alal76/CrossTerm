import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { ToastProvider } from "@/components/Shared/Toast";
import TeamPanel from "@/components/Settings/TeamPanel";

const mockInvoke = vi.mocked(invoke);

function renderWithToast() {
  return render(
    <ToastProvider>
      <TeamPanel />
    </ToastProvider>,
  );
}

const MEMBER = {
  id: "m1",
  display_name: "Jane Smith",
  email: "jane@example.com",
  role: "power_user" as const,
  public_key: null,
  added_at: "2026-01-01T00:00:00Z",
  last_active: "2026-01-02T00:00:00Z",
};

const TEAM_CONFIG = {
  members: [],
  require_mfa: false,
  session_timeout_minutes: 60,
  allowed_ips: ["10.0.0.0/8"],
};

describe("TeamPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rbac_list_members") return Promise.resolve([]);
      if (cmd === "rbac_get_team_config") return Promise.resolve(TEAM_CONFIG);
      return Promise.resolve(undefined);
    });
  });

  it("shows the empty state when there are no members", async () => {
    renderWithToast();
    expect(await screen.findByText("No team members yet.")).toBeInTheDocument();
  });

  it("renders a member table row with role/email/last-active", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rbac_list_members") return Promise.resolve([MEMBER]);
      if (cmd === "rbac_get_team_config") return Promise.resolve(TEAM_CONFIG);
      return Promise.resolve(undefined);
    });
    renderWithToast();

    expect(await screen.findByText("Jane Smith")).toBeInTheDocument();
    expect(screen.getByText("jane@example.com")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Power User")).toBeInTheDocument();
  });

  it("shows an error toast when loading fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rbac_list_members") return Promise.reject(new Error("network down"));
      return Promise.resolve(undefined);
    });
    renderWithToast();
    expect(await screen.findByText(/Failed to load team data/)).toBeInTheDocument();
  });

  it("opens the invite modal, validates the display name, and submits", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rbac_list_members") return Promise.resolve([]);
      if (cmd === "rbac_get_team_config") return Promise.resolve(TEAM_CONFIG);
      if (cmd === "rbac_add_member") return Promise.resolve(MEMBER);
      return Promise.resolve(undefined);
    });
    renderWithToast();
    await screen.findByText("No team members yet.");

    fireEvent.click(screen.getAllByText("Invite Member")[0]);
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    // The display-name input has a native `required` attribute, which blocks
    // jsdom's form submission before the submit event fires for a truly
    // empty value. Whitespace passes native validation but still fails the
    // component's own `.trim()` check, so it reliably exercises that branch.
    fireEvent.change(screen.getByLabelText(/Display Name/), { target: { value: "   " } });
    fireEvent.click(screen.getByText("Add Member"));
    expect(await screen.findByText("Display name is required.")).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("rbac_add_member", expect.anything());

    fireEvent.change(screen.getByLabelText(/Display Name/), { target: { value: "Jane Smith" } });
    fireEvent.change(screen.getByLabelText(/Email/), { target: { value: "jane@example.com" } });
    fireEvent.change(screen.getByLabelText("Role"), { target: { value: "power_user" } });
    fireEvent.click(screen.getByText("Add Member"));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("rbac_add_member", {
        displayName: "Jane Smith",
        email: "jane@example.com",
        role: "power_user",
      }),
    );
    expect(await screen.findByText("Jane Smith")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("closes the invite modal via Cancel", async () => {
    renderWithToast();
    await screen.findByText("No team members yet.");
    fireEvent.click(screen.getAllByText("Invite Member")[0]);
    fireEvent.click(screen.getByText("Cancel"));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("changes a member's role inline", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rbac_list_members") return Promise.resolve([MEMBER]);
      if (cmd === "rbac_get_team_config") return Promise.resolve(TEAM_CONFIG);
      if (cmd === "rbac_update_member_role") return Promise.resolve({ ...MEMBER, role: "admin" });
      return Promise.resolve(undefined);
    });
    renderWithToast();
    await screen.findByText("Jane Smith");

    fireEvent.change(screen.getByDisplayValue("Power User"), { target: { value: "admin" } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("rbac_update_member_role", { memberId: "m1", role: "admin" }),
    );
    expect(await screen.findByDisplayValue("Admin")).toBeInTheDocument();
  });

  it("removes a member via the two-step confirm", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rbac_list_members") return Promise.resolve([MEMBER]);
      if (cmd === "rbac_get_team_config") return Promise.resolve(TEAM_CONFIG);
      if (cmd === "rbac_remove_member") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    renderWithToast();
    await screen.findByText("Jane Smith");

    fireEvent.click(screen.getByText("Remove"));
    expect(screen.getByText("Remove?")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Yes"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("rbac_remove_member", { memberId: "m1" }));
    await waitFor(() => expect(screen.queryByText("Jane Smith")).not.toBeInTheDocument());
  });

  it("cancels a pending remove via No", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rbac_list_members") return Promise.resolve([MEMBER]);
      if (cmd === "rbac_get_team_config") return Promise.resolve(TEAM_CONFIG);
      return Promise.resolve(undefined);
    });
    renderWithToast();
    await screen.findByText("Jane Smith");

    fireEvent.click(screen.getByText("Remove"));
    fireEvent.click(screen.getByText("No"));
    expect(screen.queryByText("Remove?")).not.toBeInTheDocument();
    expect(screen.getByText("Jane Smith")).toBeInTheDocument();
  });

  it("toggles Require MFA and edits session timeout and allowed IPs, then saves", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rbac_list_members") return Promise.resolve([]);
      if (cmd === "rbac_get_team_config") return Promise.resolve(TEAM_CONFIG);
      if (cmd === "rbac_update_team_config") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    renderWithToast();
    await screen.findByText("No team members yet.");

    const mfaToggle = screen.getByText("Require MFA").closest("div")!.parentElement!.querySelector("button")!;
    fireEvent.click(mfaToggle);

    const timeoutInput = screen.getByDisplayValue("60");
    fireEvent.change(timeoutInput, { target: { value: "120" } });

    const ipsInput = screen.getByPlaceholderText("192.168.1.0/24, 10.0.0.1");
    fireEvent.change(ipsInput, { target: { value: "10.0.0.0/8, 172.16.0.0/12" } });

    fireEvent.click(screen.getByText("Save Settings"));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("rbac_update_team_config", {
        config: expect.objectContaining({
          require_mfa: true,
          session_timeout_minutes: 120,
          allowed_ips: ["10.0.0.0/8", "172.16.0.0/12"],
        }),
      }),
    );
    expect(await screen.findByText("Team settings saved.")).toBeInTheDocument();
  });

  it("shows an error toast when saving team settings fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "rbac_list_members") return Promise.resolve([]);
      if (cmd === "rbac_get_team_config") return Promise.resolve(TEAM_CONFIG);
      if (cmd === "rbac_update_team_config") return Promise.reject(new Error("locked"));
      return Promise.resolve(undefined);
    });
    renderWithToast();
    await screen.findByText("No team members yet.");

    fireEvent.click(screen.getByText("Save Settings"));
    expect(await screen.findByText(/Failed to save settings/)).toBeInTheDocument();
  });
});
