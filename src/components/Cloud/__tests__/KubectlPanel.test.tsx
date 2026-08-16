import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import KubectlPanel from "@/components/Cloud/KubectlPanel";
import { invoke } from "@tauri-apps/api/core";

const mockInvoke = vi.mocked(invoke);

const NAMESPACES_OUTPUT = "NAME       STATUS   AGE\ndefault    Active   10d\nkube-system Active  10d\n";
const PODS_OUTPUT = "NAME       READY   STATUS    RESTARTS   AGE\nweb-abc123 1/1     Running   0          2d\n";

function mockAll(overrides: Partial<Record<string, unknown>> = {}) {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "kubectl_get_namespaces") return Promise.resolve(overrides.namespaces ?? NAMESPACES_OUTPUT);
    if (cmd === "kubectl_list_pods") return Promise.resolve(overrides.pods ?? PODS_OUTPUT);
    if (cmd === "cloud_azure_aks_get_credentials") return Promise.resolve(undefined);
    if (cmd === "cloud_gcp_gke_get_credentials") return Promise.resolve(undefined);
    return Promise.resolve(undefined);
  });
}

describe("KubectlPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("fetches Azure AKS credentials before listing pods", async () => {
    mockAll();
    render(<KubectlPanel provider="azure" cluster="my-aks" resourceGroup="rg-1" />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_aks_get_credentials", { cluster: "my-aks", resourceGroup: "rg-1" });
    });
    expect(await screen.findByText("web-abc123")).toBeInTheDocument();
    expect(screen.getByText("Running")).toBeInTheDocument();
  });

  it("fetches GCP GKE credentials before listing pods", async () => {
    mockAll();
    render(<KubectlPanel provider="gcp" cluster="my-gke" zone="us-central1-a" project="my-project" />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_gke_get_credentials", { cluster: "my-gke", zone: "us-central1-a", project: "my-project" });
    });
    expect(await screen.findByText("web-abc123")).toBeInTheDocument();
  });

  it("parses namespaces from kubectl output and populates the selector", async () => {
    mockAll();
    render(<KubectlPanel provider="gcp" cluster="my-gke" />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("kubectl_get_namespaces", {}));

    expect(await screen.findByRole("option", { name: "kube-system" })).toBeInTheDocument();
  });

  it("shows the no-pods message when the namespace is empty", async () => {
    mockAll({ pods: "NAME   READY   STATUS   RESTARTS   AGE\n" });
    render(<KubectlPanel provider="gcp" cluster="my-gke" />);
    expect(await screen.findByText("No pods found")).toBeInTheDocument();
  });

  it("execs a command into a pod", async () => {
    mockAll();
    render(<KubectlPanel provider="gcp" cluster="my-gke" zone="z1" project="p1" />);
    await screen.findByText("web-abc123");

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "cloud_gcp_gke_exec") return Promise.resolve("total 0\n");
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByTitle("Exec"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_gcp_gke_exec", {
        cluster: "my-gke", namespace: "default", pod: "web-abc123", command: "ls -la", zone: "z1", project: "p1",
      });
    });
    expect(await screen.findByText("total 0")).toBeInTheDocument();
  });

  it("runs a custom command via the Run button and Enter key", async () => {
    mockAll();
    render(<KubectlPanel provider="azure" cluster="my-aks" resourceGroup="rg-1" />);
    await screen.findByText("web-abc123");
    fireEvent.click(screen.getByTitle("Exec"));
    await screen.findByText(/Exec — web-abc123/);

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "cloud_azure_aks_exec") return Promise.resolve("done");
      return Promise.resolve(undefined);
    });
    fireEvent.change(screen.getByPlaceholderText("ls -la"), { target: { value: "cat /etc/hostname" } });
    fireEvent.click(screen.getByText("Run"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_azure_aks_exec", {
        cluster: "my-aks", namespace: "default", pod: "web-abc123", command: "cat /etc/hostname", resourceGroup: "rg-1",
      });
    });
  });

  it("shows an exec error", async () => {
    mockAll();
    render(<KubectlPanel provider="gcp" cluster="my-gke" />);
    await screen.findByText("web-abc123");

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "cloud_gcp_gke_exec") return Promise.reject(new Error("pod not found"));
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByTitle("Exec"));

    expect(await screen.findByText(/pod not found/)).toBeInTheDocument();
  });

  it("refresh button reloads pods", async () => {
    mockAll();
    render(<KubectlPanel provider="gcp" cluster="my-gke" />);
    await screen.findByText("web-abc123");
    mockInvoke.mockClear();
    mockAll();

    fireEvent.click(screen.getByText("Refresh"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("kubectl_list_pods", { namespace: "default" }));
  });
});
