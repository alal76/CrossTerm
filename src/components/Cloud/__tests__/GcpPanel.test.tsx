import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import GcpPanel from "@/components/Cloud/GcpPanel";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/sessionStore";
import type { CloudProviderStatus, GcpConfig, GcpInstance, GcsBucket } from "@/types";

const mockInvoke = vi.mocked(invoke);

function status(overrides: Partial<CloudProviderStatus> = {}): CloudProviderStatus {
  return {
    provider: "gcp",
    cli_status: { type: "installed", version: "1.0", path: "/usr/bin/gcloud" },
    profiles: [],
    ...overrides,
  };
}

function config(overrides: Partial<GcpConfig> = {}): GcpConfig {
  return { name: "default", project: "my-project", region: "us-central1", zone: "us-central1-a", is_active: true, ...overrides };
}

function instance(overrides: Partial<GcpInstance> = {}): GcpInstance {
  return {
    id: "gi-1",
    name: "web-1",
    zone: "us-central1-a",
    machine_type: "e2-micro",
    status: "RUNNING",
    external_ip: "9.9.9.9",
    ...overrides,
  };
}

function mockAll(overrides: Partial<Record<string, unknown>> = {}) {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "cloud_gcp_list_configs") return Promise.resolve(overrides.configs ?? [config()]);
    if (cmd === "cloud_gcp_list_instances") return Promise.resolve(overrides.instances ?? [instance()]);
    if (cmd === "cloud_gcp_list_buckets") return Promise.resolve(overrides.buckets ?? []);
    return Promise.resolve(undefined);
  });
}

describe("GcpPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSessionStore.setState({ sessions: [], openTabs: [], activeTabId: null });
  });

  it("shows a not-installed message when the CLI is missing", () => {
    render(<GcpPanel status={status({ cli_status: { type: "not_installed" } })} />);
    expect(screen.getByText("Install the Google Cloud SDK to get started.")).toBeInTheDocument();
  });

  it("selects the active config's project and loads instances", async () => {
    mockAll();
    render(<GcpPanel status={status()} />);

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_list_configs"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_list_instances", { project: "my-project" });
    });
    expect(await screen.findByText("web-1")).toBeInTheDocument();
  });

  it("shows the empty state when there are no instances", async () => {
    mockAll({ instances: [] });
    render(<GcpPanel status={status()} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_list_instances", { project: "my-project" });
    });
    expect(await screen.findByText(/No resources found/)).toBeInTheDocument();
  });

  it("switches to the storage tab and drills into a bucket", async () => {
    mockAll({
      buckets: [{ name: "gcs-bucket", location: "US", storage_class: "STANDARD", time_created: "2026-01-01" } satisfies GcsBucket],
    });
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "cloud_gcp_list_configs") return Promise.resolve([config()]);
      if (cmd === "cloud_gcp_list_instances") return Promise.resolve([instance()]);
      if (cmd === "cloud_gcp_list_buckets") return Promise.resolve([{ name: "gcs-bucket", location: "US", storage_class: "STANDARD", time_created: "2026-01-01" }]);
      if (cmd === "cloud_gcp_list_objects") return Promise.resolve([{ name: "obj.txt", size: 10, content_type: "text/plain", time_created: "2026-01-01" }]);
      return Promise.resolve(undefined);
    });
    render(<GcpPanel status={status()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_list_instances", { project: "my-project" }));

    fireEvent.click(screen.getByRole("button", { name: "Storage" }));
    expect(await screen.findByText("gcs-bucket")).toBeInTheDocument();

    fireEvent.click(screen.getByText("gcs-bucket"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_list_objects", { bucket: "gcs-bucket", prefix: "" });
    });
    expect(await screen.findByText("obj.txt")).toBeInTheDocument();
  });

  it("starts, stops, and IAP-tunnels to an instance via the real backend commands", async () => {
    mockAll();
    render(<GcpPanel status={status()} />);
    await screen.findByText("web-1");

    fireEvent.click(screen.getByTitle("Start"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_start_instance", { project: "my-project", zone: "us-central1-a", instance: "web-1" });
    });

    fireEvent.click(screen.getByTitle("Stop"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_stop_instance", { project: "my-project", zone: "us-central1-a", instance: "web-1" });
    });

    fireEvent.click(screen.getByTitle("IAP Tunnel"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_iap_tunnel", { project: "my-project", zone: "us-central1-a", instance: "web-1" });
    });
  });

  it("connect opens a real SSH session tab instead of calling a nonexistent backend command", async () => {
    mockAll();
    render(<GcpPanel status={status()} />);
    await screen.findByText("web-1");

    fireEvent.click(screen.getByTitle("Connect"));

    await waitFor(() => {
      const sessions = useSessionStore.getState().sessions;
      expect(sessions).toHaveLength(1);
      expect(sessions[0].connection.host).toBe("9.9.9.9");
      expect(sessions[0].type).toBe("ssh");
    });
  });

  it("logs in and launches Cloud Shell", async () => {
    mockAll();
    render(<GcpPanel status={status()} />);
    await screen.findByText("web-1");

    fireEvent.click(screen.getByText("Login"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_auth_login"));

    fireEvent.click(screen.getByText("Cloud Shell"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_cloud_shell"));
  });
});
