import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import CloudDashboard from "@/components/Cloud/CloudDashboard";
import { invoke } from "@tauri-apps/api/core";
import type { CloudProviderStatus } from "@/types";

const mockInvoke = vi.mocked(invoke);

function status(overrides: Partial<CloudProviderStatus> = {}): CloudProviderStatus {
  return {
    provider: "aws",
    cli_status: { type: "installed", version: "2.0", path: "/usr/bin/aws" },
    profiles: ["default"],
    active_profile: "default",
    ...overrides,
  };
}

describe("CloudDashboard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Sub-panels (AwsPanel/AzurePanel/GcpPanel) issue their own invoke calls
    // on mount — resolve everything harmlessly so this test can focus on
    // CloudDashboard's own tab/status logic.
    mockInvoke.mockResolvedValue([]);
  });

  it("shows a loading spinner while detecting CLIs", () => {
    mockInvoke.mockReturnValue(new Promise(() => {}));
    render(<CloudDashboard />);
    expect(document.querySelector(".animate-spin")).toBeInTheDocument();
  });

  it("detects CLIs on mount and shows a connected badge", async () => {
    mockInvoke.mockResolvedValue([status()]);
    render(<CloudDashboard />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_detect_clis");
    });
    expect(await screen.findByText("Connected")).toBeInTheDocument();
  });

  it("shows a not-installed badge for a missing CLI", async () => {
    mockInvoke.mockResolvedValue([status({ cli_status: { type: "not_installed" } })]);
    render(<CloudDashboard />);
    expect((await screen.findAllByText("CLI not installed")).length).toBeGreaterThan(0);
  });

  it("shows a not-authenticated badge when there are no profiles", async () => {
    mockInvoke.mockResolvedValue([status({ profiles: [], active_profile: undefined })]);
    render(<CloudDashboard />);
    expect(await screen.findByText("Not authenticated")).toBeInTheDocument();
  });

  it("switches tabs between AWS, Azure, GCP, and the asset tree", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<CloudDashboard />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_detect_clis"));

    fireEvent.click(screen.getByText("Microsoft Azure"));
    fireEvent.click(screen.getByText("Google Cloud"));
    fireEvent.click(screen.getByText("Resource Explorer"));
    fireEvent.click(screen.getByText("Amazon Web Services"));
    // No crash across all 4 tabs is the meaningful assertion here.
    expect(screen.getByText("Cloud Dashboard")).toBeInTheDocument();
  });
});
