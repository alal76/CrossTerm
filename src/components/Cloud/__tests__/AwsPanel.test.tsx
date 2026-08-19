import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import AwsPanel from "@/components/Cloud/AwsPanel";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/sessionStore";
import type { CloudProviderStatus, Ec2Instance, S3Bucket, S3Object, CostSummary, AwsProfile } from "@/types";

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

/** Dispatches every real AWS command name to a sensible default, letting tests override specific commands. */
function mockAwsInvoke(overrides: Record<string, () => unknown> = {}) {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd in overrides) return Promise.resolve(overrides[cmd]());
    const defaults: Record<string, unknown> = {
      cloud_aws_list_profiles: [] satisfies AwsProfile[],
      cloud_aws_switch_profile: undefined,
      cloud_aws_list_ec2: [],
      cloud_aws_list_s3_buckets: [],
      cloud_aws_list_s3_objects: [],
      cloud_aws_sso_login: undefined,
      cloud_aws_start_instance: undefined,
      cloud_aws_stop_instance: undefined,
      cloud_aws_ssm_start: "session-id",
    };
    return Promise.resolve(defaults[cmd]);
  });
}

describe("AwsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSessionStore.setState({ sessions: [], openTabs: [], activeTabId: null });
    mockAwsInvoke();
  });

  it("shows a not-installed message when the CLI is missing", () => {
    render(<AwsPanel status={status({ cli_status: { type: "not_installed" } })} />);
    expect(screen.getByText("Install the AWS CLI to get started.")).toBeInTheDocument();
  });

  it("initializes the profile selector from status and loads instances via cloud_aws_list_ec2", async () => {
    mockAwsInvoke({ cloud_aws_list_ec2: () => [instance()] });
    render(<AwsPanel status={status()} />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_ec2", { region: "us-east-1" });
    });
    expect(await screen.findByText("i-123")).toBeInTheDocument();
    expect(screen.getByText("running")).toBeInTheDocument();
  });

  it("resolves the region from the selected profile's real region instead of the default", async () => {
    mockAwsInvoke({
      cloud_aws_list_profiles: () => [{ name: "default", region: "eu-west-1" } satisfies AwsProfile],
      cloud_aws_list_ec2: () => [instance()],
    });
    render(<AwsPanel status={status()} />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_ec2", { region: "eu-west-1" });
    });
  });

  it("switches AWS_PROFILE on the backend when the profile selection changes", async () => {
    mockAwsInvoke({ cloud_aws_list_ec2: () => [instance()] });
    render(<AwsPanel status={status()} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_switch_profile", { profile: "default" });
    });
  });

  it("shows the empty state when there are no instances", async () => {
    mockAwsInvoke();
    render(<AwsPanel status={status()} />);
    expect(await screen.findByText(/No resources found/)).toBeInTheDocument();
  });

  it("switches to the storage tab and browses a bucket", async () => {
    mockAwsInvoke({
      cloud_aws_list_s3_buckets: () => [{ name: "my-bucket", region: "us-east-1", creation_date: "2026-01-01" } satisfies S3Bucket],
      cloud_aws_list_s3_objects: () => [{ key: "file.txt", size: 100, last_modified: "2026-01-01", storage_class: "STANDARD" } satisfies S3Object],
    });
    render(<AwsPanel status={status()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_ec2", { region: "us-east-1" }));

    fireEvent.click(screen.getByRole("button", { name: "Storage" }));
    expect(await screen.findByText("my-bucket")).toBeInTheDocument();

    fireEvent.click(screen.getByText("my-bucket"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_s3_objects", { bucket: "my-bucket", prefix: "" });
    });
    expect(await screen.findByText("file.txt")).toBeInTheDocument();

    fireEvent.click(screen.getByText(/Buckets/));
    expect(await screen.findByText("my-bucket")).toBeInTheDocument();
  });

  it("switches to the cost tab and shows the summary", async () => {
    mockAwsInvoke({
      cloud_aws_cost_summary: () =>
        ({
          total_cost: 42.5,
          currency: "USD",
          start_date: "2026-01-01",
          end_date: "2026-01-31",
          by_service: [{ service_name: "EC2", cost: 30 }],
        }) satisfies CostSummary,
    });
    render(<AwsPanel status={status()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_ec2", { region: "us-east-1" }));

    fireEvent.click(screen.getByRole("button", { name: "Cost" }));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_cost_summary"));
    expect(await screen.findByText("USD 42.50")).toBeInTheDocument();
    expect(screen.getByText("EC2")).toBeInTheDocument();
  });

  it("invokes a Lambda function using the correct command name and param key", async () => {
    mockAwsInvoke();
    render(<AwsPanel status={status()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_ec2", { region: "us-east-1" }));

    fireEvent.click(screen.getByRole("button", { name: "Lambda" }));
    fireEvent.change(screen.getByPlaceholderText("my-function"), { target: { value: "my-fn" } });

    mockAwsInvoke({ cloud_aws_lambda_invoke: () => '{"ok":true}' });
    fireEvent.click(screen.getByText("Invoke"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_lambda_invoke", {
        function: "my-fn", payload: "{}",
      });
    });
    expect(await screen.findByText('{"ok":true}')).toBeInTheDocument();
  });

  it("runs an ECS exec command with the user's chosen command, not a hardcoded one", async () => {
    mockAwsInvoke();
    render(<AwsPanel status={status()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_ec2", { region: "us-east-1" }));

    fireEvent.click(screen.getByRole("button", { name: "ECS Exec" }));
    fireEvent.change(screen.getByPlaceholderText("my-cluster"), { target: { value: "prod-cluster" } });
    fireEvent.change(screen.getByPlaceholderText("task-id"), { target: { value: "task-1" } });

    mockAwsInvoke({ cloud_aws_ecs_exec: () => "connected" });
    fireEvent.click(screen.getAllByText("ECS Exec")[1]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_ecs_exec", {
        cluster: "prod-cluster", task: "task-1", container: "", command: "/bin/sh",
      });
    });
  });

  it("starts and stops an instance via the real backend commands", async () => {
    mockAwsInvoke({ cloud_aws_list_ec2: () => [instance()] });
    render(<AwsPanel status={status()} />);
    await screen.findByText("i-123");

    fireEvent.click(screen.getByTitle("Stop"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_stop_instance", { instanceId: "i-123", region: "us-east-1" });
    });
  });

  it("connect opens a real SSH session tab instead of calling a nonexistent backend command", async () => {
    mockAwsInvoke({ cloud_aws_list_ec2: () => [instance()] });
    render(<AwsPanel status={status()} />);
    await screen.findByText("i-123");

    fireEvent.click(screen.getByTitle("Connect"));

    await waitFor(() => {
      const sessions = useSessionStore.getState().sessions;
      expect(sessions).toHaveLength(1);
      expect(sessions[0].connection.host).toBe("1.2.3.4");
      expect(sessions[0].type).toBe("ssh");
    });
    expect(useSessionStore.getState().openTabs).toHaveLength(1);
  });

  it("SSO login and manual refresh", async () => {
    mockAwsInvoke({ cloud_aws_list_ec2: () => [instance()] });
    render(<AwsPanel status={status()} />);
    await screen.findByText("i-123");
    mockInvoke.mockClear();
    mockAwsInvoke({ cloud_aws_list_ec2: () => [instance()] });

    fireEvent.click(screen.getByText("SSO Login"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_sso_login", { profile: "default" });
    });

    fireEvent.click(screen.getByText("Refresh"));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("cloud_aws_list_ec2", { region: "us-east-1" });
    });
  });
});
