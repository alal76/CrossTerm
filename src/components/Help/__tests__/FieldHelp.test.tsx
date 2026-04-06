import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@/i18n";
import FieldHelp from "@/components/Help/FieldHelp";

describe("FieldHelp", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("FT-H-09: renders (?) icon and shows description on click", async () => {
    const user = userEvent.setup();
    const description = "The port number for the connection.";

    render(<FieldHelp description={description} />);

    // The toggle button with (?) icon should be present
    const toggleBtn = screen.getByRole("button", { name: "Toggle help" });
    expect(toggleBtn).toBeInTheDocument();

    // Description should not be visible initially
    expect(screen.queryByText(description)).not.toBeInTheDocument();

    // Click to open
    await user.click(toggleBtn);
    expect(screen.getByText(description)).toBeInTheDocument();

    // Click again to close
    await user.click(toggleBtn);
    expect(screen.queryByText(description)).not.toBeInTheDocument();
  });
});
