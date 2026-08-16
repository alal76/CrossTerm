import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import AuditLog from "@/components/Audit/AuditLog";
import { invoke } from "@tauri-apps/api/core";
import type { AuditEntry } from "@/types";

const mockInvoke = vi.mocked(invoke);

function entry(overrides: Partial<AuditEntry> = {}): AuditEntry {
  return {
    id: "1",
    timestamp: "2026-01-01T00:00:00Z",
    user: "alice",
    action: "login",
    resource: "vault",
    success: true,
    ...overrides,
  };
}

describe("AuditLog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    globalThis.URL.createObjectURL = vi.fn(() => "blob:mock");
    globalThis.URL.revokeObjectURL = vi.fn();
  });

  it("loads and displays audit entries on mount", async () => {
    mockInvoke.mockResolvedValue([entry({ resource: "vault-1" })]);
    render(<AuditLog />);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("security_audit_list", { limit: 500 });
    });
    expect(await screen.findByText("vault-1")).toBeInTheDocument();
  });

  it("shows the empty state when there are no entries", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<AuditLog />);
    expect(await screen.findByText(/No audit entries/i)).toBeInTheDocument();
  });

  it("searches entries by query", async () => {
    mockInvoke.mockResolvedValueOnce([entry()]);
    render(<AuditLog />);
    await screen.findByText("login");

    mockInvoke.mockResolvedValueOnce([entry({ resource: "search-hit" })]);
    fireEvent.change(screen.getByPlaceholderText("Search audit entries..."), {
      target: { value: "search-hit" },
    });
    fireEvent.keyDown(screen.getByPlaceholderText("Search audit entries..."), { key: "Enter" });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("security_audit_search", { query: "search-hit" });
    });
    expect(await screen.findByText("search-hit")).toBeInTheDocument();
  });

  it("filters entries by action", async () => {
    mockInvoke.mockResolvedValue([entry({ action: "login", resource: "r1" }), entry({ action: "logout", resource: "r2" })]);
    render(<AuditLog />);
    await screen.findByText("r1");

    fireEvent.change(screen.getByDisplayValue("All Actions"), { target: { value: "logout" } });

    expect(screen.queryByText("r1")).not.toBeInTheDocument();
    expect(screen.getByText("r2")).toBeInTheDocument();
  });

  it("clears the log after confirmation", async () => {
    mockInvoke.mockResolvedValueOnce([entry()]);
    render(<AuditLog />);
    await screen.findByText("login");

    fireEvent.click(screen.getByText("Clear Audit Log"));
    expect(screen.getByText(/Are you sure/)).toBeInTheDocument();

    mockInvoke.mockResolvedValueOnce(0);
    fireEvent.click(screen.getByText("Confirm"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("security_clear_audit_log");
    });
    expect(await screen.findByText(/No audit entries/i)).toBeInTheDocument();
  });

  it("cancelling the clear confirmation dismisses it without clearing", async () => {
    mockInvoke.mockResolvedValueOnce([entry()]);
    render(<AuditLog />);
    await screen.findByText("login");

    fireEvent.click(screen.getByText("Clear Audit Log"));
    fireEvent.click(screen.getByText("Cancel"));

    expect(screen.queryByText(/Are you sure/)).not.toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("security_clear_audit_log");
  });

  it("exports entries as CSV and JSON", async () => {
    mockInvoke.mockResolvedValue([entry()]);
    render(<AuditLog />);
    await screen.findByText("login");

    fireEvent.click(screen.getByTitle("Export CSV"));
    fireEvent.click(screen.getByTitle("Export JSON"));

    expect(globalThis.URL.createObjectURL).toHaveBeenCalledTimes(2);
  });
});
