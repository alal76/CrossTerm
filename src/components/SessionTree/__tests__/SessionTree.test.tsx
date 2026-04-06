import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import "@/i18n";
import SessionTree from "@/components/SessionTree/SessionTree";
import { useSessionStore } from "@/stores/sessionStore";
import { SessionType } from "@/types";
import type { Session } from "@/types";

function makeSession(overrides: Partial<Session> = {}): Session {
  return {
    id: overrides.id ?? "sess-1",
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

describe("SessionTree", () => {
  beforeEach(() => {
    resetStore();
    vi.clearAllMocks();
  });

  it("renders sessions grouped by folder hierarchy", () => {
    useSessionStore.setState({
      sessions: [
        makeSession({ id: "s1", name: "Prod Web", group: "Production/AWS" }),
        makeSession({ id: "s2", name: "Prod DB", group: "Production/AWS" }),
        makeSession({ id: "s3", name: "Dev API", group: "Development" }),
      ],
    });

    render(<SessionTree />);

    // Folder headers should be visible (auto-expanded on mount)
    expect(screen.getByText("Production")).toBeInTheDocument();
    expect(screen.getByText("AWS")).toBeInTheDocument();
    expect(screen.getByText("Development")).toBeInTheDocument();

    // Sessions should appear within their folders
    expect(screen.getByText("Prod Web")).toBeInTheDocument();
    expect(screen.getByText("Prod DB")).toBeInTheDocument();
    expect(screen.getByText("Dev API")).toBeInTheDocument();
  });

  it("search input filters sessions by name", async () => {
    useSessionStore.setState({
      sessions: [
        makeSession({ id: "s1", name: "Alpha Server", group: "default" }),
        makeSession({ id: "s2", name: "Beta Server", group: "default" }),
        makeSession({ id: "s3", name: "Gamma Host", group: "default" }),
      ],
    });

    render(<SessionTree />);

    const searchInput = screen.getByPlaceholderText("Search sessions…");
    await userEvent.type(searchInput, "Beta");

    expect(screen.getByText("Beta Server")).toBeInTheDocument();
    expect(screen.queryByText("Alpha Server")).not.toBeInTheDocument();
    expect(screen.queryByText("Gamma Host")).not.toBeInTheDocument();
  });

  it("empty state renders when no sessions exist", () => {
    render(<SessionTree />);

    expect(screen.getByText("No sessions yet")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Create a new session or import from a file to get started."
      )
    ).toBeInTheDocument();
    expect(screen.getByText("New Session")).toBeInTheDocument();
    expect(screen.getByText("Import")).toBeInTheDocument();
  });

  it("FT-C-03: right-click shows context menu", async () => {
    useSessionStore.setState({
      sessions: [
        makeSession({ id: "s1", name: "Ctx Server", group: "default" }),
      ],
    });

    render(<SessionTree />);

    const sessionBtn = screen.getByText("Ctx Server");
    // Fire a contextmenu event (right-click)
    fireEvent.contextMenu(sessionBtn, { clientX: 100, clientY: 200 });

    // Context menu items for a session should appear
    expect(screen.getByText("Connect")).toBeInTheDocument();
    expect(screen.getByText("Edit")).toBeInTheDocument();
    expect(screen.getByText("Duplicate")).toBeInTheDocument();
    expect(screen.getByText("Delete")).toBeInTheDocument();
  });

  it("FT-C-04: click session calls onSessionSelect", async () => {
    const onSessionSelect = vi.fn();

    useSessionStore.setState({
      sessions: [
        makeSession({ id: "s1", name: "Clickable Server", group: "default" }),
      ],
    });

    render(<SessionTree onSessionSelect={onSessionSelect} />);

    const sessionBtn = screen.getByText("Clickable Server");
    await userEvent.click(sessionBtn);

    expect(onSessionSelect).toHaveBeenCalledOnce();
    expect(onSessionSelect).toHaveBeenCalledWith(
      expect.objectContaining({ id: "s1", name: "Clickable Server" })
    );
  });
});
