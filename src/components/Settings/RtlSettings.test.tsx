import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import RtlSettings from "./RtlSettings";

describe("RtlSettings", () => {
  it("renders direction dropdown with correct selected value", () => {
    render(<RtlSettings enabled={true} direction="rtl" onChange={vi.fn()} />);
    const select = screen.getByRole("combobox") as HTMLSelectElement;
    expect(select.value).toBe("rtl");
  });

  it("onChange called when toggle is clicked", async () => {
    const onChange = vi.fn();
    render(<RtlSettings enabled={false} direction="auto" onChange={onChange} />);
    const checkbox = screen.getByRole("checkbox");
    await userEvent.click(checkbox);
    expect(onChange).toHaveBeenCalledOnce();
    expect(onChange).toHaveBeenCalledWith(true, "auto");
  });
});
