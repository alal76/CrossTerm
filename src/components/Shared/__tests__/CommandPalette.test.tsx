import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@/i18n";
import CommandPalette from "@/components/Shared/CommandPalette";

function openPalette() {
  fireEvent.keyDown(document, {
    key: "p",
    ctrlKey: true,
    shiftKey: true,
  });
}

describe("CommandPalette", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("FT-C-16: renders and closes on Escape", () => {
    render(<CommandPalette />);

    // Not visible initially
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    // Open via Ctrl+Shift+P
    openPalette();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText("Type a command…")
    ).toBeInTheDocument();

    // Close via Escape
    const input = screen.getByPlaceholderText("Type a command…");
    fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("FT-C-17: filters actions by search query", async () => {
    const user = userEvent.setup();
    render(<CommandPalette />);

    openPalette();

    const input = screen.getByPlaceholderText("Type a command…");
    await user.type(input, "Settings");

    // "Open Settings" should be visible
    expect(screen.getByText("Open Settings")).toBeInTheDocument();

    // Unrelated actions should be filtered out
    expect(screen.queryByText("Lock Vault")).not.toBeInTheDocument();
  });

  // FT-C-18: Enter key on selected item executes action
  it("FT-C-18: Enter key on selected item executes action", async () => {
    const onOpenSettings = vi.fn();
    render(<CommandPalette onOpenSettings={onOpenSettings} />);

    openPalette();

    const input = screen.getByPlaceholderText("Type a command…");

    // Type to filter to "Open Settings"
    await userEvent.type(input, "Open Settings");

    // "Open Settings" should be visible and selected (first match)
    expect(screen.getByText("Open Settings")).toBeInTheDocument();

    // Press Enter to execute
    fireEvent.keyDown(input, { key: "Enter" });

    // The action callback should have been invoked
    expect(onOpenSettings).toHaveBeenCalledTimes(1);

    // Palette should be closed
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});
