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
});
