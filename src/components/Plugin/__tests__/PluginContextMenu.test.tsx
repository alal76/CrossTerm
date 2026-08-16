import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import PluginContextMenu from "@/components/Plugin/PluginContextMenu";

describe("PluginContextMenu", () => {
  it("renders items sorted by priority", () => {
    render(
      <PluginContextMenu
        items={[
          { id: "b", pluginId: "p1", label: "Second", priority: 2, onClick: vi.fn() },
          { id: "a", pluginId: "p1", label: "First", priority: 1, onClick: vi.fn() },
        ]}
        position={{ x: 10, y: 20 }}
        onClose={vi.fn()}
      />,
    );
    const items = screen.getAllByRole("menuitem");
    expect(items.map((el) => el.textContent)).toEqual(["First", "Second"]);
  });

  it("inserts a separator between different groups", () => {
    render(
      <PluginContextMenu
        items={[
          { id: "a", pluginId: "p1", label: "First", priority: 1, group: "g1", onClick: vi.fn() },
          { id: "b", pluginId: "p1", label: "Second", priority: 2, group: "g2", onClick: vi.fn() },
        ]}
        position={{ x: 0, y: 0 }}
        onClose={vi.fn()}
      />,
    );
    expect(document.querySelector("hr")).toBeInTheDocument();
  });

  it("clicking an item calls its onClick and onClose", () => {
    const onClick = vi.fn();
    const onClose = vi.fn();
    render(
      <PluginContextMenu
        items={[{ id: "a", pluginId: "p1", label: "Run", priority: 1, onClick }]}
        position={{ x: 0, y: 0 }}
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByText("Run"));
    expect(onClick).toHaveBeenCalledOnce();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("Escape key calls onClose", () => {
    const onClose = vi.fn();
    render(<PluginContextMenu items={[]} position={{ x: 0, y: 0 }} onClose={onClose} />);
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("clicking outside the menu calls onClose", () => {
    const onClose = vi.fn();
    render(<PluginContextMenu items={[]} position={{ x: 0, y: 0 }} onClose={onClose} />);
    fireEvent.mouseDown(document.body);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("clicking inside the menu does not call onClose", () => {
    const onClose = vi.fn();
    render(
      <PluginContextMenu
        items={[{ id: "a", pluginId: "p1", label: "Run", priority: 1, onClick: vi.fn() }]}
        position={{ x: 0, y: 0 }}
        onClose={onClose}
      />,
    );
    fireEvent.mouseDown(screen.getByRole("menu"));
    expect(onClose).not.toHaveBeenCalled();
  });
});
