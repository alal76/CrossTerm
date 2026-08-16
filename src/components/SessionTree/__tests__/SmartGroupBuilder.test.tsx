import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import SmartGroupBuilder, { buildFilterExpr } from "@/components/SessionTree/SmartGroupBuilder";

describe("buildFilterExpr", () => {
  it("returns null for an empty row list", () => {
    expect(buildFilterExpr([])).toBeNull();
  });

  it("returns null when the only row has no value", () => {
    expect(buildFilterExpr([{ key: "a", type: "name_contains", value: "" }])).toBeNull();
  });

  it("returns a single leaf expr for one populated row", () => {
    expect(buildFilterExpr([{ key: "a", type: "name_contains", value: "prod" }])).toEqual({
      type: "name_contains",
      value: "prod",
    });
  });

  it("combines multiple populated rows with AND", () => {
    const result = buildFilterExpr([
      { key: "a", type: "name_contains", value: "prod" },
      { key: "b", type: "tag", value: "aws" },
    ]);
    expect(result).toEqual({
      type: "and",
      children: [
        { type: "name_contains", value: "prod" },
        { type: "tag", value: "aws" },
      ],
    });
  });

  it("skips empty rows when combining", () => {
    const result = buildFilterExpr([
      { key: "a", type: "name_contains", value: "prod" },
      { key: "b", type: "tag", value: "" },
    ]);
    expect(result).toEqual({ type: "name_contains", value: "prod" });
  });

  it("parses last_connected_before as a number and rejects non-numeric input", () => {
    expect(buildFilterExpr([{ key: "a", type: "last_connected_before", value: "30" }])).toEqual({
      type: "last_connected_before",
      days: 30,
    });
    expect(buildFilterExpr([{ key: "a", type: "last_connected_before", value: "abc" }])).toBeNull();
  });
});

describe("SmartGroupBuilder", () => {
  it("adding and removing condition rows works, with at least one row always kept", () => {
    render(<SmartGroupBuilder onClose={vi.fn()} onCreate={vi.fn()} />);
    expect(screen.getAllByTitle("Remove condition")).toHaveLength(1);
    expect(screen.getAllByTitle("Remove condition")[0]).toBeDisabled();

    fireEvent.click(screen.getByText("Add condition"));
    expect(screen.getAllByTitle("Remove condition")).toHaveLength(2);

    fireEvent.click(screen.getAllByTitle("Remove condition")[1]);
    expect(screen.getAllByTitle("Remove condition")).toHaveLength(1);
  });

  it("calls onClose when Cancel is clicked", () => {
    const onClose = vi.fn();
    render(<SmartGroupBuilder onClose={onClose} onCreate={vi.fn()} />);
    fireEvent.click(screen.getByText("Cancel"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("calls onCreate with the name and built filter", () => {
    const onCreate = vi.fn();
    render(<SmartGroupBuilder onClose={vi.fn()} onCreate={onCreate} />);

    fireEvent.change(screen.getByPlaceholderText("e.g. Production SSH"), { target: { value: "My Group" } });
    fireEvent.change(screen.getByPlaceholderText("value…"), { target: { value: "web" } });
    fireEvent.click(screen.getByText("Create"));

    expect(onCreate).toHaveBeenCalledWith("My Group", { type: "name_contains", value: "web" });
  });
});
