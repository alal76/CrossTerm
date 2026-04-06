import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@/i18n";
import SessionEditor from "@/components/SessionTree/SessionEditor";
import { useSessionStore } from "@/stores/sessionStore";
import { SessionType } from "@/types";
import type { Session } from "@/types";

function makeSession(overrides: Partial<Session> = {}): Session {
  return {
    id: overrides.id ?? "sess-edit-1",
    name: overrides.name ?? "Test Server",
    type: SessionType.SSH,
    group: overrides.group ?? "default",
    tags: [],
    connection: { host: "10.0.0.1", port: 22 },
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    autoReconnect: false,
    keepAliveIntervalSeconds: 60,
    ...overrides,
  };
}

function resetStore() {
  useSessionStore.setState({
    sessions: [],
    sessionFolders: [],
    openTabs: [],
    activeTabId: null,
    splitPane: null,
    favorites: [],
    recentSessions: [],
  });
}

describe("SessionEditor", () => {
  const onClose = vi.fn();

  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  // FT-C-06: Renders all form fields. Validates required fields on submit.
  it("FT-C-06: renders all form fields and validates required fields", async () => {
    const user = userEvent.setup();
    render(<SessionEditor onClose={onClose} />);

    // Dialog should be open
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    // All key form fields should be present
    expect(screen.getByPlaceholderText("My Server")).toBeInTheDocument(); // Name
    expect(screen.getByDisplayValue("SSH")).toBeInTheDocument(); // Type select (default SSH)
    expect(screen.getByPlaceholderText("192.168.1.100")).toBeInTheDocument(); // Host
    expect(screen.getByPlaceholderText("22")).toBeInTheDocument(); // Port
    expect(screen.getByPlaceholderText("Production")).toBeInTheDocument(); // Group
    expect(screen.getByPlaceholderText("web, staging")).toBeInTheDocument(); // Tags
    expect(screen.getByPlaceholderText("Credential name or ID")).toBeInTheDocument(); // Credential
    expect(
      screen.getByPlaceholderText("Commands to run after connection...")
    ).toBeInTheDocument(); // Startup Script
    expect(screen.getByPlaceholderText("Optional notes…")).toBeInTheDocument(); // Notes

    // Submit without filling required fields
    const createButton = screen.getByRole("button", { name: "Create" });
    await user.click(createButton);

    // Name is required
    expect(screen.getByText("Name is required")).toBeInTheDocument();
    // Host is required for SSH
    expect(screen.getByText("Host is required")).toBeInTheDocument();

    // onClose should NOT have been called (validation failed)
    expect(onClose).not.toHaveBeenCalled();
  });

  // FT-C-07: Port auto-populates when session type changes
  it("FT-C-07: port auto-populates when session type changes", async () => {
    const user = userEvent.setup();
    render(<SessionEditor onClose={onClose} />);

    const portInput = screen.getByPlaceholderText("22") as HTMLInputElement;
    const typeSelect = screen.getByDisplayValue("SSH") as HTMLSelectElement;

    // Default SSH -> port 22
    expect(portInput.value).toBe("22");

    // Change to RDP -> port 3389
    await user.selectOptions(typeSelect, SessionType.RDP);
    expect(portInput.value).toBe("3389");

    // Change to VNC -> port 5900
    await user.selectOptions(typeSelect, SessionType.VNC);
    expect(portInput.value).toBe("5900");

    // Change to Telnet -> port 23
    await user.selectOptions(typeSelect, SessionType.Telnet);
    expect(portInput.value).toBe("23");

    // Change to SFTP -> port 22
    await user.selectOptions(typeSelect, SessionType.SFTP);
    expect(portInput.value).toBe("22");
  });

  // FT-C-08: Submit creates session via store with form data
  it("FT-C-08: submit creates session via addSession with form data", async () => {
    const user = userEvent.setup();
    const addSessionSpy = vi.fn();
    useSessionStore.setState({ addSession: addSessionSpy } as unknown as Parameters<typeof useSessionStore.setState>[0]);

    render(<SessionEditor onClose={onClose} />);

    // Fill in form fields
    const nameInput = screen.getByPlaceholderText("My Server");
    const hostInput = screen.getByPlaceholderText("192.168.1.100");
    const groupInput = screen.getByPlaceholderText("Production");
    const tagsInput = screen.getByPlaceholderText("web, staging");

    await user.type(nameInput, "My Production Server");
    await user.type(hostInput, "prod.example.com");
    await user.type(groupInput, "Production/AWS");
    await user.type(tagsInput, "web, prod");

    // Submit
    const createButton = screen.getByRole("button", { name: "Create" });
    await user.click(createButton);

    // addSession should have been called with the correct data
    expect(addSessionSpy).toHaveBeenCalledTimes(1);
    const createdSession = addSessionSpy.mock.calls[0][0] as Session;
    expect(createdSession.name).toBe("My Production Server");
    expect(createdSession.type).toBe(SessionType.SSH);
    expect(createdSession.connection.host).toBe("prod.example.com");
    expect(createdSession.connection.port).toBe(22);
    expect(createdSession.group).toBe("Production/AWS");
    expect(createdSession.tags).toEqual(["web", "prod"]);

    // onClose should have been called
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("FT-C-08: edit mode populates form with existing session data", () => {
    const session = makeSession({
      name: "Existing Server",
      connection: { host: "10.0.0.5", port: 2222 },
      group: "Staging",
    });

    render(<SessionEditor session={session} onClose={onClose} />);

    expect(screen.getByRole("dialog")).toHaveAttribute("aria-label", "Edit Session");
    expect((screen.getByPlaceholderText("My Server") as HTMLInputElement).value).toBe(
      "Existing Server"
    );
    expect((screen.getByPlaceholderText("192.168.1.100") as HTMLInputElement).value).toBe(
      "10.0.0.5"
    );
  });
});
