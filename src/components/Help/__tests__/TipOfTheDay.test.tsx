import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import "@/i18n";
import TipOfTheDay from "@/components/Help/TipOfTheDay";

describe("TipOfTheDay", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows nothing before the delay elapses", () => {
    render(<TipOfTheDay />);
    expect(screen.queryByText("Tip of the Day")).not.toBeInTheDocument();
  });

  it("shows a tip after the delay", async () => {
    render(<TipOfTheDay />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
    });
    expect(screen.getByText("Tip of the Day")).toBeInTheDocument();
  });

  it("does not show when the user previously opted out", () => {
    localStorage.setItem("crossterm-tip-optout", "true");
    render(<TipOfTheDay />);
    expect(screen.queryByText("Tip of the Day")).not.toBeInTheDocument();
  });

  it("dismiss hides the banner", async () => {
    render(<TipOfTheDay />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
    });
    fireEvent.click(screen.getByRole("button", { name: "" }));
    expect(screen.queryByText("Tip of the Day")).not.toBeInTheDocument();
  });

  it("Next Tip advances to the next tip and persists it", async () => {
    render(<TipOfTheDay />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
    });
    const firstTipText = screen.getByLabelText("Tip of the Day").querySelector("p")?.textContent;

    fireEvent.click(screen.getByText("Next Tip"));
    const secondTipText = screen.getByLabelText("Tip of the Day").querySelector("p")?.textContent;
    expect(secondTipText).not.toBe(firstTipText);
    expect(localStorage.getItem("crossterm-tip-index")).toBeTruthy();
  });

  it("Don't show again opts out and hides the banner", async () => {
    render(<TipOfTheDay />);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
    });
    fireEvent.click(screen.getByText("Don't show again"));
    expect(screen.queryByText("Tip of the Day")).not.toBeInTheDocument();
    expect(localStorage.getItem("crossterm-tip-optout")).toBe("true");
  });
});
