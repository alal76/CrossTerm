import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import DockerLogsViewer from "@/components/DockerLogs/DockerLogsViewer";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DockerLogsConfig } from "@/types";

Element.prototype.scrollIntoView = vi.fn();

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

const config: DockerLogsConfig = { host: "10.0.0.80", port: 2375, container_id: "abc123def456", tty: false, timestamps: false };

describe("DockerLogsViewer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("connects and shows the container id", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "docker_logs_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<DockerLogsViewer sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Docker Logs — abc123def456/)).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("docker_logs_connect", { config });
  });

  it("shows an error state when the connection fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "docker_logs_connect") return Promise.reject(new Error("Connection refused"));
      return Promise.resolve(undefined);
    });

    render(<DockerLogsViewer sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Couldn't stream logs/)).toBeInTheDocument();
    expect(screen.getByText(/Connection refused/)).toBeInTheDocument();
  });

  it("renders incoming log lines, marking stderr distinctly", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "docker_logs_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<DockerLogsViewer sessionId="sess-1" config={config} />);
    await screen.findByText(/Docker Logs/);

    handlers["docker_logs:line"]({ payload: { session_id: "conn-1", stream: "stdout", data: "server started" } });
    handlers["docker_logs:line"]({ payload: { session_id: "conn-1", stream: "stderr", data: "warning: low memory" } });

    expect(await screen.findByText("server started")).toBeInTheDocument();
    expect(await screen.findByText("warning: low memory")).toBeInTheDocument();
    expect(screen.getByText("[stderr]")).toBeInTheDocument();
  });

  it("ignores log lines for a different connection", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "docker_logs_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    render(<DockerLogsViewer sessionId="sess-1" config={config} />);
    await screen.findByText(/Docker Logs/);

    handlers["docker_logs:line"]({ payload: { session_id: "other-conn", stream: "stdout", data: "should not appear" } });
    await waitFor(() => expect(screen.queryByText("should not appear")).not.toBeInTheDocument());
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "docker_logs_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });

    const { unmount } = render(<DockerLogsViewer sessionId="sess-1" config={config} />);
    await screen.findByText(/Docker Logs/);
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("docker_logs_disconnect", { id: "conn-1" });
  });
});
