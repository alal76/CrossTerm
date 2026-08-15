import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import NfsExplorer from "@/components/Nfs/NfsExplorer";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import type { NfsConfig, NfsEntry } from "@/types";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-fs", () => ({
  readFile: vi.fn(),
  writeFile: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);
const mockSave = vi.mocked(save);
const mockWriteFile = vi.mocked(writeFile);

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const config: NfsConfig = {
  host: "10.0.0.20",
  export_path: "/srv/nfs/share",
  uid: 1000,
  gid: 1000,
};

const entries: NfsEntry[] = [
  { name: "docs", file_type: "directory", size: 0, mode: 0o755 },
  { name: "notes.txt", file_type: "regular", size: 3, mode: 0o644 },
];

describe("NfsExplorer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("connects, lists the root, and renders directories and files", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "nfs_connect") return Promise.resolve("conn-1");
      if (cmd === "nfs_list") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });

    renderWithToast(<NfsExplorer sessionId="sess-1" config={config} />);

    expect(await screen.findByText("docs")).toBeInTheDocument();
    expect(screen.getByText("notes.txt")).toBeInTheDocument();
    expect(screen.getByText("3 B")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("nfs_connect", { config });
  });

  it("navigates into a directory on click and shows an Up button", async () => {
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      const path = (args as { path?: string } | undefined)?.path;
      if (cmd === "nfs_connect") return Promise.resolve("conn-1");
      if (cmd === "nfs_list") {
        if (path === "docs") {
          return Promise.resolve([{ name: "readme.md", file_type: "regular", size: 10, mode: 0o644 }]);
        }
        return Promise.resolve(entries);
      }
      return Promise.resolve(undefined);
    });

    renderWithToast(<NfsExplorer sessionId="sess-1" config={config} />);
    await screen.findByText("docs");

    fireEvent.click(screen.getByText("docs"));

    expect(await screen.findByText("readme.md")).toBeInTheDocument();
    expect(screen.getByText("Up")).toBeInTheDocument();
  });

  it("downloads a file via nfs_read + the save dialog + writeFile", async () => {
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === "nfs_connect") return Promise.resolve("conn-1");
      if (cmd === "nfs_list") return Promise.resolve(entries);
      if (cmd === "nfs_read") {
        const a = args as { path?: string; offset?: number; count?: number };
        expect(a.path).toBe("notes.txt");
        expect(a.offset).toBe(0);
        expect(a.count).toBe(3);
        return Promise.resolve([7, 8, 9]);
      }
      return Promise.resolve(undefined);
    });
    mockSave.mockResolvedValue("/local/notes.txt");

    renderWithToast(<NfsExplorer sessionId="sess-1" config={config} />);
    await screen.findByText("notes.txt");

    fireEvent.click(screen.getByTitle("Download"));

    await waitFor(() => {
      expect(mockWriteFile).toHaveBeenCalledWith("/local/notes.txt", new Uint8Array([7, 8, 9]));
    });
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "nfs_connect") return Promise.resolve("conn-1");
      if (cmd === "nfs_list") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<NfsExplorer sessionId="sess-1" config={config} />);
    await screen.findByText("docs");
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("nfs_disconnect", { id: "conn-1" });
  });

  it("shows an error state when the mount fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "nfs_connect") return Promise.reject(new Error("Mount failed: permission denied (status 13)"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<NfsExplorer sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Couldn't mount/)).toBeInTheDocument();
    expect(screen.getByText(/permission denied/)).toBeInTheDocument();
  });
});
