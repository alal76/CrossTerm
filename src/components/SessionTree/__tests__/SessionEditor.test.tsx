import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
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

    // Always-visible fields should be present
    expect(screen.getByPlaceholderText("My Server")).toBeInTheDocument(); // Name
    expect(screen.getByDisplayValue("SSH")).toBeInTheDocument(); // Type select (default SSH)
    expect(screen.getByPlaceholderText("192.168.1.100")).toBeInTheDocument(); // Host
    expect(screen.getByPlaceholderText("22")).toBeInTheDocument(); // Port

    // Advanced fields are behind the collapsed "Advanced" toggle
    await user.click(screen.getByRole("button", { name: "Advanced" }));
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
    await user.click(screen.getByRole("button", { name: "Advanced" }));
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

  it("Advanced section is collapsed by default for a new session", () => {
    render(<SessionEditor onClose={onClose} />);

    expect(screen.getByRole("button", { name: "Advanced" })).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByPlaceholderText("Production")).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("web, staging")).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("Credential name or ID")).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("Commands to run after connection...")).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("Optional notes…")).not.toBeInTheDocument();
  });

  it("clicking the Advanced toggle reveals the advanced fields", async () => {
    const user = userEvent.setup();
    render(<SessionEditor onClose={onClose} />);

    const toggle = screen.getByRole("button", { name: "Advanced" });
    await user.click(toggle);

    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByPlaceholderText("Production")).toBeInTheDocument();
    expect(screen.getByPlaceholderText("web, staging")).toBeInTheDocument();
  });

  it("Advanced section starts expanded when editing a session that already has advanced data", () => {
    const session = makeSession({ group: "Staging", tags: ["prod"] });
    render(<SessionEditor session={session} onClose={onClose} />);

    expect(screen.getByRole("button", { name: "Advanced" })).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByPlaceholderText("Production")).toBeInTheDocument();
  });

  // Regression coverage: sessionConfig.ts's builders for Proxmox Console,
  // Docker Logs, and Kubernetes Port-Forward all read protocol-specific
  // identifiers (node+vmid, container ID, pod name) that the editor never
  // collected — every session of these types was structurally unable to
  // connect regardless of what the user entered, since the required field
  // was always empty. Confirmed live for Proxmox Console against a real
  // host before this fix. These three now have required-field validation
  // matching Name/Host's existing pattern.
  describe("protocol-specific required fields block Save when empty", () => {
    async function fillNameAndHost(user: ReturnType<typeof userEvent.setup>) {
      await user.type(screen.getByPlaceholderText("My Server"), "Test");
      await user.type(screen.getByPlaceholderText("192.168.1.100"), "10.0.0.1");
    }

    it("Proxmox Console requires Node and VM/CT ID", async () => {
      const user = userEvent.setup();
      render(<SessionEditor onClose={onClose} defaultType={SessionType.ProxmoxConsole} />);
      await fillNameAndHost(user);

      await user.click(screen.getByRole("button", { name: "Create" }));

      expect(screen.getByText("Node is required")).toBeInTheDocument();
      expect(screen.getByText("VM/CT ID is required")).toBeInTheDocument();
      expect(onClose).not.toHaveBeenCalled();
    });

    it("Docker Logs requires a Container ID or name", async () => {
      const user = userEvent.setup();
      render(<SessionEditor onClose={onClose} defaultType={SessionType.DockerLogs} />);
      await fillNameAndHost(user);

      await user.click(screen.getByRole("button", { name: "Create" }));

      expect(screen.getByText("Container ID or name is required")).toBeInTheDocument();
      expect(onClose).not.toHaveBeenCalled();
    });

    it("Kubernetes Port-Forward requires a Pod Name", async () => {
      const user = userEvent.setup();
      render(<SessionEditor onClose={onClose} defaultType={SessionType.KubernetesPortForward} />);
      await user.type(screen.getByPlaceholderText("My Server"), "Test");

      await user.click(screen.getByRole("button", { name: "Create" }));

      expect(screen.getByText("Pod name is required")).toBeInTheDocument();
      expect(onClose).not.toHaveBeenCalled();
    });
  });

  describe("protocol-specific fields save into protocolOptions correctly", () => {
    function setupSpy() {
      const addSessionSpy = vi.fn();
      useSessionStore.setState({ addSession: addSessionSpy } as unknown as Parameters<typeof useSessionStore.setState>[0]);
      return addSessionSpy;
    }

    it("VNC saves the password field", async () => {
      const user = userEvent.setup();
      const addSessionSpy = setupSpy();
      render(<SessionEditor onClose={onClose} defaultType={SessionType.VNC} />);

      await user.type(screen.getByPlaceholderText("My Server"), "VNC Box");
      await user.type(screen.getByPlaceholderText("192.168.1.100"), "10.0.0.2");
      await user.type(screen.getByPlaceholderText("Password"), "hunter2");
      await user.click(screen.getByRole("button", { name: "Create" }));

      const saved = addSessionSpy.mock.calls[0][0] as Session;
      expect(saved.connection.protocolOptions?.password).toBe("hunter2");
    });

    it("RDP saves both password and domain", async () => {
      const user = userEvent.setup();
      const addSessionSpy = setupSpy();
      render(<SessionEditor onClose={onClose} defaultType={SessionType.RDP} />);

      await user.type(screen.getByPlaceholderText("My Server"), "RDP Box");
      await user.type(screen.getByPlaceholderText("192.168.1.100"), "10.0.0.3");
      await user.type(screen.getByPlaceholderText("Password"), "s3cret");
      await user.type(screen.getByPlaceholderText("e.g. CORP, WORKGROUP"), "CORP");
      await user.click(screen.getByRole("button", { name: "Create" }));

      const saved = addSessionSpy.mock.calls[0][0] as Session;
      expect(saved.connection.protocolOptions?.password).toBe("s3cret");
      expect(saved.connection.protocolOptions?.domain).toBe("CORP");
    });

    it("Docker Logs saves container_id under the exact key buildDockerLogsConfig reads", async () => {
      const user = userEvent.setup();
      const addSessionSpy = setupSpy();
      render(<SessionEditor onClose={onClose} defaultType={SessionType.DockerLogs} />);

      await user.type(screen.getByPlaceholderText("My Server"), "Logs");
      await user.type(screen.getByPlaceholderText("192.168.1.100"), "10.0.0.4");
      await user.type(screen.getByPlaceholderText("e.g. web-app or a1b2c3d4e5f6"), "web-app");
      await user.click(screen.getByRole("button", { name: "Create" }));

      const saved = addSessionSpy.mock.calls[0][0] as Session;
      expect(saved.connection.protocolOptions?.container_id).toBe("web-app");
    });

    it("Kubernetes Port-Forward saves pod_name and namespace under the exact keys buildK8sPortForwardConfig reads", async () => {
      const user = userEvent.setup();
      const addSessionSpy = setupSpy();
      render(<SessionEditor onClose={onClose} defaultType={SessionType.KubernetesPortForward} />);

      await user.type(screen.getByPlaceholderText("My Server"), "PF");
      await user.type(screen.getByPlaceholderText("192.168.1.100"), "10.0.0.7");
      await user.type(screen.getByPlaceholderText("e.g. web-app-7d8f9c-x2k4p"), "web-app-7d8f9c-x2k4p");
      const nsInput = screen.getByPlaceholderText("default");
      await user.clear(nsInput);
      await user.type(nsInput, "prod");
      await user.click(screen.getByRole("button", { name: "Create" }));

      const saved = addSessionSpy.mock.calls[0][0] as Session;
      expect(saved.connection.protocolOptions?.pod_name).toBe("web-app-7d8f9c-x2k4p");
      expect(saved.connection.protocolOptions?.namespace).toBe("prod");
    });

    // NetConf's builder reads "private_key"; X11 Forward's reads "key_data"
    // — same UI field, different backend key names. Getting this wrong
    // silently breaks key-based auth for one or the other.
    it("NetConf saves the private key under 'private_key'", async () => {
      const user = userEvent.setup();
      const addSessionSpy = setupSpy();
      render(<SessionEditor onClose={onClose} defaultType={SessionType.NetConf} />);

      await user.type(screen.getByPlaceholderText("My Server"), "Router");
      await user.type(screen.getByPlaceholderText("192.168.1.100"), "10.0.0.5");
      await user.type(screen.getByPlaceholderText("-----BEGIN OPENSSH PRIVATE KEY-----"), "KEYDATA");
      await user.click(screen.getByRole("button", { name: "Create" }));

      const saved = addSessionSpy.mock.calls[0][0] as Session;
      expect(saved.connection.protocolOptions?.private_key).toBe("KEYDATA");
      expect(saved.connection.protocolOptions?.key_data).toBeUndefined();
    });

    it("X11 Forward saves the private key under 'key_data'", async () => {
      const user = userEvent.setup();
      const addSessionSpy = setupSpy();
      render(<SessionEditor onClose={onClose} defaultType={SessionType.X11Forward} />);

      await user.type(screen.getByPlaceholderText("My Server"), "X11 Box");
      await user.type(screen.getByPlaceholderText("192.168.1.100"), "10.0.0.6");
      await user.type(screen.getByPlaceholderText("-----BEGIN OPENSSH PRIVATE KEY-----"), "KEYDATA");
      await user.click(screen.getByRole("button", { name: "Create" }));

      const saved = addSessionSpy.mock.calls[0][0] as Session;
      expect(saved.connection.protocolOptions?.key_data).toBe("KEYDATA");
      expect(saved.connection.protocolOptions?.private_key).toBeUndefined();
    });

    it("Proxmox Console saves node, vmid, resource_type, realm, and password", async () => {
      const user = userEvent.setup();
      const addSessionSpy = setupSpy();
      render(<SessionEditor onClose={onClose} defaultType={SessionType.ProxmoxConsole} />);

      await user.type(screen.getByPlaceholderText("My Server"), "PVE");
      await user.type(screen.getByPlaceholderText("192.168.1.100"), "192.168.0.251");
      await user.type(screen.getByPlaceholderText("pve1"), "pve1");
      await user.type(screen.getByPlaceholderText("100"), "100");
      await user.type(screen.getByPlaceholderText("Proxmox API password"), "rootpw");
      await user.click(screen.getByRole("button", { name: "Create" }));

      const saved = addSessionSpy.mock.calls[0][0] as Session;
      expect(saved.connection.protocolOptions?.node).toBe("pve1");
      expect(saved.connection.protocolOptions?.vmid).toBe("100");
      expect(saved.connection.protocolOptions?.resource_type).toBe("qemu");
      expect(saved.connection.protocolOptions?.realm).toBe("pam");
      expect(saved.connection.protocolOptions?.password).toBe("rootpw");
    });
  });
});
