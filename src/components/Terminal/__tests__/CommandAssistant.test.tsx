import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import CommandAssistant from "@/components/Terminal/CommandAssistant";
import { invoke } from "@tauri-apps/api/core";

const mockInvoke = vi.mocked(invoke);

describe("CommandAssistant", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
  });

  it("checks Ollama availability on mount and enables the input when available", async () => {
    mockInvoke.mockResolvedValue(true);
    render(
      <CommandAssistant sessionId="s1" onInsertCommand={vi.fn()} onClose={vi.fn()} />,
    );

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("ai_check_available"));
    expect(screen.getByPlaceholderText(/Ask AI/)).not.toBeDisabled();
  });

  it("shows a banner and disables input when Ollama is unavailable", async () => {
    mockInvoke.mockResolvedValue(false);
    render(<CommandAssistant sessionId="s1" onInsertCommand={vi.fn()} onClose={vi.fn()} />);

    expect(await screen.findByText(/Ollama not running/)).toBeInTheDocument();
    expect(screen.getByPlaceholderText(/Ask AI/)).toBeDisabled();
  });

  it("submits a query and renders suggestion cards", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ai_check_available") return Promise.resolve(true);
      if (cmd === "ai_suggest_command") {
        return Promise.resolve([
          { command: "find . -size +100M", explanation: "Find large files", risk_level: "safe" },
        ]);
      }
      return Promise.resolve(undefined);
    });

    render(
      <CommandAssistant
        sessionId="s1"
        currentDirectory="/home/user"
        shell="bash"
        recentCommands={["ls", "cd /tmp"]}
        onInsertCommand={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("ai_check_available"));

    fireEvent.change(screen.getByPlaceholderText(/Ask AI/), { target: { value: "find large files" } });
    fireEvent.click(screen.getByText("Suggest"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("ai_suggest_command", {
        userRequest: "find large files",
        context: expect.objectContaining({
          current_directory: "/home/user",
          shell: "bash",
          recent_commands: ["ls", "cd /tmp"],
        }),
      });
    });
    expect(await screen.findByText("find . -size +100M")).toBeInTheDocument();
    expect(screen.getByText("Safe")).toBeInTheDocument();
  });

  it("shows an error state when the suggestion request fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ai_check_available") return Promise.resolve(true);
      if (cmd === "ai_suggest_command") return Promise.reject("model not found");
      return Promise.resolve(undefined);
    });
    render(<CommandAssistant sessionId="s1" onInsertCommand={vi.fn()} onClose={vi.fn()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("ai_check_available"));

    fireEvent.change(screen.getByPlaceholderText(/Ask AI/), { target: { value: "do a thing" } });
    fireEvent.click(screen.getByText("Suggest"));

    expect(await screen.findByText("model not found")).toBeInTheDocument();
  });

  it("inserting a suggestion calls onInsertCommand and onClose", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ai_check_available") return Promise.resolve(true);
      if (cmd === "ai_suggest_command") {
        return Promise.resolve([{ command: "ls -la", explanation: "list files", risk_level: "safe" }]);
      }
      return Promise.resolve(undefined);
    });
    const onInsertCommand = vi.fn();
    const onClose = vi.fn();
    render(<CommandAssistant sessionId="s1" onInsertCommand={onInsertCommand} onClose={onClose} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("ai_check_available"));

    fireEvent.change(screen.getByPlaceholderText(/Ask AI/), { target: { value: "list files" } });
    fireEvent.click(screen.getByText("Suggest"));
    await screen.findByText("ls -la");

    fireEvent.click(screen.getByTitle("Insert into terminal"));
    expect(onInsertCommand).toHaveBeenCalledWith("ls -la");
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("copying a suggestion writes it to the clipboard", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ai_check_available") return Promise.resolve(true);
      if (cmd === "ai_suggest_command") {
        return Promise.resolve([{ command: "ps aux", explanation: "list processes", risk_level: "caution" }]);
      }
      return Promise.resolve(undefined);
    });
    render(<CommandAssistant sessionId="s1" onInsertCommand={vi.fn()} onClose={vi.fn()} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("ai_check_available"));

    fireEvent.change(screen.getByPlaceholderText(/Ask AI/), { target: { value: "list processes" } });
    fireEvent.click(screen.getByText("Suggest"));
    await screen.findByText("ps aux");

    fireEvent.click(screen.getByTitle("Copy command"));
    await waitFor(() => {
      expect(navigator.clipboard.writeText).toHaveBeenCalledWith("ps aux");
    });
  });

  it("Escape key calls onClose", async () => {
    mockInvoke.mockResolvedValue(true);
    const onClose = vi.fn();
    render(<CommandAssistant sessionId="s1" onInsertCommand={vi.fn()} onClose={onClose} />);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("ai_check_available"));

    fireEvent.keyDown(screen.getByPlaceholderText(/Ask AI/), { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("close button calls onClose", async () => {
    mockInvoke.mockResolvedValue(true);
    const onClose = vi.fn();
    render(<CommandAssistant sessionId="s1" onInsertCommand={vi.fn()} onClose={onClose} />);
    fireEvent.click(screen.getByTitle("Close AI assistant"));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
