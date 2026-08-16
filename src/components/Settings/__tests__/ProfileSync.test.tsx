import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import "@/i18n";
import ProfileSync from "@/components/Settings/ProfileSync";
import { ToastProvider } from "@/components/Shared/Toast";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
  readFile: vi.fn(),
  writeFile: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);

function renderWithToast() {
  return render(<ToastProvider><ProfileSync /></ToastProvider>);
}

describe("ProfileSync", () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sync_get_status") return Promise.resolve({});
      return Promise.resolve(undefined);
    });
  });

  it("Export and Import are disabled until a password is entered", () => {
    renderWithToast();
    expect(screen.getByText("Export Settings")).toBeDisabled();
    expect(screen.getByText("Import Settings")).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Bundle password"), { target: { value: "hunter2" } });
    expect(screen.getByText("Export Settings")).not.toBeDisabled();
    expect(screen.getByText("Import Settings")).not.toBeDisabled();
  });

  // Regression coverage: sync_export used to return an always-empty,
  // XOR-"encrypted" bundle regardless of what invoke args were passed.
  // The real command needs a password, and the frontend never sent one.
  it("Export sends the entered password and writes the real bytes returned by the backend", async () => {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const { writeFile } = await import("@tauri-apps/plugin-fs");
    vi.mocked(save).mockResolvedValue("/tmp/bundle.ctbundle");
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sync_get_status") return Promise.resolve({});
      if (cmd === "sync_export") return Promise.resolve([1, 2, 3, 4]);
      return Promise.resolve(undefined);
    });

    renderWithToast();
    fireEvent.change(screen.getByLabelText("Bundle password"), { target: { value: "hunter2" } });
    fireEvent.click(screen.getByText("Export Settings"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("sync_export", { password: "hunter2" });
    });
    await waitFor(() => {
      expect(writeFile).toHaveBeenCalledWith("/tmp/bundle.ctbundle", new Uint8Array([1, 2, 3, 4]));
    });
    expect(await screen.findByText("Profile bundle exported.")).toBeInTheDocument();
  });

  it("shows an error toast when export fails instead of silently swallowing it", async () => {
    const { save } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(save).mockResolvedValue("/tmp/bundle.ctbundle");
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sync_get_status") return Promise.resolve({});
      if (cmd === "sync_export") return Promise.reject(new Error("disk full"));
      return Promise.resolve(undefined);
    });

    renderWithToast();
    fireEvent.change(screen.getByLabelText("Bundle password"), { target: { value: "hunter2" } });
    fireEvent.click(screen.getByText("Export Settings"));

    expect(await screen.findByText(/Export failed: Error: disk full/)).toBeInTheDocument();
  });

  // Regression coverage: sync_import used to decrypt-and-discard the
  // bundle ("In a full implementation, apply bundle settings to the app")
  // — nothing was ever actually imported, and the frontend never even
  // reported how much (or how little) happened.
  it("Import sends the password and shows a summary of what was actually imported", async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const { readFile } = await import("@tauri-apps/plugin-fs");
    vi.mocked(open).mockResolvedValue("/tmp/bundle.ctbundle");
    vi.mocked(readFile).mockResolvedValue(new Uint8Array([9, 9, 9]));
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sync_get_status") return Promise.resolve({});
      if (cmd === "sync_import") {
        return Promise.resolve({ sessions_imported: 3, snippets_imported: 2, settings_applied: true });
      }
      return Promise.resolve(undefined);
    });

    renderWithToast();
    fireEvent.change(screen.getByLabelText("Bundle password"), { target: { value: "hunter2" } });
    fireEvent.click(screen.getByText("Import Settings"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("sync_import", {
        data: [9, 9, 9],
        password: "hunter2",
      });
    });
    expect(
      await screen.findByText("Imported 3 session(s) and 2 snippet(s), and applied settings."),
    ).toBeInTheDocument();
  });

  it("shows an error toast when import fails (e.g. wrong password)", async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const { readFile } = await import("@tauri-apps/plugin-fs");
    vi.mocked(open).mockResolvedValue("/tmp/bundle.ctbundle");
    vi.mocked(readFile).mockResolvedValue(new Uint8Array([9, 9, 9]));
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sync_get_status") return Promise.resolve({});
      if (cmd === "sync_import") return Promise.reject(new Error("wrong password, or the bundle is corrupted"));
      return Promise.resolve(undefined);
    });

    renderWithToast();
    fireEvent.change(screen.getByLabelText("Bundle password"), { target: { value: "wrong" } });
    fireEvent.click(screen.getByText("Import Settings"));

    expect(await screen.findByText(/Import failed:.*wrong password/)).toBeInTheDocument();
  });

  it("shows last export/import status from sync_get_status", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "sync_get_status") {
        return Promise.resolve({ last_export: "2026-01-01T00:00:00Z", last_import: "2026-01-02T00:00:00Z" });
      }
      return Promise.resolve(undefined);
    });
    renderWithToast();

    expect(await screen.findByText(/2026-01-01T00:00:00Z/)).toBeInTheDocument();
    expect(screen.getByText(/2026-01-02T00:00:00Z/)).toBeInTheDocument();
  });
});
