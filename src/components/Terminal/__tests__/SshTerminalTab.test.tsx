import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import "@/i18n";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useSessionStore } from "@/stores/sessionStore";
import { useVaultStore } from "@/stores/vaultStore";
import { ConnectionStatus } from "@/types";

vi.mock("@/components/Terminal/SshTerminalView", () => ({
  default: ({ connectionId }: { connectionId: string }) => (
    <div data-testid="ssh-terminal-view">terminal:{connectionId}</div>
  ),
}));
vi.mock("@/components/Terminal/ReconnectOverlay", () => ({
  default: ({ reason, onReconnect, onClose }: { reason: string; onReconnect: () => void; onClose: () => void }) => (
    <div data-testid="reconnect-overlay">
      <span>{reason}</span>
      <button onClick={onReconnect}>Reconnect now</button>
      <button onClick={onClose}>Close overlay</button>
    </div>
  ),
}));

import SshTerminalTab from "@/components/Terminal/SshTerminalTab";

const mockInvoke = vi.mocked(invoke);
const listenCallbacks = new Map<string, (event: unknown) => void>();
// eslint-disable-next-line @typescript-eslint/no-explicit-any
(listen as any).mockImplementation((eventName: string, cb: (event: unknown) => void) => {
  listenCallbacks.set(eventName, cb);
  return Promise.resolve(() => listenCallbacks.delete(eventName));
});

const baseProps = {
  sessionId: "sess-1",
  isActive: true,
  host: "example.com",
  port: 22,
  username: "root",
  auth: { type: "password" as const, password: "secret" },
};

