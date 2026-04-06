import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import "@/i18n";
import SftpBrowser from "@/components/SftpBrowser/SftpBrowser";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);

const MOCK_ENTRIES = [
  { name: "docs", is_dir: true, size: 0, modified: "2025-01-01T00:00:00Z", permissions: "drwxr-xr-x" },
  { name: "src", is_dir: true, size: 0, modified: "2025-02-15T12:00:00Z", permissions: "drwxr-xr-x" },
  { name: "README.md", is_dir: false, size: 1024, modified: "2025-03-10T08:30:00Z", permissions: "-rw-r--r--" },
  { name: "config.json", is_dir: false, size: 256, modified: "2025-04-01T14:00:00Z", permissions: "-rw-r--r--" },
];

describe("SftpBrowser", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // FT-C-26: Renders dual panes with file listings
  it("FT-C-26: renders dual panes with file listings when connected", async () => {
    // sftp_open returns a session ID
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "sftp_open") return "sftp-session-1";
      if (cmd === "sftp_list") return MOCK_ENTRIES;
      return undefined;
    });

    render(<SftpBrowser connectionId="conn-1" />);

    // Wait for SFTP session to open and files to load
    await waitFor(() => {
      expect(screen.getByText("README.md")).toBeInTheDocument();
    });

    // Should have both local and remote pane headers
    expect(screen.getByText("Local")).toBeInTheDocument();
    expect(screen.getByText("Remote")).toBeInTheDocument();

    // Remote pane should list files
    expect(screen.getByText("docs")).toBeInTheDocument();
    expect(screen.getByText("src")).toBeInTheDocument();
    expect(screen.getByText("config.json")).toBeInTheDocument();
  });

  // FT-C-27: Breadcrumb click navigates to directory
  it("FT-C-27: breadcrumb click navigates back to root", async () => {
    const listCalls: string[] = [];

    mockInvoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "sftp_open") return "sftp-session-1";
      if (cmd === "sftp_list") {
        const path = (args?.path as string) ?? "/";
        listCalls.push(path);
        if (path === "/docs") {
          return [
            { name: "api-guide.md", is_dir: false, size: 2048, modified: null, permissions: "-rw-r--r--" },
          ];
        }
        return MOCK_ENTRIES;
      }
      return undefined;
    });

    render(<SftpBrowser connectionId="conn-1" />);

    // Wait for initial load
    await waitFor(() => {
      expect(screen.getByText("README.md")).toBeInTheDocument();
    });

    // Double-click the "docs" directory to navigate into it
    const docsCell = screen.getByText("docs");
    fireEvent.doubleClick(docsCell.closest("tr")!);

    // Should navigate to /docs and load its contents
    await waitFor(() => {
      expect(screen.getByText("api-guide.md")).toBeInTheDocument();
    });

    // Now use breadcrumb: click the Home button to go back to root
    // The Home button is the first button inside the breadcrumb (contains Home SVG icon)
    const homeBtn = document.querySelector(String.raw`.flex.items-center.gap-0\.5 button`);
    expect(homeBtn).toBeTruthy();
    fireEvent.click(homeBtn!);

    // Should navigate back to root and show root entries
    await waitFor(() => {
      expect(screen.getByText("README.md")).toBeInTheDocument();
    });

    // Verify sftp_list was called with "/"  after the breadcrumb click
    const rootCallsAfterNav = listCalls.filter((p) => p === "/");
    expect(rootCallsAfterNav.length).toBeGreaterThanOrEqual(2); // initial + breadcrumb click
  });

  // FT-C-28: Double-click directory enters it
  it("FT-C-28: double-click directory enters it", async () => {
    const invokedPaths: string[] = [];

    mockInvoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === "sftp_open") return "sftp-session-1";
      if (cmd === "sftp_list") {
        const path = (args?.path as string) ?? "/";
        invokedPaths.push(path);
        if (path === "/docs") {
          return [
            { name: "api-guide.md", is_dir: false, size: 2048, modified: null, permissions: "-rw-r--r--" },
          ];
        }
        return MOCK_ENTRIES;
      }
      return undefined;
    });

    render(<SftpBrowser connectionId="conn-1" />);

    // Wait for initial file listing
    await waitFor(() => {
      expect(screen.getByText("docs")).toBeInTheDocument();
    });

    // Double-click the "docs" directory row
    const docsCell = screen.getByText("docs");
    fireEvent.doubleClick(docsCell.closest("tr")!);

    // Should navigate into /docs and show its contents
    await waitFor(() => {
      expect(screen.getByText("api-guide.md")).toBeInTheDocument();
    });

    // Verify sftp_list was called with the subdirectory path
    expect(invokedPaths).toContain("/docs");
  });

  it("renders not-connected state without connectionId", () => {
    render(<SftpBrowser />);

    expect(screen.getByText("No connection")).toBeInTheDocument();
  });
});
