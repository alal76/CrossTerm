import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import FirstLaunchWizard from "@/components/Shared/FirstLaunchWizard";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "@/stores/appStore";

const mockInvoke = vi.mocked(invoke);

describe("FirstLaunchWizard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({ firstLaunchComplete: false, profiles: [], activeProfileId: null });
  });

  it("shows the welcome step first", () => {
    render(<FirstLaunchWizard />);
    expect(screen.getByText("CrossTerm")).toBeInTheDocument();
    expect(screen.getByText("Get Started")).toBeInTheDocument();
  });

  it("requires a profile name before advancing", async () => {
    render(<FirstLaunchWizard />);
    fireEvent.click(screen.getByText("Get Started"));
    expect(await screen.findByText("Profile Name")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Create"));
    expect(await screen.findByText("Profile name is required.")).toBeInTheDocument();
  });

  it("creates a profile and advances to the password step", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "profile_create") return Promise.resolve("profile-1");
      if (cmd === "profile_switch") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    render(<FirstLaunchWizard />);
    fireEvent.click(screen.getByText("Get Started"));
    fireEvent.change(await screen.findByLabelText("Profile Name"), { target: { value: "Work" } });
    fireEvent.click(screen.getByText("Create"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("profile_create", { name: "Work" });
    });
    expect(await screen.findByText("Set Master Password")).toBeInTheDocument();
    expect(useAppStore.getState().profiles.some((p) => p.name === "Work")).toBe(true);
  });

  it("falls back to a local profile id when profile_create fails", async () => {
    mockInvoke.mockRejectedValue(new Error("backend unavailable"));
    render(<FirstLaunchWizard />);
    fireEvent.click(screen.getByText("Get Started"));
    fireEvent.change(await screen.findByLabelText("Profile Name"), { target: { value: "Work" } });
    fireEvent.click(screen.getByText("Create"));

    expect(await screen.findByText("Set Master Password")).toBeInTheDocument();
    expect(useAppStore.getState().activeProfileId).toBeTruthy();
  });

  it("validates password requirements", async () => {
    mockInvoke.mockResolvedValue("profile-1");
    render(<FirstLaunchWizard />);
    fireEvent.click(screen.getByText("Get Started"));
    fireEvent.change(await screen.findByLabelText("Profile Name"), { target: { value: "Work" } });
    fireEvent.click(screen.getByText("Create"));
    await screen.findByText("Set Master Password");

    fireEvent.click(screen.getByText("Create"));
    expect(await screen.findByText("Password is required.")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Master Password"), { target: { value: "short" } });
    fireEvent.change(screen.getByLabelText("Confirm Password"), { target: { value: "short" } });
    fireEvent.click(screen.getByText("Create"));
    expect(await screen.findByText("Password must be at least 8 characters.")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Master Password"), { target: { value: "longenough1" } });
    fireEvent.change(screen.getByLabelText("Confirm Password"), { target: { value: "different1" } });
    fireEvent.click(screen.getByText("Create"));
    expect(await screen.findByText("Passwords do not match.")).toBeInTheDocument();
  });

  it("creates the vault and advances to the theme step", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "profile_create") return Promise.resolve("profile-1");
      if (cmd === "vault_create") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    render(<FirstLaunchWizard />);
    fireEvent.click(screen.getByText("Get Started"));
    fireEvent.change(await screen.findByLabelText("Profile Name"), { target: { value: "Work" } });
    fireEvent.click(screen.getByText("Create"));
    await screen.findByText("Set Master Password");

    fireEvent.change(screen.getByLabelText("Master Password"), { target: { value: "longenough1" } });
    fireEvent.change(screen.getByLabelText("Confirm Password"), { target: { value: "longenough1" } });
    fireEvent.click(screen.getByText("Create"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("vault_create", expect.objectContaining({ masterPassword: "longenough1" }));
    });
    expect(await screen.findByText("Choose a Theme")).toBeInTheDocument();
  });

  it("selects a theme and finishes the wizard", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "profile_create") return Promise.resolve("profile-1");
      if (cmd === "settings_update") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    render(<FirstLaunchWizard />);
    fireEvent.click(screen.getByText("Get Started"));
    fireEvent.change(await screen.findByLabelText("Profile Name"), { target: { value: "Work" } });
    fireEvent.click(screen.getByText("Create"));
    await screen.findByText("Set Master Password");
    fireEvent.change(screen.getByLabelText("Master Password"), { target: { value: "longenough1" } });
    fireEvent.change(screen.getByLabelText("Confirm Password"), { target: { value: "longenough1" } });
    fireEvent.click(screen.getByText("Create"));
    await screen.findByText("Choose a Theme");

    fireEvent.click(screen.getByText("Light"));
    fireEvent.click(screen.getByText("Finish"));

    await waitFor(() => {
      expect(useAppStore.getState().firstLaunchComplete).toBe(true);
    });
  });

  it("Back navigates to the previous step", async () => {
    render(<FirstLaunchWizard />);
    fireEvent.click(screen.getByText("Get Started"));
    await screen.findByText("Create a Profile");

    fireEvent.click(screen.getByText("Back"));
    expect(await screen.findByText("Get Started")).toBeInTheDocument();
  });

  it("expands the Learn More section", async () => {
    render(<FirstLaunchWizard />);
    fireEvent.click(screen.getByText("Get Started"));
    await screen.findByText("Create a Profile");

    fireEvent.click(screen.getByText(/Learn more about profiles/));
    expect(
      await screen.findByText(/Profiles let you keep separate sets of sessions/),
    ).toBeInTheDocument();
  });
});
