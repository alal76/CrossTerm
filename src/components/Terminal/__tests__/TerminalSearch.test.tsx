import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import TerminalSearch from "@/components/Terminal/TerminalSearch";
import type { SearchAddon } from "@xterm/addon-search";

function makeSearchAddon(): SearchAddon {
  return {
    findNext: vi.fn(),
    findPrevious: vi.fn(),
    clearDecorations: vi.fn(),
  } as unknown as SearchAddon;
}

describe("TerminalSearch", () => {
  it("renders nothing when not visible", () => {
    const { container } = render(
      <TerminalSearch searchAddon={null} visible={false} onClose={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders the search input when visible", () => {
    render(<TerminalSearch searchAddon={null} visible onClose={vi.fn()} />);
    expect(screen.getByPlaceholderText("Search…")).toBeInTheDocument();
  });

  it("calls findNext as the user types", () => {
    const addon = makeSearchAddon();
    render(<TerminalSearch searchAddon={addon} visible onClose={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText("Search…"), { target: { value: "err" } });
    expect(addon.findNext).toHaveBeenCalledWith("err", { caseSensitive: false, regex: false, incremental: true });
  });

  it("Enter searches next, Shift+Enter searches previous", () => {
    const addon = makeSearchAddon();
    render(<TerminalSearch searchAddon={addon} visible onClose={vi.fn()} />);
    const input = screen.getByPlaceholderText("Search…");
    fireEvent.change(input, { target: { value: "err" } });
    vi.mocked(addon.findNext).mockClear();

    fireEvent.keyDown(input, { key: "Enter" });
    expect(addon.findNext).toHaveBeenCalledWith(
      "err",
      { caseSensitive: false, regex: false, wholeWord: false, incremental: true },
    );

    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(addon.findPrevious).toHaveBeenCalledWith(
      "err",
      { caseSensitive: false, regex: false, wholeWord: false, incremental: true },
    );
  });

  it("Escape calls onClose", () => {
    const onClose = vi.fn();
    render(<TerminalSearch searchAddon={null} visible onClose={onClose} />);
    fireEvent.keyDown(screen.getByPlaceholderText("Search…"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("toggles case sensitivity and reflects it in the next search", () => {
    const addon = makeSearchAddon();
    render(<TerminalSearch searchAddon={addon} visible onClose={vi.fn()} />);
    fireEvent.click(screen.getByTitle("Case sensitive"));
    fireEvent.change(screen.getByPlaceholderText("Search…"), { target: { value: "err" } });
    expect(addon.findNext).toHaveBeenCalledWith("err", { caseSensitive: true, regex: false, incremental: true });
  });

  it("regex toggle button calls onRegexToggle", () => {
    const onRegexToggle = vi.fn();
    render(<TerminalSearch searchAddon={null} visible onClose={vi.fn()} onRegexToggle={onRegexToggle} />);
    fireEvent.click(screen.getByTitle("Use regular expression"));
    expect(onRegexToggle).toHaveBeenCalledOnce();
  });

  it("prev/next/close buttons drive the addon and onClose", () => {
    const addon = makeSearchAddon();
    const onClose = vi.fn();
    render(<TerminalSearch searchAddon={addon} visible onClose={onClose} />);
    fireEvent.change(screen.getByPlaceholderText("Search…"), { target: { value: "err" } });

    fireEvent.click(screen.getByTitle("Previous match"));
    expect(addon.findPrevious).toHaveBeenCalled();

    fireEvent.click(screen.getByTitle("Next match"));
    expect(addon.findNext).toHaveBeenCalled();

    fireEvent.click(screen.getByTitle("Close"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("clears decorations when it becomes hidden", () => {
    const addon = makeSearchAddon();
    const { rerender } = render(<TerminalSearch searchAddon={addon} visible onClose={vi.fn()} />);
    rerender(<TerminalSearch searchAddon={addon} visible={false} onClose={vi.fn()} />);
    expect(addon.clearDecorations).toHaveBeenCalled();
  });
});
