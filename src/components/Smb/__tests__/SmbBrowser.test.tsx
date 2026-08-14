import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import SmbBrowser from "@/components/Smb/SmbBrowser";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { SmbConfig, SmbEntry } from "@/types";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-fs", () => ({
  readFile: vi.fn(),
  writeFile: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);
const mockOpen = vi.mocked(open);
const mockSave = vi.mocked(save);

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: SmbConfig = {
  host: "192.168.0.30",
  port: 445,
  username: "alal",
  password: "pw",
  domain: undefined,
  share: "public",
};

const entries: SmbEntry[] = [
  { name: "Documents", entry_type: "directory", size: 0 },
  { name: "notes.txt", entry_type: "file", size: 42 },
];

describe("SmbBrowser", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("connects, lists the root, and renders folders and files", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "smb_connect") return Promise.resolve("conn-1");
      if (cmd === "smb_list_dir") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });

    renderWithToast(<SmbBrowser sessionId="sess-1" config={config} />);

    expect(await screen.findByText("Documents")).toBeInTheDocument();
    expect(screen.getByText("notes.txt")).toBeInTheDocument();
    expect(screen.getByText("42 B")).toBeInTheDocument();
  });

  it("navigates into a folder on click and shows an Up button", async () => {
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      const path = (args as { path?: string } | undefined)?.path;
      if (cmd === "smb_connect") return Promise.resolve("conn-1");
      if (cmd === "smb_list_dir") {
        if (path === "Documents") {
          return Promise.resolve([{ name: "readme.md", entry_type: "file" as const, size: 10 }]);
        }
        return Promise.resolve(entries);
      }
      return Promise.resolve(undefined);
    });

    renderWithToast(<SmbBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("Documents");

    fireEvent.click(screen.getByText("Documents"));

    expect(await screen.findByText("readme.md")).toBeInTheDocument();
    expect(screen.getByText("Up")).toBeInTheDocument();
  });

  it("downloads a file via the save dialog straight to a local path", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "smb_connect") return Promise.resolve("conn-1");
      if (cmd === "smb_list_dir") return Promise.resolve(entries);
      if (cmd === "smb_get") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    mockSave.mockResolvedValue("/local/notes.txt");

    renderWithToast(<SmbBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("notes.txt");

    fireEvent.click(screen.getByTitle("Download"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("smb_get", {
        id: "conn-1",
        remotePath: "notes.txt",
        localPath: "/local/notes.txt",
      });
    });
  });

  it("uploads a file via the open dialog straight from a local path", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "smb_connect") return Promise.resolve("conn-1");
      if (cmd === "smb_list_dir") return Promise.resolve(entries);
      if (cmd === "smb_put") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    mockOpen.mockResolvedValue("/local/upload.txt");

    renderWithToast(<SmbBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("notes.txt");

    fireEvent.click(screen.getByText("Upload"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("smb_put", {
        id: "conn-1",
        localPath: "/local/upload.txt",
        remotePath: "upload.txt",
      });
    });
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "smb_connect") return Promise.resolve("conn-1");
      if (cmd === "smb_list_dir") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<SmbBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("Documents");
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("smb_disconnect", { id: "conn-1" });
  });

  it("shows a share picker and connects once a share is chosen when config.share is empty", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "smb_list_shares") return Promise.resolve(["public", "backups"]);
      if (cmd === "smb_connect") return Promise.resolve("conn-1");
      if (cmd === "smb_list_dir") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });

    renderWithToast(<SmbBrowser sessionId="sess-1" config={{ ...config, share: "" }} />);

    expect(await screen.findByText("public")).toBeInTheDocument();
    expect(screen.getByText("backups")).toBeInTheDocument();

    fireEvent.click(screen.getByText("public"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("smb_connect", {
        config: { ...config, share: "public" },
      });
    });
    expect(await screen.findByText("Documents")).toBeInTheDocument();
  });
});
