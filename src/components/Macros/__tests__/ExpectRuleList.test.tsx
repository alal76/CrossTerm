import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import ExpectRuleList from "@/components/Macros/ExpectRuleList";
import { invoke } from "@tauri-apps/api/core";
import type { ExpectRule } from "@/types";

const mockInvoke = vi.mocked(invoke);

function rule(overrides: Partial<ExpectRule> = {}): ExpectRule {
  return {
    id: "r1",
    name: "Login prompt",
    pattern: "login:",
    action: { type: "send_text", text: "admin\\n" },
    enabled: true,
    ...overrides,
  };
}

describe("ExpectRuleList", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads and renders rules", async () => {
    mockInvoke.mockResolvedValue([rule()]);
    render(<ExpectRuleList />);
    expect(await screen.findByText("Login prompt")).toBeInTheDocument();
    expect(screen.getByText("login:")).toBeInTheDocument();
  });

  it("shows the empty state", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<ExpectRuleList />);
    expect(await screen.findByText(/No expect rules defined/)).toBeInTheDocument();
  });

  it("adds a rule via the form", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    render(<ExpectRuleList />);
    await screen.findByText(/No expect rules/);

    fireEvent.click(screen.getByText("Add Rule"));
    fireEvent.change(screen.getByPlaceholderText("Rule name"), { target: { value: "Password prompt" } });
    fireEvent.change(screen.getByPlaceholderText("Regex pattern"), { target: { value: "assword:" } });
    fireEvent.change(screen.getByPlaceholderText("Action value"), { target: { value: "secret\\n" } });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "expect_rule_create") return Promise.resolve(undefined);
      if (cmd === "expect_rule_list") return Promise.resolve([rule({ id: "r2", name: "Password prompt", pattern: "assword:" })]);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByText("Create"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("expect_rule_create", {
        name: "Password prompt",
        pattern: "assword:",
        action: { type: "send_text", text: "secret\\n" },
      });
    });
    expect(await screen.findByText("Password prompt")).toBeInTheDocument();
  });

  it("validates a regex pattern via the Test button", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<ExpectRuleList />);
    await screen.findByText(/No expect rules/);
    fireEvent.click(screen.getByText("Add Rule"));

    fireEvent.change(screen.getByPlaceholderText("Regex pattern"), { target: { value: "[invalid" } });
    fireEvent.click(screen.getByText("Test"));
    expect(await screen.findByText(/SyntaxError/)).toBeInTheDocument();
  });

  it("toggles and deletes a rule", async () => {
    mockInvoke.mockResolvedValueOnce([rule()]);
    render(<ExpectRuleList />);
    await screen.findByText("Login prompt");

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "expect_rule_toggle") return Promise.resolve(undefined);
      if (cmd === "expect_rule_list") return Promise.resolve([rule({ enabled: false })]);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getAllByRole("button").find((b) => b.querySelector("svg.lucide-toggle-right"))!);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("expect_rule_toggle", { id: "r1", enabled: false });
    });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "expect_rule_delete") return Promise.resolve(undefined);
      if (cmd === "expect_rule_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getAllByRole("button").find((b) => b.querySelector("svg.lucide-trash2"))!);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("expect_rule_delete", { id: "r1" });
    });
  });

  it("cancel closes the form without creating a rule", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<ExpectRuleList />);
    await screen.findByText(/No expect rules/);
    fireEvent.click(screen.getByText("Add Rule"));
    fireEvent.click(screen.getByText("Cancel"));
    expect(screen.queryByPlaceholderText("Rule name")).not.toBeInTheDocument();
  });
});
