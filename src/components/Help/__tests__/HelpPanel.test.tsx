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

    // Use role-based queries for sidebar buttons so we don't accidentally match
    // article body text that contains the same string as a title (e.g. "SSH
    // Connections" appears both as a button and as link text in Getting Started).
    expect(screen.getByRole("button", { name: "SSH Connections" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Credential Vault" })).toBeInTheDocument();

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

    // "Customization" article should be filtered out — it contains no SFTP
    // content in its title, keywords, or body.
    // (Keyboard Shortcuts is not used here because it gained an SFTP Browser
    // shortcuts section and now legitimately matches the "SFTP" query.)
    const buttons = screen.getAllByRole("button");
    const customizationBtn = buttons.find(
      (btn) => btn.textContent?.trim() === "Customization"
    );
    expect(customizationBtn).toBeUndefined();
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
