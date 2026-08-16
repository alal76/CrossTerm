import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import CodeEditor from "@/components/Editor/CodeEditor";
import { invoke } from "@tauri-apps/api/core";
import type { EditorFile } from "@/types";

const mockInvoke = vi.mocked(invoke);

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

function file(overrides: Partial<EditorFile> = {}): EditorFile {
  return {
    id: "f1",
    path: "/home/user/main.rs",
    content: "fn main() {}",
    encoding: "UTF-8",
    language: "rust",
    modified: false,
    line_count: 1,
    size_bytes: 12,
    ...overrides,
  };
}

describe("CodeEditor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the empty state when no files are open", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<CodeEditor />);
    expect(await screen.findByText("No files open. Open a file to start editing.")).toBeInTheDocument();
  });

  it("loads already-open files on mount", async () => {
    mockInvoke.mockResolvedValue([file()]);
    render(<CodeEditor />);
    expect(await screen.findByText("main.rs")).toBeInTheDocument();
  });

  it("opens a file via the file picker", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "editor_list_open") return Promise.resolve([]);
      if (cmd === "editor_open") return Promise.resolve(file());
      return Promise.resolve(undefined);
    });
    const { open } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(open).mockResolvedValue("/home/user/main.rs");

    render(<CodeEditor />);
    await screen.findByText("No files open. Open a file to start editing.");

    fireEvent.click(screen.getByTitle("Open File"));

    expect(await screen.findByDisplayValue("fn main() {}")).toBeInTheDocument();
  });

  it("edits content and marks the tab as modified", async () => {
    mockInvoke.mockResolvedValue([file()]);
    render(<CodeEditor />);
    fireEvent.click(await screen.findByText("main.rs"));
    const textarea = await screen.findByDisplayValue("fn main() {}");

    fireEvent.change(textarea, { target: { value: "fn main() { println!(); }" } });
    expect(screen.getByText("Modified")).toBeInTheDocument();
  });

  it("saves via the save button", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "editor_list_open") return Promise.resolve([file({ modified: true })]);
      if (cmd === "editor_save") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    render(<CodeEditor />);
    fireEvent.click(await screen.findByText("main.rs"));

    fireEvent.click(screen.getByTitle("Save"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("editor_save", { fileId: "f1", content: "fn main() {}" });
    });
  });

  it("closes a tab", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "editor_list_open") return Promise.resolve([file()]);
      if (cmd === "editor_close") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    render(<CodeEditor />);
    await screen.findByText("main.rs");

    fireEvent.click(document.querySelector('[role="tab"] button')!);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("editor_close", { fileId: "f1" });
    });
    expect(await screen.findByText("No files open. Open a file to start editing.")).toBeInTheDocument();
  });

  it("switches between two open tabs", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "editor_list_open") {
        return Promise.resolve([
          file({ id: "f1", path: "/a.rs", content: "// a" }),
          file({ id: "f2", path: "/b.rs", content: "// b" }),
        ]);
      }
      return Promise.resolve(undefined);
    });
    render(<CodeEditor />);
    await screen.findByText("a.rs");

    fireEvent.click(screen.getByText("b.rs"));
    expect(await screen.findByDisplayValue("// b")).toBeInTheDocument();
  });

  it("opens the search bar and searches", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "editor_list_open") return Promise.resolve([file()]);
      if (cmd === "editor_search") {
        return Promise.resolve([{ line: 1, column: 0, length: 4, text: "main" }]);
      }
      return Promise.resolve(undefined);
    });
    render(<CodeEditor />);
    fireEvent.click(await screen.findByText("main.rs"));

    fireEvent.keyDown(screen.getByRole("application"), { key: "f", metaKey: true });
    const searchInput = await screen.findByPlaceholderText("Search");
    fireEvent.change(searchInput, { target: { value: "main" } });
    fireEvent.keyDown(searchInput, { key: "Enter" });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("editor_search", { fileId: "f1", query: "main", regex: false });
    });
    expect(await screen.findByText("1 found")).toBeInTheDocument();
  });

  it("Cmd+S triggers a save", async () => {
    mockInvoke.mockResolvedValue([file()]);
    render(<CodeEditor />);
    fireEvent.click(await screen.findByText("main.rs"));

    fireEvent.keyDown(screen.getByRole("application"), { key: "s", metaKey: true });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("editor_save", { fileId: "f1", content: "fn main() {}" });
    });
  });
});
