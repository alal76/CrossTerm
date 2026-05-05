import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderHook } from "@testing-library/react";
import TimestampJumper, { useTimestampIndex } from "./TimestampJumper";

describe("TimestampJumper", () => {
  it("renders 'Jump to:' label", () => {
    render(<TimestampJumper onJump={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByText(/jump to:/i)).toBeTruthy();
  });

  it("clicking Jump calls onJump with parsed Date", async () => {
    const onJump = vi.fn();
    render(<TimestampJumper onJump={onJump} onClose={vi.fn()} />);

    const labeledInput = document.getElementById("timestamp-jumper-input") as HTMLInputElement;
    await userEvent.type(labeledInput, "2026-05-05T10:00");

    await userEvent.click(screen.getByRole("button", { name: /^jump$/i }));

    expect(onJump).toHaveBeenCalledOnce();
    const arg = onJump.mock.calls[0][0] as Date;
    expect(arg).toBeInstanceOf(Date);
    expect(isNaN(arg.getTime())).toBe(false);
  });

  it("useTimestampIndex extracts correct line index for a timestamped line", () => {
    const lines = [
      "no timestamp here",
      "2026-05-05T12:34:56 some log message",
      "another line without timestamp",
      "2026-05-05T09:00:00Z info startup",
    ];

    const { result } = renderHook(() => useTimestampIndex(lines));
    const map = result.current;

    const ts1 = new Date("2026-05-05T12:34:56").getTime();
    const ts2 = new Date("2026-05-05T09:00:00").getTime();

    expect(map.get(ts1)).toBe(1);
    expect(map.get(ts2)).toBe(3);
    expect(map.size).toBe(2);
  });
});
