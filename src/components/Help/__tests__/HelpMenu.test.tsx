import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@/i18n";
import HelpMenu from "@/components/Help/HelpMenu";

describe("HelpMenu", () => {
  const onOpenHelp = vi.fn();
  const onOpenShortcuts = vi.fn();
  const onStartTour = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("FT-H-10: renders all menu items", async () => {
    const user = userEvent.setup();

    render(
      <HelpMenu
        onOpenHelp={onOpenHelp}
        onOpenShortcuts={onOpenShortcuts}
        onStartTour={onStartTour}
      />
    );

    // Open the menu by clicking the help button
    const helpButton = screen.getByTitle("Help");
    await user.click(helpButton);

    // All non-divider menu items should be visible
    expect(screen.getByText("Getting Started")).toBeInTheDocument();
    expect(screen.getByText("Keyboard Shortcuts")).toBeInTheDocument();
    expect(screen.getByText("Start Tour")).toBeInTheDocument();
    expect(screen.getByText("Troubleshooting")).toBeInTheDocument();
    expect(screen.getByText("Report Issue")).toBeInTheDocument();
    expect(screen.getByText("About")).toBeInTheDocument();
  });
});
