import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@/i18n";
import ShortcutOverlay from "@/components/Help/ShortcutOverlay";
import { useAppStore } from "@/stores/appStore";

function resetStore() {
  useAppStore.setState({ customShortcuts: {} });
}

describe("ShortcutOverlay", () => {
  const onClose = vi.fn();

  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  it("FT-H-04: renders shortcuts by category", () => {
    render(<ShortcutOverlay open={true} onClose={onClose} />);

    // Category headings should be present
    expect(screen.getByText("General")).toBeInTheDocument();
    expect(screen.getByText("Tabs")).toBeInTheDocument();
    expect(screen.getByText("Terminal")).toBeInTheDocument();
    expect(screen.getByText("Split Panes")).toBeInTheDocument();
    expect(screen.getByText("Navigation")).toBeInTheDocument();

    // Some known shortcut labels should appear
    expect(screen.getByText("Command Palette")).toBeInTheDocument();
    expect(screen.getByText("Close Tab")).toBeInTheDocument();
    expect(screen.getByText("Copy")).toBeInTheDocument();
  });

  it("FT-H-05: search filters shortcuts", async () => {
    const user = userEvent.setup();
    render(<ShortcutOverlay open={true} onClose={onClose} />);

    const searchInput = screen.getByPlaceholderText("Search shortcuts…");
    await user.type(searchInput, "Copy");

    // "Copy" should be visible
    expect(screen.getByText("Copy")).toBeInTheDocument();

    // Unrelated shortcuts should be filtered out
    expect(screen.queryByText("Command Palette")).not.toBeInTheDocument();
    expect(screen.queryByText("Close Tab")).not.toBeInTheDocument();
  });
});
