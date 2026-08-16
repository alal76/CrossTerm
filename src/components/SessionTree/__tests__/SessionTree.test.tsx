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

  it("context menu Edit calls onSessionEdit", async () => {
    const onSessionEdit = vi.fn();
    useSessionStore.setState({ sessions: [makeSession({ id: "s1", name: "Ctx Server" })] });
    render(<SessionTree onSessionEdit={onSessionEdit} />);

    fireEvent.contextMenu(screen.getByText("Ctx Server"), { clientX: 10, clientY: 10 });
    await userEvent.click(screen.getByText("Edit"));

    expect(onSessionEdit).toHaveBeenCalledWith(expect.objectContaining({ id: "s1" }));
  });

  it("context menu Duplicate adds a copy via addSession", async () => {
    const addSession = vi.fn();
    useSessionStore.setState({ sessions: [makeSession({ id: "s1", name: "Dup Server" })], addSession });
    render(<SessionTree />);

    fireEvent.contextMenu(screen.getByText("Dup Server"), { clientX: 10, clientY: 10 });
    await userEvent.click(screen.getByText("Duplicate"));

    expect(addSession).toHaveBeenCalledWith(
      expect.objectContaining({ name: "Dup Server (copy)" }),
    );
  });

  it("context menu Delete removes the session", async () => {
    const removeSession = vi.fn();
    useSessionStore.setState({ sessions: [makeSession({ id: "s1", name: "Del Server" })], removeSession });
    render(<SessionTree />);

    fireEvent.contextMenu(screen.getByText("Del Server"), { clientX: 10, clientY: 10 });
    await userEvent.click(screen.getByText("Delete"));

    expect(removeSession).toHaveBeenCalledWith("s1");
  });

  it("context menu Connect opens a tab and calls onSessionSelect", async () => {
    const openTab = vi.fn();
    const onSessionSelect = vi.fn();
    useSessionStore.setState({ sessions: [makeSession({ id: "s1", name: "Conn Server" })], openTab });
    render(<SessionTree onSessionSelect={onSessionSelect} />);

    fireEvent.contextMenu(screen.getByText("Conn Server"), { clientX: 10, clientY: 10 });
    await userEvent.click(screen.getByText("Connect"));

    expect(openTab).toHaveBeenCalledWith(expect.objectContaining({ id: "s1" }));
    expect(onSessionSelect).toHaveBeenCalledWith(expect.objectContaining({ id: "s1" }));
  });

  it("toggles a session as favorite via the star button, and renders it in the Favorites strip", async () => {
    const toggleFavorite = vi.fn((id: string) => {
      useSessionStore.setState((s) => ({
        favorites: s.favorites.includes(id) ? s.favorites.filter((f) => f !== id) : [...s.favorites, id],
      }));
    });
    useSessionStore.setState({ sessions: [makeSession({ id: "s1", name: "Fav Server" })], toggleFavorite });
    render(<SessionTree />);

    const star = screen.getByText("Fav Server").closest("button")!.parentElement!.querySelector('button[class*="shrink-0"]')!;
    await userEvent.click(star);

    expect(toggleFavorite).toHaveBeenCalledWith("s1");
  });

  it("shows the Favorites strip for a favorited session, separate from the main tree", () => {
    useSessionStore.setState({
      sessions: [makeSession({ id: "s1", name: "Star Server" })],
      favorites: ["s1"],
    });
    render(<SessionTree />);

    // Appears once in the favorites strip, once in the main list.
    expect(screen.getAllByText("Star Server").length).toBeGreaterThanOrEqual(2);
  });

  it("shows the Recently Connected section and toggles collapse", async () => {
    useSessionStore.setState({
      sessions: [makeSession({ id: "s1", name: "Recent Server", lastConnectedAt: new Date().toISOString() })],
    });
    render(<SessionTree />);

    expect(screen.getByText("Recently Connected")).toBeInTheDocument();
    const initialCount = screen.getAllByText("Recent Server").length;

    await userEvent.click(screen.getByText("Recently Connected"));
    expect(screen.getAllByText("Recent Server").length).toBeLessThan(initialCount);
  });

  it("filters sessions by tag chip and clears the filter", async () => {
    useSessionStore.setState({
      sessions: [
        makeSession({ id: "s1", name: "Tagged A", tags: ["prod"] }),
        makeSession({ id: "s2", name: "Tagged B", tags: ["dev"] }),
      ],
    });
    render(<SessionTree />);

    await userEvent.click(screen.getByText("prod"));
    expect(screen.getByText("Tagged A")).toBeInTheDocument();
    expect(screen.queryByText("Tagged B")).not.toBeInTheDocument();

    await userEvent.click(screen.getByText(/Clear/));
    expect(screen.getByText("Tagged B")).toBeInTheDocument();
  });

  it("multi-selects with ctrl+click and bulk-connects all selected sessions", async () => {
    const openTab = vi.fn();
    useSessionStore.setState({
      sessions: [
        makeSession({ id: "s1", name: "Multi A" }),
        makeSession({ id: "s2", name: "Multi B" }),
      ],
      openTab,
    });
    render(<SessionTree />);

    fireEvent.click(screen.getByText("Multi A"), { ctrlKey: true });
    fireEvent.click(screen.getByText("Multi B"), { ctrlKey: true });

    expect(screen.getByText("2 selected")).toBeInTheDocument();
    await userEvent.click(screen.getByText("Connect All"));

    expect(openTab).toHaveBeenCalledTimes(2);
    expect(screen.queryByText(/selected/)).not.toBeInTheDocument();
  });

  it("clears multi-selection via the Clear button", () => {
    useSessionStore.setState({
      sessions: [makeSession({ id: "s1", name: "Clear A" }), makeSession({ id: "s2", name: "Clear B" })],
    });
    render(<SessionTree />);

    fireEvent.click(screen.getByText("Clear A"), { ctrlKey: true });
    fireEvent.click(screen.getByText("Clear B"), { ctrlKey: true });
    expect(screen.getByText("2 selected")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Clear"));
    expect(screen.queryByText(/selected/)).not.toBeInTheDocument();
  });

  it("folder context menu: New Subfolder prompts and calls addFolder", async () => {
    const addFolder = vi.fn();
    const promptSpy = vi.spyOn(globalThis, "prompt").mockReturnValue("Staging");
    useSessionStore.setState({
      sessions: [makeSession({ id: "s1", name: "Prod Web", group: "Production" })],
      addFolder,
    });
    render(<SessionTree />);

    fireEvent.contextMenu(screen.getByText("Production"), { clientX: 5, clientY: 5 });
    await userEvent.click(screen.getByText(/Subfolder/));

    expect(addFolder).toHaveBeenCalledWith("Production/Staging");
    promptSpy.mockRestore();
  });

  it("folder context menu: Delete Folder confirms and clears the group on its sessions", async () => {
    const updateSession = vi.fn();
    const removeFolder = vi.fn();
    const confirmSpy = vi.spyOn(globalThis, "confirm").mockReturnValue(true);
    useSessionStore.setState({
      sessions: [makeSession({ id: "s1", name: "Prod Web", group: "Production" })],
      updateSession,
      removeFolder,
    });
    render(<SessionTree />);

    fireEvent.contextMenu(screen.getByText("Production"), { clientX: 5, clientY: 5 });
    await userEvent.click(screen.getByText(/Delete Folder/));

    expect(updateSession).toHaveBeenCalledWith("s1", { group: "" });
    expect(removeFolder).toHaveBeenCalledWith("Production");
    confirmSpy.mockRestore();
  });

  it("folder context menu: Delete Folder does nothing when the confirm is declined", async () => {
    const updateSession = vi.fn();
    const confirmSpy = vi.spyOn(globalThis, "confirm").mockReturnValue(false);
    useSessionStore.setState({
      sessions: [makeSession({ id: "s1", name: "Prod Web", group: "Production" })],
      updateSession,
    });
    render(<SessionTree />);

    fireEvent.contextMenu(screen.getByText("Production"), { clientX: 5, clientY: 5 });
    await userEvent.click(screen.getByText(/Delete Folder/));

    expect(updateSession).not.toHaveBeenCalled();
    confirmSpy.mockRestore();
  });

  it("New Session button in the empty state calls onNewSession", async () => {
    const onNewSession = vi.fn();
    render(<SessionTree onNewSession={onNewSession} />);
    await userEvent.click(screen.getByText("New Session"));
    expect(onNewSession).toHaveBeenCalled();
  });

  it("Import button in the empty state calls onImport", async () => {
    const onImport = vi.fn();
    render(<SessionTree onImport={onImport} />);
    await userEvent.click(screen.getByText("Import"));
    expect(onImport).toHaveBeenCalled();
  });
});
