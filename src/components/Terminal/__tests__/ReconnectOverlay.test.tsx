import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import "@/i18n";
import ReconnectOverlay from "@/components/Terminal/ReconnectOverlay";

describe("ReconnectOverlay", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("shows the connection-lost message and the reason", () => {
    render(<ReconnectOverlay reason="ECONNRESET" onReconnect={vi.fn()} onClose={vi.fn()} />);
    expect(screen.getByText("ECONNRESET")).toBeInTheDocument();
  });

  it("Reconnect Now calls onReconnect", async () => {
    const onReconnect = vi.fn().mockResolvedValue(true);
    render(<ReconnectOverlay reason="lost" onReconnect={onReconnect} onClose={vi.fn()} />);
    await act(async () => {
      fireEvent.click(screen.getByText("Reconnect Now"));
    });
    expect(onReconnect).toHaveBeenCalledOnce();
  });

  it("Close calls onClose", () => {
    const onClose = vi.fn();
    render(<ReconnectOverlay reason="lost" onReconnect={vi.fn()} onClose={onClose} />);
    fireEvent.click(screen.getByText("Close"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("auto-reconnects when the countdown reaches zero", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const onReconnect = vi.fn().mockResolvedValue(true);
    render(<ReconnectOverlay reason="lost" onReconnect={onReconnect} onClose={vi.fn()} />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });

    expect(onReconnect).toHaveBeenCalled();
  });

  it("shows the exhausted state after MAX_ATTEMPTS failed reconnects", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const onReconnect = vi.fn().mockResolvedValue(false);
    render(<ReconnectOverlay reason="lost" onReconnect={onReconnect} onClose={vi.fn()} />);

    // Backoff grows 2s,4s,8s,16s,32s across 5 attempts — advance generously.
    for (let i = 0; i < 5; i++) {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(65_000);
      });
    }

    expect(await screen.findByText("Connection Failed")).toBeInTheDocument();
  });

  it("Retry from the exhausted state resets back to the countdown view", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const onReconnect = vi.fn().mockResolvedValue(false);
    render(<ReconnectOverlay reason="lost" onReconnect={onReconnect} onClose={vi.fn()} />);

    for (let i = 0; i < 5; i++) {
      await act(async () => {
        await vi.advanceTimersByTimeAsync(65_000);
      });
    }
    expect(await screen.findByText("Connection Failed")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Retry"));
    expect(screen.getByText("Connection Lost")).toBeInTheDocument();
  });
});
