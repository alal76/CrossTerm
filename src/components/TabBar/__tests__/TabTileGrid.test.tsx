import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import "@/i18n";
import TabTileGrid from "@/components/TabBar/TabTileGrid";

function makeDataTransfer(tabId: string) {
  return {
    getData: () => tabId,
    setData: vi.fn(),
    dropEffect: "move",
  } as unknown as DataTransfer;
}

describe("TabTileGrid", () => {
  it("renders 4 empty grid cells by default", () => {
    render(<TabTileGrid />);
    expect(screen.getAllByRole("gridcell")).toHaveLength(4);
  });

  it("renders a tiled tab's id in its cell", () => {
    render(<TabTileGrid tabs={[{ tabId: "tab-1", position: { row: 0, col: 1 } }]} />);
    expect(screen.getByText("tab-1")).toBeInTheDocument();
  });

  it("calls onTabDrop with the dropped tab id and cell position", () => {
    const onTabDrop = vi.fn();
    render(<TabTileGrid onTabDrop={onTabDrop} />);

    const cells = screen.getAllByRole("gridcell");
    fireEvent.dragOver(cells[2], { dataTransfer: makeDataTransfer("tab-x") });
    fireEvent.drop(cells[2], { dataTransfer: makeDataTransfer("tab-x") });

    expect(onTabDrop).toHaveBeenCalledWith("tab-x", { row: 1, col: 0 });
  });

  it("does not call onTabDrop when the drag payload has no tab id", () => {
    const onTabDrop = vi.fn();
    render(<TabTileGrid onTabDrop={onTabDrop} />);

    fireEvent.drop(screen.getAllByRole("gridcell")[0], { dataTransfer: makeDataTransfer("") });
    expect(onTabDrop).not.toHaveBeenCalled();
  });

  it("clears the drag-over highlight on drag leave", () => {
    render(<TabTileGrid />);
    const cell = screen.getAllByRole("gridcell")[0];
    fireEvent.dragOver(cell, { dataTransfer: makeDataTransfer("tab-x") });
    fireEvent.dragLeave(cell);
    expect(cell.className).not.toContain("border-accent-primary");
  });
});
