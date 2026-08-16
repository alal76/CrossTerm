import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ImportWizard from "@/components/Shared/ImportWizard";
import { invoke } from "@tauri-apps/api/core";

const mockInvoke = vi.mocked(invoke);

function source(overrides: Record<string, unknown> = {}) {
  return {
    source_type: "putty",
    display_name: "PuTTY",
    path: "HKCU\\Software\\SimonTatham\\PuTTY\\Sessions",
    session_count: 2,
    available: true,
    ...overrides,
  };
}

function parsedSession(overrides: Record<string, unknown> = {}) {
  return { host: "10.0.0.5", port: 22, user: "root", session_type: "ssh", ...overrides };
}

describe("ImportWizard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders nothing when closed", () => {
    const { container } = render(<ImportWizard open={false} onClose={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("detects sources and pre-checks the available ones", async () => {
    mockInvoke.mockResolvedValue([source(), source({ source_type: "mobaxterm", display_name: "MobaXterm", available: false })]);
    render(<ImportWizard open onClose={vi.fn()} />);

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("import_detect_sources"));
    expect(await screen.findByText("PuTTY")).toBeInTheDocument();
    expect(screen.getByText("MobaXterm")).toBeInTheDocument();
  });

  it("shows no-sources message when detection returns empty", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<ImportWizard open onClose={vi.fn()} />);
    expect(await screen.findByText("No importable sources detected.")).toBeInTheDocument();
  });

  it("Next is disabled until a source is checked", async () => {
    mockInvoke.mockResolvedValue([source({ available: false })]);
    render(<ImportWizard open onClose={vi.fn()} />);
    await screen.findByText("PuTTY");
    expect(screen.getByRole("button", { name: /Next/ })).toBeDisabled();
  });

  it("goes to preview and parses selected sources", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "import_detect_sources") return Promise.resolve([source()]);
      if (cmd === "import_parse_source") return Promise.resolve([parsedSession()]);
      return Promise.resolve(undefined);
    });
    render(<ImportWizard open onClose={vi.fn()} />);
    await screen.findByText("PuTTY");

    fireEvent.click(screen.getByRole("button", { name: /Next/ }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("import_parse_source", { sourceType: "putty" });
    });
    expect(await screen.findByText("10.0.0.5")).toBeInTheDocument();
  });

  it("completes the import and shows the summary", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "import_detect_sources") return Promise.resolve([source()]);
      if (cmd === "import_parse_source") return Promise.resolve([parsedSession(), parsedSession({ host: "10.0.0.6" })]);
      if (cmd === "session_import_batch") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    const onComplete = vi.fn();
    render(<ImportWizard open onClose={vi.fn()} onComplete={onComplete} />);
    await screen.findByText("PuTTY");
    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    await screen.findByText("10.0.0.5");

    // Uncheck one session to exercise the skip-count path.
    const rowCheckboxes = screen.getAllByRole("checkbox");
    fireEvent.click(rowCheckboxes[rowCheckboxes.length - 1]);

    fireEvent.click(screen.getByRole("button", { name: /Import/ }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("session_import_batch", { sessions: [parsedSession()] });
    });
    expect(await screen.findByText("Import complete")).toBeInTheDocument();
    expect(onComplete).toHaveBeenCalledWith(1);
  });

  it("cancel on step 0 calls onClose", async () => {
    mockInvoke.mockResolvedValue([]);
    const onClose = vi.fn();
    render(<ImportWizard open onClose={onClose} />);
    await screen.findByText("No importable sources detected.");
    fireEvent.click(screen.getByRole("button", { name: /Cancel/ }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("Back returns from preview to detect", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "import_detect_sources") return Promise.resolve([source()]);
      if (cmd === "import_parse_source") return Promise.resolve([parsedSession()]);
      return Promise.resolve(undefined);
    });
    render(<ImportWizard open onClose={vi.fn()} />);
    await screen.findByText("PuTTY");
    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    await screen.findByText("10.0.0.5");

    fireEvent.click(screen.getByRole("button", { name: /Back/ }));
    expect(await screen.findByText("PuTTY")).toBeInTheDocument();
  });

  it("Done after summary calls onClose", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "import_detect_sources") return Promise.resolve([source()]);
      if (cmd === "import_parse_source") return Promise.resolve([parsedSession()]);
      if (cmd === "session_import_batch") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    const onClose = vi.fn();
    render(<ImportWizard open onClose={onClose} />);
    await screen.findByText("PuTTY");
    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    await screen.findByText("10.0.0.5");
    fireEvent.click(screen.getByRole("button", { name: /Import/ }));
    await screen.findByText("Import complete");

    fireEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
