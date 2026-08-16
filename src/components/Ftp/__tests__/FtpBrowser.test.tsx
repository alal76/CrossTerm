import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import FtpBrowser from "@/components/Ftp/FtpBrowser";
import { invoke } from "@tauri-apps/api/core";
import type { FtpEntry } from "@/types";

const mockInvoke = vi.mocked(invoke);

function entry(overrides: Partial<FtpEntry> = {}): FtpEntry {
  return { name: "file.txt", size: 100, entry_type: "file", ...overrides };
}

async function connect() {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "ftp_connect") return Promise.resolve("conn-1");
    if (cmd === "ftp_list") return Promise.resolve([entry()]);
    return Promise.resolve(undefined);
  });
  render(<FtpBrowser />);
  fireEvent.change(screen.getByPlaceholderText("ftp.example.com"), { target: { value: "ftp.test.com" } });
  fireEvent.click(screen.getByText("Connect"));
  await screen.findByText("file.txt");
}

describe("FtpBrowser", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the connection form when not connected", () => {
    render(<FtpBrowser />);
    expect(screen.getByPlaceholderText("ftp.example.com")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Connect/ })).toBeDisabled();
  });

  it("connects and lists the root directory", async () => {
    await connect();
    expect(mockInvoke).toHaveBeenCalledWith("ftp_connect", {
      config: expect.objectContaining({ host: "ftp.test.com", port: 21 }),
    });
    expect(mockInvoke).toHaveBeenCalledWith("ftp_list", { connId: "conn-1", path: "/" });
  });

  it("navigates into a directory on double-click", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ftp_connect") return Promise.resolve("conn-1");
      if (cmd === "ftp_list") return Promise.resolve([entry({ name: "sub", entry_type: "directory" })]);
      return Promise.resolve(undefined);
    });
    render(<FtpBrowser />);
    fireEvent.change(screen.getByPlaceholderText("ftp.example.com"), { target: { value: "ftp.test.com" } });
    fireEvent.click(screen.getByText("Connect"));
    await screen.findByText("sub");

    fireEvent.doubleClick(screen.getByText("sub").closest("tr")!);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("ftp_list", { connId: "conn-1", path: "/sub" });
    });
  });

  it("deletes an entry and reloads the directory", async () => {
    await connect();
    mockInvoke.mockClear();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ftp_delete") return Promise.resolve(undefined);
      if (cmd === "ftp_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    fireEvent.click(screen.getByTitle("Delete"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("ftp_delete", { connId: "conn-1", path: "/file.txt" });
      expect(mockInvoke).toHaveBeenCalledWith("ftp_list", { connId: "conn-1", path: "/" });
    });
  });

  it("shows the empty-directory message", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ftp_connect") return Promise.resolve("conn-1");
      if (cmd === "ftp_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    render(<FtpBrowser />);
    fireEvent.change(screen.getByPlaceholderText("ftp.example.com"), { target: { value: "ftp.test.com" } });
    fireEvent.click(screen.getByText("Connect"));
    expect(await screen.findByText("This directory is empty.")).toBeInTheDocument();
  });

  it("disconnects and returns to the connection form", async () => {
    await connect();
    mockInvoke.mockClear();
    mockInvoke.mockResolvedValue(undefined);

    fireEvent.click(screen.getByText("Disconnect"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("ftp_disconnect", { connId: "conn-1" });
    });
    expect(await screen.findByPlaceholderText("ftp.example.com")).toBeInTheDocument();
  });

  it("creates a new folder via the prompt", async () => {
    await connect();
    mockInvoke.mockClear();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ftp_mkdir") return Promise.resolve(undefined);
      if (cmd === "ftp_list") return Promise.resolve([entry({ name: "newdir", entry_type: "directory" })]);
      return Promise.resolve(undefined);
    });
    const promptSpy = vi.spyOn(globalThis, "prompt").mockReturnValue("newdir");

    fireEvent.click(screen.getByText("New Folder"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("ftp_mkdir", { connId: "conn-1", path: "/newdir" });
    });
    promptSpy.mockRestore();
  });
});
