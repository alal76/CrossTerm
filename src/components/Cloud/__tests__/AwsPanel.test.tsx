import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import AwsPanel from "@/components/Cloud/AwsPanel";
import { invoke } from "@tauri-apps/api/core";
import type { CloudProviderStatus, Ec2Instance, S3Bucket, S3Object, CostSummary } from "@/types";

const mockInvoke = vi.mocked(invoke);

function status(overrides: Partial<CloudProviderStatus> = {}): CloudProviderStatus {
  return {
    provider: "aws",
    cli_status: { type: "installed", version: "2.0", path: "/usr/bin/aws" },
    profiles: ["default", "prod"],
    active_profile: "default",
    ...overrides,
  };
}

function instance(overrides: Partial<Ec2Instance> = {}): Ec2Instance {
  return {
    id: "i-123",
    name: "web-1",
    state: "running",
    instance_type: "t3.micro",
    public_ip: "1.2.3.4",
    launch_time: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

describe("AwsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows a not-installed message when the CLI is missing", () => {
    render(<AwsPanel status={status({ cli_status: { type: "not_installed" } })} />);
    expect(screen.getByText("Install the AWS CLI to get started.")).toBeInTheDocument();
  });

  it("initializes the profile selector from status and loads instances", async () => {
    mockInvoke.mockResolvedValue([instance()]);
    render(<AwsPanel status={status()} />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_instances", { profile: "default" });
    });
    expect(await screen.findByText("i-123")).toBeInTheDocument();
    expect(screen.getByText("running")).toBeInTheDocument();
  });

  it("shows the empty state when there are no instances", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<AwsPanel status={status()} />);
    expect(await screen.findByText(/No resources found/)).toBeInTheDocument();
  });

  it("switches to the storage tab and browses a bucket", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "cloud_aws_list_buckets") {
        return Promise.resolve([{ name: "my-bucket", region: "us-east-1", creation_date: "2026-01-01" } satisfies S3Bucket]);
      }
      if (cmd === "cloud_aws_list_objects") {
        return Promise.resolve([{ key: "file.txt", size: 100, last_modified: "2026-01-01", storage_class: "STANDARD" } satisfies S3Object]);
      }
      return Promise.resolve([]);
    });
    render(<AwsPanel status={status()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_instances", { profile: "default" }));

    fireEvent.click(screen.getByRole("button", { name: "Storage" }));
    expect(await screen.findByText("my-bucket")).toBeInTheDocument();

    fireEvent.click(screen.getByText("my-bucket"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_objects", { profile: "default", bucket: "my-bucket" });
    });
    expect(await screen.findByText("file.txt")).toBeInTheDocument();

    fireEvent.click(screen.getByText(/Buckets/));
    expect(await screen.findByText("my-bucket")).toBeInTheDocument();
  });

  it("switches to the cost tab and shows the summary", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "cloud_aws_get_cost") {
        return Promise.resolve({
          total_cost: 42.5,
          currency: "USD",
          start_date: "2026-01-01",
          end_date: "2026-01-31",
          by_service: [{ service_name: "EC2", cost: 30 }],
        } satisfies CostSummary);
      }
      return Promise.resolve([]);
    });
    render(<AwsPanel status={status()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_instances", { profile: "default" }));

    fireEvent.click(screen.getByRole("button", { name: "Cost" }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_get_cost", { profile: "default" }));
    expect(await screen.findByText("USD 42.50")).toBeInTheDocument();
    expect(screen.getByText("EC2")).toBeInTheDocument();
  });

  it("invokes a Lambda function", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<AwsPanel status={status()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_instances", { profile: "default" }));

    fireEvent.click(screen.getByRole("button", { name: "Lambda" }));
    fireEvent.change(screen.getByPlaceholderText("my-function"), { target: { value: "my-fn" } });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "cloud_aws_lambda_invoke") return Promise.resolve('{"ok":true}');
      return Promise.resolve([]);
    });
    fireEvent.click(screen.getByText("Invoke"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_lambda_invoke", {
        profile: "default", functionName: "my-fn", payload: "{}",
      });
    });
    expect(await screen.findByText('{"ok":true}')).toBeInTheDocument();
  });

  it("runs an ECS exec command", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<AwsPanel status={status()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_instances", { profile: "default" }));

    fireEvent.click(screen.getByRole("button", { name: "ECS Exec" }));
    fireEvent.change(screen.getByPlaceholderText("my-cluster"), { target: { value: "prod-cluster" } });
    fireEvent.change(screen.getByPlaceholderText("task-id"), { target: { value: "task-1" } });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "cloud_aws_ecs_exec") return Promise.resolve("connected");
      return Promise.resolve([]);
    });
    fireEvent.click(screen.getAllByText("ECS Exec")[1]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_ecs_exec", {
        profile: "default", cluster: "prod-cluster", task: "task-1", container: undefined, command: "/bin/sh",
      });
    });
  });

  it("starts, stops, and connects to an instance", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "cloud_aws_list_instances") return Promise.resolve([instance()]);
      return Promise.resolve(undefined);
    });
    render(<AwsPanel status={status()} />);
    await screen.findByText("i-123");

    fireEvent.click(screen.getByTitle("Stop"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_stop_instance", { profile: "default", instanceId: "i-123" });
    });

    fireEvent.click(screen.getByTitle("Connect"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_connect_instance", { profile: "default", instanceId: "i-123" });
    });
  });

  it("SSO login and manual refresh", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "cloud_aws_list_instances") return Promise.resolve([instance()]);
      return Promise.resolve(undefined);
    });
    render(<AwsPanel status={status()} />);
    await screen.findByText("i-123");
    mockInvoke.mockClear();

    fireEvent.click(screen.getByText("SSO Login"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_sso_login", { profile: "default" });
    });

    fireEvent.click(screen.getByText("Refresh"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_instances", { profile: "default" });
    });
  });
});
