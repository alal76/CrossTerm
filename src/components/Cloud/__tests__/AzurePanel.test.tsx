import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import AzurePanel from "@/components/Cloud/AzurePanel";
import { invoke } from "@tauri-apps/api/core";
import type { CloudProviderStatus, AzureSubscription, AzureVm, AzureStorageAccount } from "@/types";

const mockInvoke = vi.mocked(invoke);

function status(overrides: Partial<CloudProviderStatus> = {}): CloudProviderStatus {
  return {
    provider: "azure",
    cli_status: { type: "installed", version: "2.0", path: "/usr/bin/az" },
    profiles: [],
    ...overrides,
  };
}

function subscription(overrides: Partial<AzureSubscription> = {}): AzureSubscription {
  return { id: "sub-1", name: "Production", state: "Enabled", tenant_id: "t1", ...overrides };
}

function vm(overrides: Partial<AzureVm> = {}): AzureVm {
  return {
    id: "vm-1",
    name: "web-vm",
    resource_group: "rg-1",
    location: "eastus",
    status: "running",
    size: "Standard_B1s",
    public_ip: "5.6.7.8",
    ...overrides,
  };
}

function mockAll(overrides: Partial<Record<string, unknown>> = {}) {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "cloud_azure_list_subscriptions") return Promise.resolve(overrides.subs ?? [subscription()]);
    if (cmd === "cloud_azure_list_vms") return Promise.resolve(overrides.vms ?? [vm()]);
    if (cmd === "cloud_azure_list_storage_accounts") return Promise.resolve(overrides.storage ?? []);
    return Promise.resolve(undefined);
  });
}

describe("AzurePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows a not-installed message when the CLI is missing", () => {
    render(<AzurePanel status={status({ cli_status: { type: "not_installed" } })} />);
    expect(screen.getByText("Install the Azure CLI to get started.")).toBeInTheDocument();
  });

  it("loads subscriptions and VMs, selecting the first subscription", async () => {
    mockAll();
    render(<AzurePanel status={status()} />);

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_list_subscriptions"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_list_vms", { subscriptionId: "sub-1" });
    });
    expect(await screen.findByText("web-vm")).toBeInTheDocument();
  });

  it("shows the empty state when there are no VMs", async () => {
    mockAll({ vms: [] });
    render(<AzurePanel status={status()} />);
    // Two-level async cascade (subscriptions -> selectedSub -> vms) — wait
    // for the actual vms fetch before asserting on its rendered result.
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_list_vms", { subscriptionId: "sub-1" }));
    expect(await screen.findByText(/No resources found/)).toBeInTheDocument();
  });

  it("switches to the storage tab", async () => {
    mockAll({ storage: [{ name: "sa1", resource_group: "rg-1", kind: "StorageV2", sku: "Standard_LRS", location: "eastus" } satisfies AzureStorageAccount] });
    render(<AzurePanel status={status()} />);
    await screen.findByText("web-vm");

    fireEvent.click(screen.getByRole("button", { name: "Storage" }));
    expect(await screen.findByText("sa1")).toBeInTheDocument();
  });

  it("starts, stops, connects, and bastion-connects to a VM", async () => {
    mockAll();
    render(<AzurePanel status={status()} />);
    await screen.findByText("web-vm");

    fireEvent.click(screen.getByTitle("Start"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_start_vm", { subscriptionId: "sub-1", vmId: "vm-1" });
    });

    fireEvent.click(screen.getByTitle("Stop"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_stop_vm", { subscriptionId: "sub-1", vmId: "vm-1" });
    });

    fireEvent.click(screen.getByTitle("Connect"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_connect_vm", { subscriptionId: "sub-1", vmId: "vm-1" });
    });

    fireEvent.click(screen.getByTitle("Bastion"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_bastion_connect", { subscriptionId: "sub-1", vmId: "vm-1" });
    });
  });

  it("logs in and launches Cloud Shell", async () => {
    mockAll();
    render(<AzurePanel status={status()} />);
    await screen.findByText("web-vm");

    fireEvent.click(screen.getByText("Login"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_login"));

    fireEvent.click(screen.getByText("Cloud Shell"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_cloud_shell"));
  });

  it("refresh button reloads the active section", async () => {
    mockAll();
    render(<AzurePanel status={status()} />);
    await screen.findByText("web-vm");
    mockInvoke.mockClear();
    mockAll();

    fireEvent.click(screen.getByText("Refresh"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_list_vms", { subscriptionId: "sub-1" });
    });
  });
});
