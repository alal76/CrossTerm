import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import NotificationHistoryPanel from "@/components/Notifications/NotificationHistoryPanel";
import { invoke } from "@tauri-apps/api/core";

const mockInvoke = vi.mocked(invoke);

function notification(overrides: Record<string, unknown> = {}) {
  return {
    id: "n1",
    timestamp: new Date().toISOString(),
    severity: "info",
    message: "Something happened",
    session_id: null,
    category: "general",
    dismissed: false,
    ...overrides,
  };
}

describe("NotificationHistoryPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads and displays notifications grouped as Today", async () => {
    mockInvoke.mockResolvedValue([notification()]);
    render(<NotificationHistoryPanel onClose={vi.fn()} />);

    expect(await screen.findByText("Something happened")).toBeInTheDocument();
    expect(screen.getByText("Today")).toBeInTheDocument();
  });

  it("shows the empty state when there are no notifications", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<NotificationHistoryPanel onClose={vi.fn()} />);
    expect(await screen.findByText("No notifications")).toBeInTheDocument();
  });

  it("filters notifications by search query", async () => {
    mockInvoke.mockResolvedValue([
      notification({ id: "n1", message: "disk full" }),
      notification({ id: "n2", message: "connected" }),
    ]);
    render(<NotificationHistoryPanel onClose={vi.fn()} />);
    await screen.findByText("disk full");

    fireEvent.change(screen.getByPlaceholderText("Search notifications..."), {
      target: { value: "disk" },
    });

    expect(screen.getByText("disk full")).toBeInTheDocument();
    expect(screen.queryByText("connected")).not.toBeInTheDocument();
  });

  it("dismisses a single notification", async () => {
    mockInvoke.mockResolvedValueOnce([notification()]);
    render(<NotificationHistoryPanel onClose={vi.fn()} />);
    await screen.findByText("Something happened");

    mockInvoke.mockResolvedValueOnce(undefined);
    fireEvent.click(screen.getByTitle("Dismiss"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("notification_dismiss", { id: "n1" });
    });
  });

  it("clears all notifications", async () => {
    mockInvoke.mockResolvedValueOnce([notification()]);
    render(<NotificationHistoryPanel onClose={vi.fn()} />);
    await screen.findByText("Something happened");

    mockInvoke.mockResolvedValueOnce(undefined);
    fireEvent.click(screen.getByText("Clear All"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("notification_clear_all");
    });
    expect(await screen.findByText("No notifications")).toBeInTheDocument();
  });

  it("calls onClose from the close button and the backdrop", async () => {
    mockInvoke.mockResolvedValue([]);
    const onClose = vi.fn();
    render(<NotificationHistoryPanel onClose={onClose} />);
    await screen.findByText("No notifications");

    fireEvent.click(screen.getByLabelText("Close notifications"));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
