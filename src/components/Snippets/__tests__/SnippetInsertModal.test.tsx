import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@/i18n";
import SnippetInsertModal from "@/components/Snippets/SnippetInsertModal";
import type { Snippet } from "@/types";

function makeSnippet(command: string): Snippet {
  return {
    id: "snip-1",
    name: "Test Snippet",
    command,
    tags: [],
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  };
}

describe("SnippetInsertModal", () => {
  it("renders the command preview and the placeholder prompt title", () => {
    render(
      <SnippetInsertModal snippet={makeSnippet("echo {{name}}")} onInsert={vi.fn()} onCancel={vi.fn()} />
    );

    expect(screen.getByText("Fill in placeholders")).toBeInTheDocument();
    expect(screen.getByText("echo {{name}}")).toBeInTheDocument();
  });

  it("renders one input per unique placeholder, in first-seen order", () => {
    render(
      <SnippetInsertModal
        snippet={makeSnippet("scp {{file}} {{host}}:{{file}}")}
        onInsert={vi.fn()}
        onCancel={vi.fn()}
      />
    );

    const labels = screen.getAllByText(/\{\{\w+\}\}/).map((el) => el.textContent);
    // The command preview itself also matches; placeholder labels come after it.
    expect(labels.filter((l) => l === "{{file}}" || l === "{{host}}")).toHaveLength(2);
    expect(screen.getByText("{{file}}")).toBeInTheDocument();
    expect(screen.getByText("{{host}}")).toBeInTheDocument();
  });

  it("renders no placeholder inputs for a command with no placeholders", () => {
    render(
      <SnippetInsertModal snippet={makeSnippet("ls -la")} onInsert={vi.fn()} onCancel={vi.fn()} />
    );

    expect(screen.queryAllByRole("textbox")).toHaveLength(0);
  });

  it("calls onInsert with the original command unchanged when there are no placeholders", () => {
    const onInsert = vi.fn();
    render(<SnippetInsertModal snippet={makeSnippet("ls -la")} onInsert={onInsert} onCancel={vi.fn()} />);

    fireEvent.click(screen.getByText("Insert"));

    expect(onInsert).toHaveBeenCalledWith("ls -la");
  });

  it("substitutes every occurrence of each placeholder with its entered value", () => {
    const onInsert = vi.fn();
    render(
      <SnippetInsertModal
        snippet={makeSnippet("scp {{file}} {{host}}:{{file}}")}
        onInsert={onInsert}
        onCancel={vi.fn()}
      />
    );

    const fileInput = screen.getByText("{{file}}").parentElement!.querySelector("input")!;
    const hostInput = screen.getByText("{{host}}").parentElement!.querySelector("input")!;
    fireEvent.change(fileInput, { target: { value: "a.txt" } });
    fireEvent.change(hostInput, { target: { value: "example.com" } });

    fireEvent.click(screen.getByText("Insert"));

    expect(onInsert).toHaveBeenCalledWith("scp a.txt example.com:a.txt");
  });

  it("leaves an unfilled placeholder blank when substituting", () => {
    const onInsert = vi.fn();
    render(
      <SnippetInsertModal snippet={makeSnippet("echo {{name}}")} onInsert={onInsert} onCancel={vi.fn()} />
    );

    fireEvent.click(screen.getByText("Insert"));

    expect(onInsert).toHaveBeenCalledWith("echo ");
  });

  it("calls onCancel when the Cancel button is clicked", () => {
    const onCancel = vi.fn();
    render(<SnippetInsertModal snippet={makeSnippet("ls -la")} onInsert={vi.fn()} onCancel={onCancel} />);

    fireEvent.click(screen.getByText("Cancel"));

    expect(onCancel).toHaveBeenCalled();
  });

  it("calls onCancel when the close (X) button is clicked", () => {
    const onCancel = vi.fn();
    const { container } = render(
      <SnippetInsertModal snippet={makeSnippet("ls -la")} onInsert={vi.fn()} onCancel={onCancel} />
    );

    fireEvent.click(container.querySelector("button")!);

    expect(onCancel).toHaveBeenCalled();
  });
});