describe("SshTerminalTab", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listenCallbacks.clear();
    mockInvoke.mockResolvedValue(undefined);
    useSessionStore.setState({
      openTabs: [{ id: "tab-1", sessionId: "sess-1", order: 0, status: ConnectionStatus.Connecting } as never],
      updateTabStatus: vi.fn((tabId, status) => {
        useSessionStore.setState((s) => ({
          openTabs: s.openTabs.map((t) => (t.id === tabId ? { ...t, status } : t)),
        }));
      }),
      updateSession: vi.fn(),
    });
    useVaultStore.setState({
      activeVaultId: null,
      getCredential: vi.fn().mockResolvedValue(null),
      addCredential: vi.fn().mockResolvedValue("cred-1"),
    });
  });

  it("shows the connecting spinner then the terminal once connected", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });
    render(<SshTerminalTab {...baseProps} />);

    await waitFor(() => expect(screen.getByTestId("ssh-terminal-view")).toBeInTheDocument());
    expect(mockInvoke).toHaveBeenCalledWith(
      "ssh_connect",
      expect.objectContaining({ sessionId: "sess-1", host: "example.com", port: 22, username: "root" }),
    );
  });

  it("marks the tab connected in the session store on success", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });
    render(<SshTerminalTab {...baseProps} />);
    await waitFor(() => expect(screen.getByTestId("ssh-terminal-view")).toBeInTheDocument());
    expect(useSessionStore.getState().openTabs[0].status).toBe(ConnectionStatus.Connected);
  });

  it("shows an error state with retry when ssh_connect rejects", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") return Promise.reject(new Error("connection refused"));
      return Promise.resolve(undefined);
    });
    render(<SshTerminalTab {...baseProps} />);
    expect(await screen.findByText(/connection refused/)).toBeInTheDocument();
    expect(screen.getByText("Retry")).toBeInTheDocument();
  });

  it("offers the forget-host-key option when the error mentions a host key change", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") return Promise.reject(new Error("Host key changed for example.com"));
      return Promise.resolve(undefined);
    });
    render(<SshTerminalTab {...baseProps} />);
    expect(await screen.findByText("Forget host key & retry")).toBeInTheDocument();
  });

  it("forget-host-key button calls ssh_forget_host_key then retries", async () => {
    let connectAttempt = 0;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") {
        connectAttempt += 1;
        return connectAttempt === 1
          ? Promise.reject(new Error("Host key changed for example.com"))
          : Promise.resolve("conn-1");
      }
      if (cmd === "ssh_forget_host_key") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    render(<SshTerminalTab {...baseProps} />);
    fireEvent.click(await screen.findByText("Forget host key & retry"));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("ssh_forget_host_key", { host: "example.com", port: 22 }));
    await waitFor(() => expect(screen.getByTestId("ssh-terminal-view")).toBeInTheDocument());
  });

  // Regression coverage: KnownHostsDiff was fully built and tested but never
  // mounted — a host-key mismatch only ever showed a generic error message,
  // never the actual old/new fingerprint diff, even though the backend now
  // emits that detail via ssh:host_key_changed.
  describe("KnownHostsDiff (host key mismatch detail)", () => {
    it("shows the real fingerprint diff when ssh:host_key_changed fires", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.reject(new Error("Host key changed for example.com"));
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);
      await waitFor(() => expect(listenCallbacks.has("ssh:host_key_changed")).toBe(true));

      act(() => {
        listenCallbacks.get("ssh:host_key_changed")?.({
          payload: {
            host: "example.com",
            port: 22,
            old_key_type: "ssh-ed25519",
            old_fingerprint: "SHA256:OLD",
            new_key_type: "ssh-rsa",
            new_fingerprint: "SHA256:NEW",
            detected_at: "2026-01-01T00:00:00Z",
          },
        });
      });

      expect(await screen.findByText(/Host key mismatch detected for example.com:22/)).toBeInTheDocument();
      expect(screen.getByText("SHA256:OLD")).toBeInTheDocument();
      expect(screen.getByText("SHA256:NEW")).toBeInTheDocument();
    });

    it("'Accept new key' forgets the old key and retries the connection", async () => {
      let connectAttempt = 0;
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") {
          connectAttempt += 1;
          return connectAttempt === 1
            ? Promise.reject(new Error("Host key changed for example.com"))
            : Promise.resolve("conn-1");
        }
        if (cmd === "ssh_forget_host_key") return Promise.resolve(undefined);
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);
      await waitFor(() => expect(listenCallbacks.has("ssh:host_key_changed")).toBe(true));

      act(() => {
        listenCallbacks.get("ssh:host_key_changed")?.({
          payload: {
            host: "example.com",
            port: 22,
            old_key_type: "ssh-ed25519",
            old_fingerprint: "SHA256:OLD",
            new_key_type: "ssh-rsa",
            new_fingerprint: "SHA256:NEW",
            detected_at: "2026-01-01T00:00:00Z",
          },
        });
      });

      fireEvent.click(await screen.findByText("Accept new key"));

      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("ssh_forget_host_key", { host: "example.com", port: 22 }));
      await waitFor(() => expect(screen.getByTestId("ssh-terminal-view")).toBeInTheDocument());
    });
  });

  it("shows the credential retry form and resubmits with a password", async () => {
    let connectAttempt = 0;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") {
        connectAttempt += 1;
        return connectAttempt === 1
          ? Promise.reject(new Error("auth failed"))
          : Promise.resolve("conn-1");
      }
      return Promise.resolve(undefined);
    });
    render(<SshTerminalTab {...baseProps} />);
    fireEvent.click(await screen.findByText("Provide Credentials"));

    fireEvent.change(screen.getByPlaceholderText("Password"), { target: { value: "newpass" } });
    fireEvent.click(screen.getByText("Connect"));

    await waitFor(() => expect(screen.getByTestId("ssh-terminal-view")).toBeInTheDocument());
  });

  it("resolves a vault credential when auth type is none and credentialRef is set", async () => {
    useVaultStore.setState({
      getCredential: vi.fn().mockResolvedValue({
        credential_type: "password",
        username: "root",
        data: { password: "vault-pass" },
      }),
    });
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });
    render(
      <SshTerminalTab
        {...baseProps}
        auth={{ type: "none" }}
        credentialRef="cred-1"
      />,
    );
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith(
        "ssh_connect",
        expect.objectContaining({ auth: { type: "password", password: "vault-pass" } }),
      ),
    );
  });

  it("shows an interactive auth prompt and submits responses", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") return new Promise(() => {});
      if (cmd === "ssh_auth_respond") return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    render(<SshTerminalTab {...baseProps} />);

    await waitFor(() => expect(listenCallbacks.has("ssh:auth_prompt")).toBe(true));
    act(() => {
      listenCallbacks.get("ssh:auth_prompt")?.({
        payload: {
          connection_id: "conn-pending",
          name: "keyboard-interactive",
          instructions: "Enter your PIN",
          prompts: [{ prompt: "PIN:", echo: false }],
        },
      });
    });

    expect(await screen.findByText("Enter your PIN")).toBeInTheDocument();
    fireEvent.change(document.querySelector('input[type="password"]')!, { target: { value: "1234" } });
    fireEvent.click(screen.getByText("Authenticate"));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("ssh_auth_respond", {
        connectionId: "conn-pending",
        responses: ["1234"],
      }),
    );
  });

  it("shows the disconnect overlay and reconnects", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });
    render(<SshTerminalTab {...baseProps} />);
    await waitFor(() => expect(screen.getByTestId("ssh-terminal-view")).toBeInTheDocument());

    act(() => {
      listenCallbacks.get("ssh:disconnected")?.({
        payload: { connection_id: "conn-1", reason: "network timeout" },
      });
    });
    expect(await screen.findByTestId("reconnect-overlay")).toBeInTheDocument();
    expect(screen.getByText("network timeout")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Reconnect now"));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("ssh_connect", expect.objectContaining({ sessionId: "sess-1" })));
  });

  it("shows the save-to-vault offer after a successful password login and saves on click", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });
    render(<SshTerminalTab {...baseProps} />);
    // The connect effect reads authSuccessInfo.current synchronously right
    // after ssh_connect resolves, so this must fire before that microtask
    // runs — i.e. before any await back in this test.
    act(() => {
      listenCallbacks.get("ssh:auth_success")?.({ payload: { auth_method: "password" } });
    });

    await waitFor(() => expect(screen.getByText("Save credentials to vault?")).toBeInTheDocument());

    useVaultStore.setState({ activeVaultId: "vault-1" });
    fireEvent.click(screen.getByText("Save"));

    await waitFor(() => expect(useVaultStore.getState().addCredential).toHaveBeenCalled());
  });

  it("dismisses the save offer without saving", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ssh_connect") return Promise.resolve("conn-1");
      return Promise.resolve(undefined);
    });
    render(<SshTerminalTab {...baseProps} />);
    act(() => {
      listenCallbacks.get("ssh:auth_success")?.({ payload: { auth_method: "password" } });
    });
    await waitFor(() => expect(screen.getByText("Save credentials to vault?")).toBeInTheDocument());

    fireEvent.click(screen.getByText("Dismiss"));
    expect(screen.queryByText("Save credentials to vault?")).not.toBeInTheDocument();
  });

  describe("policy-driven recording", () => {
    it("starts recording and shows the compliance banner when policy requires it and notify_user is set", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "policy_check_recording_required") return Promise.resolve(true);
        if (cmd === "recording_start") return Promise.resolve("rec-1");
        if (cmd === "policy_get") {
          return Promise.resolve({ recording: { notify_user: true, allow_user_disable: true } });
        }
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);

      await waitFor(() =>
        expect(mockInvoke).toHaveBeenCalledWith("recording_start", expect.objectContaining({ sessionId: "sess-1" })),
      );
      expect(await screen.findByText(/This session is being recorded/)).toBeInTheDocument();
      expect(screen.getByText("Stop recording")).toBeInTheDocument();
    });

    it("does not show the banner when policy requires recording but notify_user is false", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "policy_check_recording_required") return Promise.resolve(true);
        if (cmd === "recording_start") return Promise.resolve("rec-1");
        if (cmd === "policy_get") {
          return Promise.resolve({ recording: { notify_user: false, allow_user_disable: false } });
        }
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);

      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("recording_start", expect.anything()));
      expect(screen.queryByText(/This session is being recorded/)).not.toBeInTheDocument();
    });

    it("does not start recording when policy does not require it for this host", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "policy_check_recording_required") return Promise.resolve(false);
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);

      await waitFor(() => expect(screen.getByTestId("ssh-terminal-view")).toBeInTheDocument());
      expect(mockInvoke).not.toHaveBeenCalledWith("recording_start", expect.anything());
    });

    it("streams ssh:output into the active recording via recording_append", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "policy_check_recording_required") return Promise.resolve(true);
        if (cmd === "recording_start") return Promise.resolve("rec-1");
        if (cmd === "policy_get") return Promise.resolve({ recording: { notify_user: true, allow_user_disable: true } });
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);
      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("recording_start", expect.anything()));

      act(() => {
        listenCallbacks.get("ssh:output")?.({ payload: { connection_id: "conn-1", data: "ls -la\r\n" } });
      });

      await waitFor(() =>
        expect(mockInvoke).toHaveBeenCalledWith("recording_append", { recordingId: "rec-1", data: "ls -la\r\n" }),
      );
    });

    it("stops recording via the banner's Stop recording action", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "policy_check_recording_required") return Promise.resolve(true);
        if (cmd === "recording_start") return Promise.resolve("rec-1");
        if (cmd === "policy_get") return Promise.resolve({ recording: { notify_user: true, allow_user_disable: true } });
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);
      await screen.findByText(/This session is being recorded/);

      fireEvent.click(screen.getByText("Stop recording"));

      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("recording_stop", { recordingId: "rec-1" }));
      expect(screen.queryByText(/This session is being recorded/)).not.toBeInTheDocument();
    });

    it("stops recording when the connection disconnects", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "policy_check_recording_required") return Promise.resolve(true);
        if (cmd === "recording_start") return Promise.resolve("rec-1");
        if (cmd === "policy_get") return Promise.resolve({ recording: { notify_user: true, allow_user_disable: true } });
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);
      await screen.findByText(/This session is being recorded/);

      act(() => {
        listenCallbacks.get("ssh:disconnected")?.({ payload: { connection_id: "conn-1", reason: "timeout" } });
      });

      await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("recording_stop", { recordingId: "rec-1" }));
    });

    it("does not fail the connection when the policy check itself errors", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "policy_check_recording_required") return Promise.reject(new Error("policy backend unavailable"));
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);

      expect(await screen.findByTestId("ssh-terminal-view")).toBeInTheDocument();
      expect(mockInvoke).not.toHaveBeenCalledWith("recording_start", expect.anything());
    });
  });

  // Regression coverage: RecordingControls was fully built and tested but
  // had no mount point — there was no way to manually start a recording
  // outside of policy-mandated ones.
  describe("manual recording (RecordingControls)", () => {
    it("offers the manual Start Recording control when policy does not require recording", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "policy_check_recording_required") return Promise.resolve(false);
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);

      expect(await screen.findByText("Start Recording")).toBeInTheDocument();
    });

    it("streams ssh:output into a manually-started recording via recording_append", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "policy_check_recording_required") return Promise.resolve(false);
        if (cmd === "recording_start") return Promise.resolve("rec-manual");
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);

      fireEvent.click(await screen.findByText("Start Recording"));
      await waitFor(() =>
        expect(mockInvoke).toHaveBeenCalledWith("recording_start", expect.objectContaining({ sessionId: "sess-1" })),
      );

      act(() => {
        listenCallbacks.get("ssh:output")?.({ payload: { connection_id: "conn-1", data: "whoami\r\n" } });
      });

      await waitFor(() =>
        expect(mockInvoke).toHaveBeenCalledWith("recording_append", { recordingId: "rec-manual", data: "whoami\r\n" }),
      );
    });

    it("hides the manual control once a policy-mandated recording is already active", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "policy_check_recording_required") return Promise.resolve(true);
        if (cmd === "recording_start") return Promise.resolve("rec-1");
        if (cmd === "policy_get") return Promise.resolve({ recording: { notify_user: true, allow_user_disable: true } });
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);

      await screen.findByText(/This session is being recorded/);
      expect(screen.queryByText("Start Recording")).not.toBeInTheDocument();
    });
  });

  // Regression coverage: CommandAssistant was fully built and tested but had
  // no mount point in the running terminal — there was no way to open it or
  // to insert its suggested command into the session.
  describe("AI command assistant (CommandAssistant)", () => {
    it("opens the assistant and inserts a suggested command via ssh_write", async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === "ssh_connect") return Promise.resolve("conn-1");
        if (cmd === "ai_check_available") return Promise.resolve(true);
        if (cmd === "ai_suggest_command") {
          return Promise.resolve([
            { command: "ls -la", explanation: "List all files", risk_level: "safe" },
          ]);
        }
        return Promise.resolve(undefined);
      });
      render(<SshTerminalTab {...baseProps} />);
      await waitFor(() => expect(screen.getByTestId("ssh-terminal-view")).toBeInTheDocument());

      fireEvent.click(screen.getByText("AI Assistant"));
      const input = await screen.findByPlaceholderText(/Ask AI/);
      fireEvent.change(input, { target: { value: "find large files" } });
      fireEvent.click(screen.getByText("Suggest"));

      fireEvent.click(await screen.findByText("Insert"));

      expect(mockInvoke).toHaveBeenCalledWith("ssh_write", { connectionId: "conn-1", data: "ls -la" });
    });
  });
});
