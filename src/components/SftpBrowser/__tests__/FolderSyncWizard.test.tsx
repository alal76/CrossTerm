import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import FolderSyncWizard from "@/components/SftpBrowser/FolderSyncWizard";
import { invoke } from "@tauri-apps/api/core";
import type { SyncEntry, SyncResult } from "@/types";

const mockInvoke = vi.mocked(invoke);

function syncEntry(overrides: Partial<SyncEntry> = {}): SyncEntry {
  return { path: "/project/main.rs", sync_action: "upload", size: 2048, ...overrides };
}

async function fillDirsAndCompare(entries: SyncEntry[]) {
  mockInvoke.mockResolvedValue(entries);
  render(<FolderSyncWizard sessionId="s1" onClose={vi.fn()} />);

  fireEvent.change(screen.getByPlaceholderText("/home/user/project"), { target: { value: "/local/project" } });
  fireEvent.change(screen.getByPlaceholderText("/var/www/project"), { target: { value: "/remote/project" } });
  fireEvent.click(screen.getByRole("button", { name: "Compare" }));
  await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("sftp_sync_compare", {
    sessionId: "s1", localDir: "/local/project", remoteDir: "/remote/project",
  }));
}

describe("FolderSyncWizard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("Compare is disabled until both directories are filled in", () => {
    render(<FolderSyncWizard sessionId="s1" onClose={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Compare" })).toBeDisabled();
  });

  it("compares directories and lists changed entries", async () => {
    await fillDirsAndCompare([syncEntry()]);
    expect(await screen.findByText("/project/main.rs")).toBeInTheDocument();
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
  });

  it("shows the no-changes message when nothing differs", async () => {
    await fillDirsAndCompare([]);
    expect(await screen.findByText("No changes detected")).toBeInTheDocument();
  });

  it("cycles a row's sync action through upload → download → skip", async () => {
    await fillDirsAndCompare([syncEntry()]);
    await screen.findByText("/project/main.rs");

    // sync_action starts as "upload", which has no translation entry
    // (only download/skip/conflict do) — falls back to the raw i18n key.
    const actionButton = screen.getByText("sftp.upload").closest("button")!;
    fireEvent.click(actionButton);
    expect(await screen.findByText("Download")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Download").closest("button")!);
    expect(await screen.findByText("Skip")).toBeInTheDocument();
  });

  it("shows an error when comparison fails", async () => {
    mockInvoke.mockRejectedValue(new Error("sftp session closed"));
    render(<FolderSyncWizard sessionId="s1" onClose={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText("/home/user/project"), { target: { value: "/a" } });
    fireEvent.change(screen.getByPlaceholderText("/var/www/project"), { target: { value: "/b" } });
    fireEvent.click(screen.getByRole("button", { name: "Compare" }));

    expect(await screen.findByText(/sftp session closed/)).toBeInTheDocument();
  });

  it("Back from review returns to directory selection", async () => {
    await fillDirsAndCompare([syncEntry()]);
    await screen.findByText("/project/main.rs");
    fireEvent.click(screen.getByText("Back"));
    expect(await screen.findByPlaceholderText("/home/user/project")).toBeInTheDocument();
  });

  it("executes the sync and shows results", async () => {
    await fillDirsAndCompare([syncEntry()]);
    await screen.findByText("/project/main.rs");

    const result: SyncResult = { uploaded: 1, downloaded: 0, skipped: 0, errors: [] };
    mockInvoke.mockResolvedValue(result);
    fireEvent.click(screen.getByText("Sync"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "sftp_sync_execute",
        expect.objectContaining({ sessionId: "s1", localDir: "/local/project", remoteDir: "/remote/project" }),
      );
    });
    expect(await screen.findByText("Sync Complete")).toBeInTheDocument();
  });

  it("shows errors from a failed sync execution", async () => {
    await fillDirsAndCompare([syncEntry()]);
    await screen.findByText("/project/main.rs");
    mockInvoke.mockRejectedValue(new Error("disk full"));
    fireEvent.click(screen.getByText("Sync"));
    expect(await screen.findByText(/disk full/)).toBeInTheDocument();
  });

  it("close button in the header calls onClose", () => {
    const onClose = vi.fn();
    render(<FolderSyncWizard sessionId="s1" onClose={onClose} />);
    fireEvent.click(screen.getByText("✕"));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
