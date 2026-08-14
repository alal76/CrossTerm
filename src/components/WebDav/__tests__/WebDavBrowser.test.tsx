import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import WebDavBrowser from "@/components/WebDav/WebDavBrowser";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import type { WebDavConfig, WebDavEntry } from "@/types";

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

const config: WebDavConfig = {
  url: "http://192.168.0.9/dav",
  username: "user",
  password: "pw",
  verify_tls: false,
};

const entries: WebDavEntry[] = [
  { href: "/docs", name: "docs", entry_type: "collection" },
  { href: "/notes.txt", name: "notes.txt", entry_type: "file", content_length: 42 },
];

describe("WebDavBrowser", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("connects, lists the root, and renders folders and files", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);

    expect(await screen.findByText("docs")).toBeInTheDocument();
    expect(screen.getByText("notes.txt")).toBeInTheDocument();
    expect(screen.getByText("42 B")).toBeInTheDocument();
  });

  it("navigates into a folder on click and shows an Up button", async () => {
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      const path = (args as { path?: string } | undefined)?.path;
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") {
        if (path === "/docs") {
          return Promise.resolve([{ href: "/docs/readme.md", name: "readme.md", entry_type: "file" as const }]);
        }
        return Promise.resolve(entries);
      }
      return Promise.resolve(undefined);
    });

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("docs");

    fireEvent.click(screen.getByText("docs"));

    expect(await screen.findByText("readme.md")).toBeInTheDocument();
    expect(screen.getByText("Up")).toBeInTheDocument();
  });

  it("downloads a file via the save dialog + writeFile", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      if (cmd === "webdav_get") return Promise.resolve([1, 2, 3]);
      return Promise.resolve(undefined);
    });
    mockSave.mockResolvedValue("/local/notes.txt");

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("notes.txt");

    const downloadButtons = screen.getAllByTitle("Download");
    fireEvent.click(downloadButtons[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("webdav_get", { id: "conn-1", path: "/notes.txt" });
    });
    expect(mockWriteFile).toHaveBeenCalledWith("/local/notes.txt", new Uint8Array([1, 2, 3]));
  });

  it("disconnects on unmount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("docs");
    unmount();

    expect(mockInvoke).toHaveBeenCalledWith("webdav_disconnect", { id: "conn-1" });
  });
});
