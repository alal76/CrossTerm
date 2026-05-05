import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { BroadcastControl, BroadcastManager } from "./BroadcastControl";

describe("BroadcastControl", () => {
  it("renders pane name", () => {
    render(
      <BroadcastControl
        paneId="p1"
        paneName="Terminal 1"
        isBroadcastEnabled={false}
        onToggle={vi.fn()}
      />
    );
    expect(screen.getByText("Terminal 1")).toBeTruthy();
  });

  it("toggling calls onToggle with correct args", async () => {
    const onToggle = vi.fn();
    render(
      <BroadcastControl
        paneId="p1"
        paneName="Terminal 1"
        isBroadcastEnabled={false}
        onToggle={onToggle}
      />
    );
    const checkbox = document.getElementById("broadcast-toggle-p1") as HTMLInputElement;
    await userEvent.click(checkbox);
    expect(onToggle).toHaveBeenCalledOnce();
    expect(onToggle).toHaveBeenCalledWith("p1", true);
  });
});

describe("BroadcastManager", () => {
  it("Enable all enables all panes", async () => {
    const onBroadcastChange = vi.fn();
    const panes = [
      { id: "p1", name: "Pane 1" },
      { id: "p2", name: "Pane 2" },
      { id: "p3", name: "Pane 3" },
    ];
    render(<BroadcastManager panes={panes} onBroadcastChange={onBroadcastChange} />);

    await userEvent.click(screen.getByRole("button", { name: /enable all/i }));

    expect(onBroadcastChange).toHaveBeenCalledOnce();
    const called = onBroadcastChange.mock.calls[0][0] as string[];
    expect(called.sort()).toEqual(["p1", "p2", "p3"]);
  });
});
