import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import QuickConnect from "@/components/Shared/QuickConnect";
import { useSessionStore } from "@/stores/sessionStore";
import { SessionType } from "@/types";
import type { Session } from "@/types";

function openQuickConnect() {
  fireEvent.keyDown(document, {
    key: "n",
    ctrlKey: true,
    shiftKey: true,
  });
}

function makeSession(overrides: Partial<Session> = {}): Session {
  return {
    id: overrides.id ?? "sess-1",
    name: overrides.name ?? "prod-server",
    type: SessionType.SSH,
    group: "default",
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

describe("QuickConnect", () => {
  beforeEach(() => {
    resetStore();
  });

  it("renders nothing when not opened", () => {
    const { container } = render(<QuickConnect />);
    expect(container.firstChild).toBeNull();
  });

  it("opens via Ctrl+Shift+N and shows input", () => {
    render(<QuickConnect />);
    openQuickConnect();

    expect(
      screen.getByPlaceholderText("ssh user@host:port")
    ).toBeInTheDocument();
  });

  it("closes on Escape", () => {
    render(<QuickConnect />);
    openQuickConnect();
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    const input = screen.getByPlaceholderText("ssh user@host:port");
    fireEvent.keyDown(input, { key: "Escape" });

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("creates session on Enter with valid input", async () => {
    const onConnect = vi.fn();
    render(<QuickConnect onConnect={onConnect} />);
    openQuickConnect();

    const input = screen.getByPlaceholderText("ssh user@host:port");
    await userEvent.type(input, "admin@myserver.com:2222");
    fireEvent.keyDown(input, { key: "Enter" });

    const state = useSessionStore.getState();
    expect(state.sessions).toHaveLength(1);
    expect(state.sessions[0].name).toBe("admin@myserver.com");
    expect(state.sessions[0].connection.host).toBe("myserver.com");
    expect(state.sessions[0].connection.port).toBe(2222);
    expect(state.openTabs).toHaveLength(1);
    expect(onConnect).toHaveBeenCalledOnce();
  });

  it("shows saved session suggestions matching input", async () => {
    useSessionStore.getState().addSession(
      makeSession({ id: "s1", name: "prod-server" })
    );

    render(<QuickConnect />);
    openQuickConnect();

    const input = screen.getByPlaceholderText("ssh user@host:port");
    await userEvent.type(input, "prod");

    expect(screen.getByText("prod-server")).toBeInTheDocument();
  });
});
