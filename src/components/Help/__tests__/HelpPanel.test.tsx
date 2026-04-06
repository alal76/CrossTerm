import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@/i18n";
import HelpPanel from "@/components/Help/HelpPanel";

describe("HelpPanel", () => {
  const onClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("FT-H-01: renders article list and markdown content", () => {
    render(<HelpPanel open={true} onClose={onClose} />);

    // Article sidebar should contain article titles
    expect(screen.getAllByText("Getting Started").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("SSH Connections")).toBeInTheDocument();
    expect(screen.getByText("Credential Vault")).toBeInTheDocument();

    // Default active article ("Getting Started") content should render
    expect(
      screen.getByText("Getting Started with CrossTerm", { exact: false })
    ).toBeInTheDocument();
  });

  it("FT-H-02: search filters articles", async () => {
    const user = userEvent.setup();
    render(<HelpPanel open={true} onClose={onClose} />);

    const searchInput = screen.getByPlaceholderText("Search help articles…");
    await user.type(searchInput, "SFTP");

    // SFTP File Transfer should remain visible
    expect(screen.getByText("SFTP File Transfer")).toBeInTheDocument();

    // "Keyboard Shortcuts" article should be filtered out
    // (it doesn't match "SFTP" in title, keywords, or body)
    const buttons = screen.getAllByRole("button");
    const shortcutsBtn = buttons.find(
      (btn) => btn.textContent?.trim() === "Keyboard Shortcuts"
    );
    expect(shortcutsBtn).toBeUndefined();
  });

  // FT-H-03: Deep link navigates to correct article section
  it("FT-H-03: deep link via articleSlug navigates to correct article", () => {
    render(<HelpPanel open={true} onClose={onClose} articleSlug="ssh-connections" />);

    // The SSH Connections article content should render
    // Check for SSH-specific body content (may have multiple matches in sidebar + content)
    expect(
      screen.getAllByText("Authentication Methods", { exact: false }).length
    ).toBeGreaterThanOrEqual(1);

    // Should not show the default "Getting Started" article body
    expect(
      screen.queryByText("Welcome to CrossTerm — a cross-platform terminal emulator", { exact: false })
    ).not.toBeInTheDocument();
  });

  it("FT-H-03: deep link to non-existent slug stays on default", () => {
    render(<HelpPanel open={true} onClose={onClose} articleSlug="nonexistent-article" />);

    // Should fall back to the default article
    expect(
      screen.getByText("Getting Started with CrossTerm", { exact: false })
    ).toBeInTheDocument();
  });
});
