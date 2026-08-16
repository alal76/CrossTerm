import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import PluginRegistry from "@/components/Plugin/PluginRegistry";
import { ToastProvider } from "@/components/Shared/Toast";
import type { PluginRegistryEntry } from "@/types";

function entry(overrides: Partial<PluginRegistryEntry> = {}): PluginRegistryEntry {
  return {
    id: "e1",
    name: "Awesome Plugin",
    version: "2.0.0",
    author: "acme",
    description: "Makes things awesome",
    downloads: 100,
    category: "terminal",
    installed: false,
    update_available: false,
    ...overrides,
  };
}

function mockFetch(entries: PluginRegistryEntry[], ok = true) {
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok,
    json: () => Promise.resolve(entries),
  }) as unknown as typeof fetch;
}

describe("PluginRegistry", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("fetches and lists registry entries on mount", async () => {
    mockFetch([entry()]);
    render(<PluginRegistry />);

    await waitFor(() => expect(globalThis.fetch).toHaveBeenCalledWith("https://registry.crossterm.dev/plugins.json"));
    expect(await screen.findByText("Awesome Plugin")).toBeInTheDocument();
  });

  it("shows the empty state when the fetch fails", async () => {
    globalThis.fetch = vi.fn().mockRejectedValue(new Error("network error")) as unknown as typeof fetch;
    render(<PluginRegistry />);
    expect(await screen.findByText(/No plugins installed/)).toBeInTheDocument();
  });

  it("filters entries by search query", async () => {
    mockFetch([entry({ id: "e1", name: "Docker Tools" }), entry({ id: "e2", name: "AWS Panel" })]);
    render(<PluginRegistry />);
    await screen.findByText("Docker Tools");

    fireEvent.change(screen.getByPlaceholderText("Search sessions..."), { target: { value: "docker" } });
    expect(screen.getByText("Docker Tools")).toBeInTheDocument();
    expect(screen.queryByText("AWS Panel")).not.toBeInTheDocument();
  });

  it("filters entries by category", async () => {
    mockFetch([
      entry({ id: "e1", name: "Net Tool", category: "network" }),
      entry({ id: "e2", name: "UI Tool", category: "ui" }),
    ]);
    render(<PluginRegistry />);
    await screen.findByText("Net Tool");

    fireEvent.click(screen.getByRole("button", { name: "Network" }));
    expect(screen.getByText("Net Tool")).toBeInTheDocument();
    expect(screen.queryByText("UI Tool")).not.toBeInTheDocument();
  });

  it("shows Install for uninstalled, Installed badge for installed, Update for outdated", async () => {
    mockFetch([
      entry({ id: "e1", name: "New Plugin", installed: false }),
      entry({ id: "e2", name: "Current Plugin", installed: true, update_available: false }),
      entry({ id: "e3", name: "Stale Plugin", installed: true, update_available: true }),
    ]);
    render(<PluginRegistry />);
    await screen.findByText("New Plugin");

    expect(screen.getByText("Install Plugin")).toBeInTheDocument();
    expect(screen.getByText("Installed")).toBeInTheDocument();
    expect(screen.getByText("Update")).toBeInTheDocument();
  });

  // Regression coverage: the Install/Update buttons had no onClick at all —
  // clicking them was a complete no-op. There's no download_url on
  // PluginRegistryEntry and no backend command to install from a remote
  // registry, so the fix is an honest explanation rather than a fake
  // install, but the click must do *something* visible.
  it("Install and Update buttons explain remote install isn't available yet", async () => {
    mockFetch([
      entry({ id: "e1", name: "New Plugin", installed: false }),
      entry({ id: "e3", name: "Stale Plugin", installed: true, update_available: true }),
    ]);
    render(<ToastProvider><PluginRegistry /></ToastProvider>);
    await screen.findByText("New Plugin");

    fireEvent.click(screen.getByText("Install Plugin"));
    expect(await screen.findByText(/isn.t available yet/)).toBeInTheDocument();

    fireEvent.click(screen.getByText("Update"));
    expect((await screen.findAllByText(/isn.t available yet/)).length).toBeGreaterThan(0);
  });

  it("refresh button re-fetches the registry", async () => {
    mockFetch([entry()]);
    render(<PluginRegistry />);
    await screen.findByText("Awesome Plugin");
    vi.mocked(globalThis.fetch).mockClear();

    fireEvent.click(screen.getByRole("button", { name: "" }));
    await waitFor(() => expect(globalThis.fetch).toHaveBeenCalled());
  });
});
