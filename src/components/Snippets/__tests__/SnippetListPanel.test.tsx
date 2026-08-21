import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import SnippetListPanel from "@/components/Snippets/SnippetListPanel";
import { invoke } from "@tauri-apps/api/core";
import type { Snippet } from "@/types";

const mockInvoke = vi.mocked(invoke);

function makeSnippet(overrides: Partial<Snippet> = {}): Snippet {
  return {
    id: "snip-1",
    name: "List files",
    command: "ls -la",
    tags: [],
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
    ...overrides,
  };
}

function fieldFor(labelText: string, tag: "input" | "textarea" = "input"): HTMLElement {
  const label = screen.getByText(labelText).closest("label")!;
  return label.querySelector(tag)!;
}

describe("SnippetListPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
  });

  it("loads and renders snippets on mount", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_list") {
        return Promise.resolve([
          makeSnippet(),
          makeSnippet({ id: "snip-2", name: "Show disk usage", command: "df -h", tags: ["disk", "info"] }),
        ]);
      }
      return Promise.resolve(undefined);
    });

    render(<SnippetListPanel />);

    expect(await screen.findByText("List files")).toBeInTheDocument();
    expect(screen.getByText("Show disk usage")).toBeInTheDocument();
    expect(screen.getByText("df -h")).toBeInTheDocument();
    expect(screen.getByText("disk")).toBeInTheDocument();
    expect(screen.getByText("info")).toBeInTheDocument();
    expect(mockInvoke).toHaveBeenCalledWith("snippet_list");
  });

  it("shows the empty state when there are no snippets", async () => {
    mockInvoke.mockResolvedValue([]);

    render(<SnippetListPanel />);

    expect(await screen.findByText("No snippets yet")).toBeInTheDocument();
  });

  it("searches snippets via snippet_search as the query changes", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_list") return Promise.resolve([makeSnippet()]);
      if (cmd === "snippet_search") return Promise.resolve([makeSnippet({ name: "Search hit" })]);
      return Promise.resolve(undefined);
    });

    render(<SnippetListPanel />);
    await screen.findByText("List files");

    fireEvent.change(screen.getByPlaceholderText("Search snippets..."), {
      target: { value: "disk" },
    });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("snippet_search", { query: "disk" });
    });
    expect(await screen.findByText("Search hit")).toBeInTheDocument();
  });

  it("treats a snippet_list failure as an empty list instead of crashing", async () => {
    mockInvoke.mockRejectedValue(new Error("backend unavailable"));

    render(<SnippetListPanel />);

    expect(await screen.findByText("No snippets yet")).toBeInTheDocument();
  });

  it("copies a snippet's command to the clipboard", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_list") return Promise.resolve([makeSnippet()]);
      return Promise.resolve(undefined);
    });

    render(<SnippetListPanel />);
    await screen.findByText("List files");

    fireEvent.click(screen.getByTitle("Copy to Clipboard"));

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("ls -la");
    });
  });

  it("copies directly to the clipboard when inserting a snippet with no placeholders", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_list") return Promise.resolve([makeSnippet()]);
      return Promise.resolve(undefined);
    });

    render(<SnippetListPanel />);
    await screen.findByText("List files");

    fireEvent.click(screen.getByTitle("Insert"));

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("ls -la");
    });
    expect(screen.queryByText("Fill in placeholders")).not.toBeInTheDocument();
  });

  it("opens the placeholder-insert modal for a snippet with placeholders, then copies the filled-in result", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_list") {
        return Promise.resolve([makeSnippet({ command: "ssh {{host}}" })]);
      }
      return Promise.resolve(undefined);
    });

    render(<SnippetListPanel />);
    await screen.findByText("List files");

    fireEvent.click(screen.getByTitle("Insert"));

    expect(await screen.findByText("Fill in placeholders")).toBeInTheDocument();
    const hostInput = screen.getByText("{{host}}").parentElement!.querySelector("input")!;
    fireEvent.change(hostInput, { target: { value: "example.com" } });
    fireEvent.click(screen.getByText("Insert"));

    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("ssh example.com");
    });
    expect(screen.queryByText("Fill in placeholders")).not.toBeInTheDocument();
  });

  it("opens the editor pre-filled when clicking edit on an existing snippet", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_list") return Promise.resolve([makeSnippet()]);
      return Promise.resolve(undefined);
    });

    render(<SnippetListPanel />);
    await screen.findByText("List files");

    fireEvent.click(screen.getByTitle("Edit Snippet"));

    expect(await screen.findByText("Edit Snippet")).toBeInTheDocument();
    expect(fieldFor("Name")).toHaveValue("List files");
  });

  it("creates a new snippet through the editor and reloads the list", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_list") return Promise.resolve([]);
      if (cmd === "snippet_create") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    render(<SnippetListPanel />);
    await screen.findByText("No snippets yet");

    fireEvent.click(screen.getByTitle("New Snippet"));
    expect(await screen.findByText("New Snippet", { selector: "h2" })).toBeInTheDocument();

    fireEvent.change(fieldFor("Name"), { target: { value: "New one" } });
    fireEvent.change(fieldFor("Command", "textarea"), { target: { value: "echo hi" } });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_create") return Promise.resolve(undefined);
      if (cmd === "snippet_list") return Promise.resolve([makeSnippet({ name: "New one", command: "echo hi" })]);
      return Promise.resolve(undefined);
    });

    fireEvent.click(screen.getByText("Save"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("snippet_create", {
        name: "New one",
        command: "echo hi",
        tags: [],
      });
    });
    expect(await screen.findByText("New one")).toBeInTheDocument();
  });

  it("deletes a snippet after confirming, and reloads the list", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_list") return Promise.resolve([makeSnippet()]);
      if (cmd === "snippet_delete") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });

    render(<SnippetListPanel />);
    await screen.findByText("List files");

    fireEvent.click(screen.getByTitle("Delete Snippet"));
    expect(await screen.findByText("Delete this snippet?")).toBeInTheDocument();

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_delete") return Promise.resolve(undefined);
      if (cmd === "snippet_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    fireEvent.click(screen.getByText("Delete"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("snippet_delete", { id: "snip-1" });
    });
    expect(await screen.findByText("No snippets yet")).toBeInTheDocument();
  });

  it("cancels a pending delete confirmation without invoking snippet_delete", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "snippet_list") return Promise.resolve([makeSnippet()]);
      return Promise.resolve(undefined);
    });

    render(<SnippetListPanel />);
    await screen.findByText("List files");

    fireEvent.click(screen.getByTitle("Delete Snippet"));
    expect(await screen.findByText("Delete this snippet?")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Cancel"));

    expect(screen.queryByText("Delete this snippet?")).not.toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("snippet_delete", expect.anything());
  });
});
