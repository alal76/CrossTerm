import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import WebDavBrowser from "@/components/WebDav/WebDavBrowser";
import { ToastProvider } from "@/components/Shared/Toast";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readFile, writeFile } from "@tauri-apps/plugin-fs";
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
const mockOpen = vi.mocked(open);
const mockReadFile = vi.mocked(readFile);

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

  it("shows a connecting state before the initial webdav_connect resolves", () => {
    mockInvoke.mockImplementation(() => new Promise(() => {})); // never resolves

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);

    expect(screen.getByText(/Connecting to/)).toBeInTheDocument();
  });

  it("shows an error state when webdav_connect fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.reject(new Error("connection refused"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);

    expect(await screen.findByText(/Couldn't reach/)).toBeInTheDocument();
    expect(screen.getByText(/connection refused/)).toBeInTheDocument();
  });

  it("shows an empty-folder message when the listing has no entries", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);

    expect(await screen.findByText("Empty folder.")).toBeInTheDocument();
  });

  it("navigates back up to the parent folder via the Up button", async () => {
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
    await screen.findByText("readme.md");

    fireEvent.click(screen.getByText("Up"));

    expect(await screen.findByText("docs")).toBeInTheDocument();
    expect(screen.queryByText("Up")).not.toBeInTheDocument();
  });

  it("refreshes the current listing via the Refresh button", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("docs");
    const listCallsBefore = mockInvoke.mock.calls.filter(([cmd]) => cmd === "webdav_list").length;

    fireEvent.click(screen.getByText("Refresh"));

    await waitFor(() => {
      const listCallsAfter = mockInvoke.mock.calls.filter(([cmd]) => cmd === "webdav_list").length;
      expect(listCallsAfter).toBe(listCallsBefore + 1);
    });
  });

  it("uploads a file via the open dialog + readFile, then refreshes the listing", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      if (cmd === "webdav_put") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    mockOpen.mockResolvedValue("/local/photo.png");
    mockReadFile.mockResolvedValue(new Uint8Array([9, 9, 9]));

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("docs");

    fireEvent.click(screen.getByText("Upload"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("webdav_put", {
        id: "conn-1",
        path: "/photo.png",
        content: [9, 9, 9],
        contentType: null,
      });
    });
    expect(await screen.findByText(/Uploaded photo.png/)).toBeInTheDocument();
  });

  it("does nothing when the upload file picker is cancelled", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });
    mockOpen.mockResolvedValue(null);

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("docs");

    fireEvent.click(screen.getByText("Upload"));

    await waitFor(() => expect(mockOpen).toHaveBeenCalled());
    expect(mockInvoke).not.toHaveBeenCalledWith("webdav_put", expect.anything());
  });

  it("shows an error toast when an upload fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      if (cmd === "webdav_put") return Promise.reject(new Error("disk full"));
      return Promise.resolve(undefined);
    });
    mockOpen.mockResolvedValue("/local/photo.png");
    mockReadFile.mockResolvedValue(new Uint8Array([1]));

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("docs");

    fireEvent.click(screen.getByText("Upload"));

    expect(await screen.findByText(/Upload failed: Error: disk full/)).toBeInTheDocument();
  });

  it("deletes a file after confirming, then refreshes the listing", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      if (cmd === "webdav_delete") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("notes.txt");

    const deleteButtons = screen.getAllByTitle("Delete");
    fireEvent.click(deleteButtons[1]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("webdav_delete", { id: "conn-1", path: "/notes.txt" });
    });
    expect(await screen.findByText(/Deleted notes.txt/)).toBeInTheDocument();
  });

  it("does not delete when the confirmation dialog is dismissed", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("notes.txt");

    const deleteButtons = screen.getAllByTitle("Delete");
    fireEvent.click(deleteButtons[1]);

    expect(window.confirm).toHaveBeenCalled();
    expect(mockInvoke).not.toHaveBeenCalledWith("webdav_delete", expect.anything());
  });

  it("shows an error toast when a download fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      if (cmd === "webdav_get") return Promise.reject(new Error("forbidden"));
      return Promise.resolve(undefined);
    });
    mockSave.mockResolvedValue("/local/notes.txt");

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("notes.txt");

    fireEvent.click(screen.getAllByTitle("Download")[0]);

    expect(await screen.findByText(/Download failed: Error: forbidden/)).toBeInTheDocument();
  });

  it("does nothing when the download save dialog is cancelled", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      return Promise.resolve(undefined);
    });
    mockSave.mockResolvedValue(null);

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("notes.txt");

    fireEvent.click(screen.getAllByTitle("Download")[0]);

    await waitFor(() => expect(mockSave).toHaveBeenCalled());
    expect(mockInvoke).not.toHaveBeenCalledWith("webdav_get", expect.anything());
  });

  it("keeps the current listing visible (without crashing) when a refresh's webdav_list call fails", async () => {
    let listCallCount = 0;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") {
        listCallCount += 1;
        return listCallCount === 1 ? Promise.resolve(entries) : Promise.reject(new Error("timed out"));
      }
      return Promise.resolve(undefined);
    });

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("docs");

    fireEvent.click(screen.getByText("Refresh"));

    await waitFor(() => {
      const calls = mockInvoke.mock.calls.filter(([cmd]) => cmd === "webdav_list");
      expect(calls).toHaveLength(2);
    });
    // The failed refresh doesn't clear out the previously-loaded entries.
    expect(screen.getByText("docs")).toBeInTheDocument();
  });

  it("disconnects the pending connection if unmounted before webdav_connect resolves", async () => {
    let resolveConnect: (id: string) => void = () => {};
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") {
        return new Promise((resolve) => {
          resolveConnect = resolve;
        });
      }
      return Promise.resolve(undefined);
    });

    const { unmount } = renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    unmount();
    resolveConnect("conn-1");

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("webdav_disconnect", { id: "conn-1" });
    });
  });

  it("shows an error toast when a delete fails", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "webdav_connect") return Promise.resolve("conn-1");
      if (cmd === "webdav_list") return Promise.resolve(entries);
      if (cmd === "webdav_delete") return Promise.reject(new Error("locked"));
      return Promise.resolve(undefined);
    });

    renderWithToast(<WebDavBrowser sessionId="sess-1" config={config} />);
    await screen.findByText("notes.txt");

    fireEvent.click(screen.getAllByTitle("Delete")[1]);

    expect(await screen.findByText(/Delete failed: Error: locked/)).toBeInTheDocument();
  });
});
