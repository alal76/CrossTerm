import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import PluginManager from "@/components/Plugin/PluginManager";
import { invoke } from "@tauri-apps/api/core";
import type { PluginInfo } from "@/types";

const mockInvoke = vi.mocked(invoke);

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

function plugin(overrides: Partial<PluginInfo> = {}): PluginInfo {
  return {
    manifest: {
      id: "p1",
      name: "Cool Plugin",
      version: "1.0.0",
      author: "acme",
      description: "Does cool things",
      permissions: ["network", "clipboard"],
      entry_point: "index.wasm",
      api_version: "1",
    },
    enabled: false,
    loaded: false,
    ...overrides,
  };
}

describe("PluginManager", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads and renders installed plugins", async () => {
    mockInvoke.mockResolvedValue([plugin()]);
    render(<PluginManager />);

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith("plugin_list"));
    expect(await screen.findByText("Cool Plugin")).toBeInTheDocument();
    expect(screen.getByText("Network")).toBeInTheDocument();
    expect(screen.getByText("Clipboard")).toBeInTheDocument();
  });

  it("shows the empty state when no plugins are installed", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<PluginManager />);
    expect(await screen.findByText(/No plugins installed/)).toBeInTheDocument();
  });

  // Regression coverage: enabling a plugin here only registers its manifest
  // — plugin_load is a stub and nothing dispatches real events to it — but
  // the UI previously gave no indication of that, letting "Enabled" imply
  // the plugin actually runs.
  it("shows an honest notice that plugin execution isn't implemented yet", async () => {
    mockInvoke.mockResolvedValue([]);
    render(<PluginManager />);
    expect(await screen.findByText(/sandboxed execution.*isn.t implemented yet/)).toBeInTheDocument();
  });

  it("shows an error message when loading fails", async () => {
    mockInvoke.mockRejectedValue(new Error("registry unavailable"));
    render(<PluginManager />);
    expect(await screen.findByText(/registry unavailable/)).toBeInTheDocument();
  });

  it("enables a disabled plugin", async () => {
    mockInvoke.mockResolvedValueOnce([plugin({ enabled: false })]);
    render(<PluginManager />);
    await screen.findByText("Cool Plugin");

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "plugin_enable") return Promise.resolve(undefined);
      if (cmd === "plugin_load") return Promise.resolve(undefined);
      if (cmd === "plugin_list") return Promise.resolve([plugin({ enabled: true })]);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByTitle("Enable"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("plugin_enable", { pluginId: "p1" });
      expect(mockInvoke).toHaveBeenCalledWith("plugin_load", { pluginId: "p1" });
    });
    expect(await screen.findByTitle("Disable")).toBeInTheDocument();
  });

  it("disables an enabled plugin", async () => {
    mockInvoke.mockResolvedValueOnce([plugin({ enabled: true })]);
    render(<PluginManager />);
    await screen.findByText("Cool Plugin");

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "plugin_disable") return Promise.resolve(undefined);
      if (cmd === "plugin_list") return Promise.resolve([plugin({ enabled: false })]);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByTitle("Disable"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("plugin_disable", { pluginId: "p1" });
    });
  });

  it("uninstalls a plugin after confirmation", async () => {
    mockInvoke.mockResolvedValueOnce([plugin()]);
    render(<PluginManager />);
    await screen.findByText("Cool Plugin");

    const confirmSpy = vi.spyOn(globalThis, "confirm").mockReturnValue(true);
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "plugin_uninstall") return Promise.resolve(undefined);
      if (cmd === "plugin_list") return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
    fireEvent.click(screen.getByText("Uninstall"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("plugin_uninstall", { pluginId: "p1" });
    });
    confirmSpy.mockRestore();
  });

  it("does not uninstall when the confirmation is declined", async () => {
    mockInvoke.mockResolvedValueOnce([plugin()]);
    render(<PluginManager />);
    await screen.findByText("Cool Plugin");

    const confirmSpy = vi.spyOn(globalThis, "confirm").mockReturnValue(false);
    fireEvent.click(screen.getByText("Uninstall"));

    expect(mockInvoke).not.toHaveBeenCalledWith("plugin_uninstall", expect.anything());
    confirmSpy.mockRestore();
  });

  it("installs a plugin via the file picker", async () => {
    mockInvoke.mockResolvedValueOnce([]);
    render(<PluginManager />);
    await screen.findByText(/No plugins installed/);

    const { open } = await import("@tauri-apps/plugin-dialog");
    vi.mocked(open).mockResolvedValue("/plugins/cool.wasm");
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "plugin_install") return Promise.resolve(undefined);
      if (cmd === "plugin_list") return Promise.resolve([plugin()]);
      return Promise.resolve(undefined);
    });

    fireEvent.click(screen.getByText("Install Plugin"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("plugin_install", { path: "/plugins/cool.wasm" });
    });
    expect(await screen.findByText("Cool Plugin")).toBeInTheDocument();
  });

  it("shows a plugin-level error message when present", async () => {
    mockInvoke.mockResolvedValue([plugin({ error: "failed to load wasm module" })]);
    render(<PluginManager />);
    expect(await screen.findByText("failed to load wasm module")).toBeInTheDocument();
  });
});
