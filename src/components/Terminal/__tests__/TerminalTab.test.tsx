import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@/i18n";
import { invoke } from "@tauri-apps/api/core";
import { useTerminalStore } from "@/stores/terminalStore";

// Mock ResizeObserver
class MockResizeObserver {
  observe = vi.fn();
  unobserve = vi.fn();
  disconnect = vi.fn();
}
globalThis.ResizeObserver = MockResizeObserver as unknown as typeof ResizeObserver;

// Mock TerminalView since TerminalTab delegates rendering to it
vi.mock("@/components/Terminal/TerminalView", () => ({
  default: ({ terminalId }: { terminalId: string }) => (
    <div data-testid={`terminal-view-${terminalId}`}>TerminalView Mock</div>
  ),
}));

import TerminalTab from "@/components/Terminal/TerminalTab";

function resetStores() {
  useTerminalStore.setState({
    terminals: new Map(),
    broadcastMode: false,
  });
}

describe("TerminalTab", () => {
  beforeEach(() => {
    resetStores();
    vi.clearAllMocks();
  });

  // FT-C-33: Shows loading state while terminal creates
  it("FT-C-33: shows loading spinner while terminal creates", () => {
    // Make invoke hang (never resolves) to keep loading state
    vi.mocked(invoke).mockReturnValue(new Promise(() => {}));

    render(<TerminalTab sessionId="session-1" isActive={true} />);

    // Loading indicator should be visible
    expect(screen.getByText("Starting terminal…")).toBeInTheDocument();
  });

  // FT-C-34: Shows error state with retry button on failure
  it("FT-C-34: shows error state with retry button on failure", async () => {
    const user = userEvent.setup();

    // First call fails
    vi.mocked(invoke).mockRejectedValueOnce("PTY spawn failed");

    render(<TerminalTab sessionId="session-1" isActive={true} />);

    // Wait for error state
    await waitFor(() => {
      expect(screen.getByText("Failed to create terminal")).toBeInTheDocument();
    });

    expect(screen.getByText("PTY spawn failed")).toBeInTheDocument();

    // Retry button should be visible
    const retryButton = screen.getByText("Retry");
    expect(retryButton).toBeInTheDocument();

    // Second call succeeds
    vi.mocked(invoke).mockResolvedValueOnce({ id: "term-retry-1" });

    await user.click(retryButton);

    // Should render TerminalView after successful retry
    await waitFor(() => {
      expect(screen.getByTestId("terminal-view-term-retry-1")).toBeInTheDocument();
    });
  });
});
