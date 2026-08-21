import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import SnippetEditor from "@/components/Snippets/SnippetEditor";
import { invoke } from "@tauri-apps/api/core";
import type { Snippet } from "@/types";

const mockInvoke = vi.mocked(invoke);

const EXISTING_SNIPPET: Snippet = {
  id: "snip-1",
  name: "List files",
  command: "ls -la",
  tags: ["fs", "list"],
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

describe("SnippetEditor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders the 'New Snippet' title and empty fields when no snippet is passed", () => {
    render(<SnippetEditor onClose={vi.fn()} onSave={vi.fn()} />);

    expect(screen.getByText("New Snippet")).toBeInTheDocument();
    const textboxes = screen.getAllByRole("textbox");
    expect(textboxes).toHaveLength(3);
    textboxes.forEach((box) => expect(box).toHaveValue(""));
  });

  it("renders the 'Edit Snippet' title and pre-fills fields when a snippet is passed", () => {
    render(<SnippetEditor snippet={EXISTING_SNIPPET} onClose={vi.fn()} onSave={vi.fn()} />);

    expect(screen.getByText("Edit Snippet")).toBeInTheDocument();
    expect(screen.getByDisplayValue("List files")).toBeInTheDocument();
    expect(screen.getByDisplayValue("ls -la")).toBeInTheDocument();
    expect(screen.getByDisplayValue("fs, list")).toBeInTheDocument();
  });

  it("disables Save when name and command are empty", () => {
    render(<SnippetEditor onClose={vi.fn()} onSave={vi.fn()} />);

    expect(screen.getByText("Save")).toBeDisabled();
  });

  it("enables Save once name and command are both filled in", () => {
    render(<SnippetEditor onClose={vi.fn()} onSave={vi.fn()} />);

    const [nameInput, commandInput] = screen.getAllByRole("textbox");
    fireEvent.change(nameInput, { target: { value: "My Snippet" } });
    fireEvent.change(commandInput, { target: { value: "echo hi" } });

    expect(screen.getByText("Save")).not.toBeDisabled();
  });

  it("calls invoke('snippet_create') with trimmed name/command and parsed tags on save for a new snippet", async () => {
    mockInvoke.mockResolvedValue(undefined);
    const onSave = vi.fn();
    render(<SnippetEditor onClose={vi.fn()} onSave={onSave} />);

    const textboxes = screen.getAllByRole("textbox");
    fireEvent.change(textboxes[0], { target: { value: "  My Snippet  " } });
    fireEvent.change(textboxes[1], { target: { value: "echo hi" } });
    fireEvent.change(textboxes[2], { target: { value: "one, two ,, three" } });

    fireEvent.click(screen.getByText("Save"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("snippet_create", {
        name: "My Snippet",
        command: "echo hi",
        tags: ["one", "two", "three"],
      });
    });
    expect(onSave).toHaveBeenCalled();
  });

  it("calls invoke('snippet_update') with the snippet id when editing an existing snippet", async () => {
    mockInvoke.mockResolvedValue(undefined);
    const onSave = vi.fn();
    render(<SnippetEditor snippet={EXISTING_SNIPPET} onClose={vi.fn()} onSave={onSave} />);

    fireEvent.click(screen.getByText("Save"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("snippet_update", {
        id: "snip-1",
        name: "List files",
        command: "ls -la",
        tags: ["fs", "list"],
      });
    });
    expect(onSave).toHaveBeenCalled();
  });

  it("does not call onSave and re-enables Save when invoke rejects", async () => {
    mockInvoke.mockRejectedValue(new Error("backend unavailable"));
    const onSave = vi.fn();
    render(<SnippetEditor snippet={EXISTING_SNIPPET} onClose={vi.fn()} onSave={onSave} />);

    fireEvent.click(screen.getByText("Save"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalled();
    });
    expect(onSave).not.toHaveBeenCalled();
    expect(screen.getByText("Save")).not.toBeDisabled();
  });

  it("calls onClose when Cancel is clicked", () => {
    const onClose = vi.fn();
    render(<SnippetEditor onClose={onClose} onSave={vi.fn()} />);

    fireEvent.click(screen.getByText("Cancel"));

    expect(onClose).toHaveBeenCalled();
  });

  it("calls onClose when the close (X) button is clicked", () => {
    const onClose = vi.fn();
    const { container } = render(<SnippetEditor onClose={onClose} onSave={vi.fn()} />);

    const closeButton = container.querySelector("button");
    expect(closeButton).toBeTruthy();
    fireEvent.click(closeButton!);

    expect(onClose).toHaveBeenCalled();
  });
});
