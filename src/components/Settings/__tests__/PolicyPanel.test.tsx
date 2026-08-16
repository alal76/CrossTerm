import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import PolicyPanel from "@/components/Settings/PolicyPanel";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));
vi.mock("@tauri-apps/plugin-fs", () => ({
  readTextFile: vi.fn(),
  writeTextFile: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);
const mockOpen = vi.mocked(open);
const mockSave = vi.mocked(save);
const mockReadTextFile = vi.mocked(readTextFile);
const mockWriteTextFile = vi.mocked(writeTextFile);

const DEFAULT_POLICY = {
  recording: {
    enabled: false,
    require_recording_for: [],
    storage_path: null,
    retention_days: 90,
    encrypt_recordings: false,
    notify_user: true,
    allow_user_disable: false,
  },
  max_session_duration_minutes: null,
  require_mfa_for_privileged: false,
  allowed_protocols: [],
  blocked_hosts: [],
  audit_all_commands: false,
};

describe("PolicyPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "policy_get") return Promise.resolve(DEFAULT_POLICY);
      if (cmd === "policy_update") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
  });

  it("shows a loading state before the policy resolves", () => {
    mockInvoke.mockReturnValue(new Promise(() => {}));
    render(<PolicyPanel />);
    expect(screen.getByText("Loading policy…")).toBeInTheDocument();
  });

  it("falls back to the default policy when policy_get fails", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "policy_get") return Promise.reject(new Error("no policy configured"));
      return Promise.resolve(undefined);
    });
    render(<PolicyPanel />);
    expect(await screen.findByText("Policy")).toBeInTheDocument();
    expect(screen.getByText("No restrictions — all protocols are allowed.")).toBeInTheDocument();
  });

  it("toggles session recording and shows the Saved indicator", async () => {
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    const toggle = screen.getByText("Enable session recording").closest("div")!.parentElement!.querySelector("button")!;
    fireEvent.click(toggle);

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({ config: expect.objectContaining({ recording: expect.objectContaining({ enabled: true }) }) }),
      ),
    );
    expect(await screen.findByText("Saved")).toHaveClass("opacity-100");
  });

  it("adds and removes a recording host pattern", async () => {
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    const input = screen.getByPlaceholderText("*.prod.example.com");
    fireEvent.change(input, { target: { value: "*.prod.example.com" } });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({
          config: expect.objectContaining({
            recording: expect.objectContaining({ require_recording_for: [{ "0": "*.prod.example.com" }] }),
          }),
        }),
      ),
    );
    expect(await screen.findByText("*.prod.example.com")).toBeInTheDocument();

    fireEvent.click(screen.getByLabelText("Remove *.prod.example.com"));
    await waitFor(() => expect(screen.getAllByText("No patterns added").length).toBeGreaterThan(0));
  });

  it("does not add a duplicate or blank pattern", async () => {
    render(<PolicyPanel />);
    await screen.findByText("Policy");
    const input = screen.getByPlaceholderText("*.prod.example.com");

    fireEvent.change(input, { target: { value: "  " } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(screen.getAllByText("No patterns added").length).toBeGreaterThan(0);
  });

  it("sets a recording storage path and retention days", async () => {
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    fireEvent.change(screen.getByPlaceholderText("/var/log/crossterm/recordings"), {
      target: { value: "/data/recordings" },
    });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({
          config: expect.objectContaining({ recording: expect.objectContaining({ storage_path: "/data/recordings" }) }),
        }),
      ),
    );

    fireEvent.change(screen.getByDisplayValue("90"), { target: { value: "30" } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({
          config: expect.objectContaining({ recording: expect.objectContaining({ retention_days: 30 }) }),
        }),
      ),
    );
  });

  it("toggles encrypt/notify/allow-disable recording switches", async () => {
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    const encryptToggle = screen.getByText("Encrypt recordings").closest("div")!.parentElement!.querySelector("button")!;
    fireEvent.click(encryptToggle);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({
          config: expect.objectContaining({ recording: expect.objectContaining({ encrypt_recordings: true }) }),
        }),
      ),
    );

    const notifyToggle = screen.getByText("Notify user").closest("div")!.parentElement!.querySelector("button")!;
    fireEvent.click(notifyToggle);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({
          config: expect.objectContaining({ recording: expect.objectContaining({ notify_user: false }) }),
        }),
      ),
    );

    const allowDisableToggle = screen
      .getByText("Allow user to disable recording")
      .closest("div")!.parentElement!.querySelector("button")!;
    fireEvent.click(allowDisableToggle);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({
          config: expect.objectContaining({ recording: expect.objectContaining({ allow_user_disable: true }) }),
        }),
      ),
    );
  });

  it("adds a blocked host pattern", async () => {
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    const input = screen.getByPlaceholderText("*.darknet.example");
    fireEvent.change(input, { target: { value: "*.darknet.example" } });
    fireEvent.click(input.parentElement!.querySelector("button")!);

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({ config: expect.objectContaining({ blocked_hosts: [{ "0": "*.darknet.example" }] }) }),
      ),
    );
  });

  it("toggles allowed protocols and updates the restriction hint", async () => {
    render(<PolicyPanel />);
    await screen.findByText("Policy");
    expect(screen.getByText("No restrictions — all protocols are allowed.")).toBeInTheDocument();

    fireEvent.click(screen.getByText("SSH"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({ config: expect.objectContaining({ allowed_protocols: ["ssh"] }) }),
      ),
    );
    expect(await screen.findByText("All other protocols will be blocked.")).toBeInTheDocument();

    fireEvent.click(screen.getByText("SSH"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({ config: expect.objectContaining({ allowed_protocols: [] }) }),
      ),
    );
  });

  it("sets and clears the max session duration", async () => {
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    const input = screen.getByPlaceholderText("Unlimited");
    fireEvent.change(input, { target: { value: "120" } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({ config: expect.objectContaining({ max_session_duration_minutes: 120 }) }),
      ),
    );

    fireEvent.change(input, { target: { value: "" } });
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({ config: expect.objectContaining({ max_session_duration_minutes: null }) }),
      ),
    );
  });

  it("toggles MFA-for-privileged and audit-all-commands", async () => {
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    fireEvent.click(screen.getByText("Require MFA for privileged sessions").closest("div")!.parentElement!.querySelector("button")!);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({ config: expect.objectContaining({ require_mfa_for_privileged: true }) }),
      ),
    );

    fireEvent.click(screen.getByText("Audit all commands").closest("div")!.parentElement!.querySelector("button")!);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({ config: expect.objectContaining({ audit_all_commands: true }) }),
      ),
    );
  });

  it("exports the policy as JSON", async () => {
    mockSave.mockResolvedValue("/tmp/crossterm-policy.json");
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    fireEvent.click(screen.getByText("Export Policy JSON"));
    await waitFor(() =>
      expect(mockWriteTextFile).toHaveBeenCalledWith(
        "/tmp/crossterm-policy.json",
        expect.stringContaining("\"recording\""),
      ),
    );
  });

  it("does not write a file when export is cancelled", async () => {
    mockSave.mockResolvedValue(null);
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    fireEvent.click(screen.getByText("Export Policy JSON"));
    await waitFor(() => expect(mockSave).toHaveBeenCalled());
    expect(mockWriteTextFile).not.toHaveBeenCalled();
  });

  it("imports a policy JSON file and applies it", async () => {
    mockOpen.mockResolvedValue("/tmp/imported-policy.json");
    mockReadTextFile.mockResolvedValue(
      JSON.stringify({ ...DEFAULT_POLICY, audit_all_commands: true }),
    );
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    fireEvent.click(screen.getByText("Import Policy JSON"));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "policy_update",
        expect.objectContaining({ config: expect.objectContaining({ audit_all_commands: true }) }),
      ),
    );
  });

  it("ignores an import when no file is selected", async () => {
    mockOpen.mockResolvedValue(null);
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    fireEvent.click(screen.getByText("Import Policy JSON"));
    await waitFor(() => expect(mockOpen).toHaveBeenCalled());
    expect(mockReadTextFile).not.toHaveBeenCalled();
  });

  it("ignores an import with invalid JSON without crashing", async () => {
    mockOpen.mockResolvedValue("/tmp/bad.json");
    mockReadTextFile.mockResolvedValue("not json");
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    fireEvent.click(screen.getByText("Import Policy JSON"));
    await waitFor(() => expect(mockReadTextFile).toHaveBeenCalled());
    // Still on the same screen, no crash.
    expect(screen.getByText("Policy")).toBeInTheDocument();
  });

  it("hides the Saved indicator again after the timeout", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    render(<PolicyPanel />);
    await screen.findByText("Policy");

    const toggle = screen.getByText("Audit all commands").closest("div")!.parentElement!.querySelector("button")!;
    fireEvent.click(toggle);
    await vi.waitFor(() => expect(screen.getByText("Saved")).toHaveClass("opacity-100"));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2600);
    });
    expect(screen.getByText("Saved")).toHaveClass("opacity-0");
    vi.useRealTimers();
  });
});
