import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import "@/i18n";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "@/stores/sessionStore";
import { useTerminalStore } from "@/stores/terminalStore";
import { useAppStore } from "@/stores/appStore";
import { ConnectionStatus, SessionType } from "@/types";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

import RemoteFileBrowser from "@/components/RemoteFiles/RemoteFileBrowser";

const mockInvoke = vi.mocked(invoke);
const mockOpen = vi.mocked(open);
const mockSave = vi.mocked(save);

function connectSshTab() {
  useSessionStore.setState({
    activeTabId: "tab-1",
    openTabs: [
      {
        id: "tab-1",
        sessionId: "sess-1",
        sessionType: SessionType.SSH,
        status: ConnectionStatus.Connected,
        order: 0,
      } as never,
    ],
  });
  useTerminalStore.setState({
    terminals: new Map([
      ["conn-1", { id: "conn-1", sessionId: "sess-1", status: ConnectionStatus.Connected, cols: 80, rows: 24, title: "" }],
    ]),
  });
}

describe("RemoteFileBrowser", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({ remoteFilesFollowTerminal: false });
    useSessionStore.setState({ activeTabId: null, openTabs: [] });
    useTerminalStore.setState({ terminals: new Map() });
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sftp_open") return Promise.resolve("sftp-1");
      if (cmd === "ssh_exec") return Promise.resolve("/home/user");
      if (cmd === "sftp_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
  });

  it("shows the no-connection state when there is no active SSH tab", () => {
    render(<RemoteFileBrowser />);
    expect(screen.getByText("No active SSH connection")).toBeInTheDocument();
    expect(screen.getByText("Connect to an SSH session to browse remote files")).toBeInTheDocument();
  });

  it("opens an SFTP session and lists the home directory once an SSH tab connects", async () => {
    connectSshTab();
    render(<RemoteFileBrowser />);

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("sftp_open", { connectionId: "conn-1" }));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("sftp_list", { sessionId: "sftp-1", path: "/home/user" }),
    );
  });

  it("renders files and folders sorted with directories first", async () => {
    connectSshTab();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sftp_open") return Promise.resolve("sftp-1");
      if (cmd === "ssh_exec") return Promise.resolve("/home/user");
      if (cmd === "sftp_list") {
        return Promise.resolve([
          { name: "readme.txt", is_dir: false, size: 2048, modified: null, permissions: null },
          { name: "projects", is_dir: true, size: 0, modified: null, permissions: null },
        ]);
      }
      return Promise.resolve(undefined);
    });
    render(<RemoteFileBrowser />);

    expect(await screen.findByText("projects")).toBeInTheDocument();
    expect(screen.getByText("readme.txt")).toBeInTheDocument();
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();

    // Directories are sorted before files regardless of name.
    const names = screen.getAllByText(/^(projects|readme\.txt)$/).map((el) => el.textContent);
    expect(names).toEqual(["projects", "readme.txt"]);
  });

  it("shows the empty-directory message when the listing is empty", async () => {
    connectSshTab();
    render(<RemoteFileBrowser />);
    expect(await screen.findByText("This directory is empty.")).toBeInTheDocument();
  });

  it("expands a directory to load its children on click", async () => {
    connectSshTab();
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "sftp_open") return Promise.resolve("sftp-1");
      if (cmd === "ssh_exec") return Promise.resolve("/home/user");
      if (cmd === "sftp_list") {
        const path = (args as { path?: string } | undefined)?.path;
        if (path === "/home/user") {
          return Promise.resolve([{ name: "projects", is_dir: true, size: 0, modified: null, permissions: null }]);
        }
        if (path === "/home/user/projects") {
          return Promise.resolve([{ name: "app.ts", is_dir: false, size: 512, modified: null, permissions: null }]);
        }
        return Promise.resolve([]);
      }
      return Promise.resolve(undefined);
    });
    render(<RemoteFileBrowser />);

    fireEvent.click(await screen.findByText("projects"));
    expect(await screen.findByText("app.ts")).toBeInTheDocument();
  });

  it("navigates into a directory on double-click and updates the breadcrumb", async () => {
    connectSshTab();
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "sftp_open") return Promise.resolve("sftp-1");
      if (cmd === "ssh_exec") return Promise.resolve("/home/user");
      if (cmd === "sftp_list") {
        const path = (args as { path?: string } | undefined)?.path;
        if (path === "/home/user") {
          return Promise.resolve([{ name: "projects", is_dir: true, size: 0, modified: null, permissions: null }]);
        }
        return Promise.resolve([]);
      }
      return Promise.resolve(undefined);
    });
    render(<RemoteFileBrowser />);

    fireEvent.doubleClick(await screen.findByText("projects"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("sftp_list", { sessionId: "sftp-1", path: "/home/user/projects" }),
    );
    expect(screen.getByText("user")).toBeInTheDocument();
    expect(screen.getByText("projects")).toBeInTheDocument();
  });

  it("navigates to the parent directory via the up button", async () => {
    connectSshTab();
    render(<RemoteFileBrowser />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("sftp_open", expect.anything()));

    // Home and Up buttons share the same title ("Go to parent"); Up is the second.
    fireEvent.click(screen.getAllByTitle("Go to parent")[1]);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("sftp_list", { sessionId: "sftp-1", path: "/home" }),
    );
  });

  it("navigates home via the home button", async () => {
    connectSshTab();
    render(<RemoteFileBrowser />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("sftp_open", expect.anything()));

    fireEvent.click(screen.getAllByTitle("Go to parent")[0]);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("sftp_list", { sessionId: "sftp-1", path: "/" }));
  });

  it("refreshes the current directory listing", async () => {
    connectSshTab();
    render(<RemoteFileBrowser />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("sftp_open", expect.anything()));
    mockInvoke.mockClear();

    fireEvent.click(screen.getByTitle("Refresh"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("sftp_list", { sessionId: "sftp-1", path: "/home/user" }),
    );
  });

  it("uploads a file picked from the dialog and refreshes the listing", async () => {
    connectSshTab();
    mockOpen.mockResolvedValue("/local/photo.png");
    render(<RemoteFileBrowser />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("sftp_open", expect.anything()));

    fireEvent.click(screen.getByTitle("Upload file"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("sftp_upload", {
        sessionId: "sftp-1",
        localPath: "/local/photo.png",
        remotePath: "/home/user/photo.png",
      }),
    );
  });

  it("does nothing when the upload dialog is cancelled", async () => {
    connectSshTab();
    mockOpen.mockResolvedValue(null);
    render(<RemoteFileBrowser />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("sftp_open", expect.anything()));
    mockInvoke.mockClear();

    fireEvent.click(screen.getByTitle("Upload file"));
    await waitFor(() => expect(mockOpen).toHaveBeenCalled());
    expect(mockInvoke).not.toHaveBeenCalledWith("sftp_upload", expect.anything());
  });

  it("downloads a file to the chosen destination", async () => {
    connectSshTab();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sftp_open") return Promise.resolve("sftp-1");
      if (cmd === "ssh_exec") return Promise.resolve("/home/user");
      if (cmd === "sftp_list") {
        return Promise.resolve([{ name: "readme.txt", is_dir: false, size: 100, modified: null, permissions: null }]);
      }
      return Promise.resolve(undefined);
    });
    mockSave.mockResolvedValue("/local/readme.txt");
    render(<RemoteFileBrowser />);

    const row = (await screen.findByText("readme.txt")).closest("button")!;
    fireEvent.click(row.querySelector('[title="Download"]')!);

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("sftp_download", {
        sessionId: "sftp-1",
        remotePath: "/home/user/readme.txt",
        localPath: "/local/readme.txt",
      }),
    );
  });

  it("shows an error message when opening the SFTP session fails", async () => {
    connectSshTab();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sftp_open") return Promise.reject(new Error("permission denied"));
      return Promise.resolve(undefined);
    });
    render(<RemoteFileBrowser />);
    expect(await screen.findByText("Error: permission denied")).toBeInTheDocument();
  });

  it("closes the SFTP session when the SSH connection disappears", async () => {
    connectSshTab();
    render(<RemoteFileBrowser />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("sftp_open", expect.anything()));

    act(() => {
      useSessionStore.setState({ activeTabId: null, openTabs: [] });
    });
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("sftp_close", { sessionId: "sftp-1" }));
    expect(screen.getByText("No active SSH connection")).toBeInTheDocument();
  });

  it("polls the remote cwd and follows the terminal when enabled", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    useAppStore.setState({ remoteFilesFollowTerminal: true });
    connectSshTab();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sftp_open") return Promise.resolve("sftp-1");
      if (cmd === "ssh_exec") return Promise.resolve("/home/user");
      if (cmd === "sftp_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<RemoteFileBrowser />);
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_exec") return Promise.resolve("/var/log");
      if (cmd === "sftp_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(screen.getByText("log")).toBeInTheDocument();
    vi.useRealTimers();
  });
});
