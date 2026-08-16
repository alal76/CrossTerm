import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import DiffViewer from "@/components/Editor/DiffViewer";
import { invoke } from "@tauri-apps/api/core";
import type { DiffResult } from "@/types";

const mockInvoke = vi.mocked(invoke);

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

function diffResult(overrides: Partial<DiffResult> = {}): DiffResult {
  return {
    left_path: "/a.txt",
    right_path: "/b.txt",
    stats: { additions: 1, deletions: 1, modifications: 0 },
    hunks: [
      {
        left_start: 1,
        left_count: 2,
        right_start: 1,
        right_count: 2,
        lines: [
          { line_type: "context", content: "unchanged", left_line: 1, right_line: 1 },
          { line_type: "removed", content: "old line", left_line: 2 },
          { line_type: "added", content: "new line", right_line: 2 },
        ],
      },
    ],
    ...overrides,
  };
}

describe("DiffViewer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the empty-state prompt before any comparison", () => {
    render(<DiffViewer />);
    expect(screen.getByText("Select two files to compare")).toBeInTheDocument();
  });

  it("compares two files and renders the diff", async () => {
    mockInvoke.mockResolvedValue(diffResult());
    render(<DiffViewer />);

    fireEvent.change(screen.getByPlaceholderText("Left file path"), { target: { value: "/a.txt" } });
    fireEvent.change(screen.getByPlaceholderText("Right file path"), { target: { value: "/b.txt" } });
    fireEvent.click(screen.getByText("Compare Files"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("editor_diff", { leftPath: "/a.txt", rightPath: "/b.txt" });
    });
    expect(await screen.findByText("old line")).toBeInTheDocument();
    expect(screen.getByText("new line")).toBeInTheDocument();
    expect(screen.getByText(/\+1 additions/)).toBeInTheDocument();
    expect(screen.getByText(/-1 deletions/)).toBeInTheDocument();
  });

  it("does not call invoke until both paths are filled in", () => {
    render(<DiffViewer />);
    fireEvent.change(screen.getByPlaceholderText("Left file path"), { target: { value: "/a.txt" } });
    fireEvent.click(screen.getByText("Compare Files"));
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("shows an error message when the diff fails", async () => {
    mockInvoke.mockRejectedValue(new Error("no such file"));
    render(<DiffViewer />);

    fireEvent.change(screen.getByPlaceholderText("Left file path"), { target: { value: "/a.txt" } });
    fireEvent.change(screen.getByPlaceholderText("Right file path"), { target: { value: "/b.txt" } });
    fireEvent.click(screen.getByText("Compare Files"));

    expect(await screen.findByText(/no such file/)).toBeInTheDocument();
  });

  it("shows 'Files are identical' when the diff has no hunks", async () => {
    mockInvoke.mockResolvedValue(diffResult({ hunks: [] }));
    render(<DiffViewer />);

    fireEvent.change(screen.getByPlaceholderText("Left file path"), { target: { value: "/a.txt" } });
    fireEvent.change(screen.getByPlaceholderText("Right file path"), { target: { value: "/b.txt" } });
    fireEvent.click(screen.getByText("Compare Files"));

    expect(await screen.findByText("Files are identical")).toBeInTheDocument();
  });

  it("picking a left file via the file-picker button sets the left path", async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(open).mockResolvedValue("/picked-left.txt");

    render(<DiffViewer />);
    const buttons = screen.getAllByRole("button");
    // First icon button is the "select left file" trigger.
    fireEvent.click(buttons[0]);

    await waitFor(() => {
      expect(screen.getByPlaceholderText("Left file path")).toHaveValue("/picked-left.txt");
    });
  });
});
