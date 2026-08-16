import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import ReconnectOverlay from "@/components/Shared/ReconnectOverlay";

describe("ReconnectOverlay (Shared)", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders nothing when status is ok", () => {
    const { container } = render(
      <ReconnectOverlay
        sessionId="s1"
        status="ok"
        latencyMs={null}
        attempt={0}
        onReconnect={vi.fn()}
        onDismiss={vi.fn()}
        onGiveUp={vi.fn()}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("shows a degraded banner with latency", () => {
    render(
      <ReconnectOverlay
        sessionId="s1"
        status="degraded"
        latencyMs={450}
        attempt={0}
        onReconnect={vi.fn()}
        onDismiss={vi.fn()}
        onGiveUp={vi.fn()}
      />,
    );
    expect(screen.getByText(/High latency \(450ms\)/)).toBeInTheDocument();
  });

  it("shows the dropped dialog with a countdown and attempt count", () => {
    render(
      <ReconnectOverlay
        sessionId="s1"
        status="dropped"
        latencyMs={null}
        attempt={1}
        onReconnect={vi.fn()}
        onDismiss={vi.fn()}
        onGiveUp={vi.fn()}
      />,
    );
    expect(screen.getByText("Connection lost")).toBeInTheDocument();
    expect(screen.getByText(/attempt 1\/5/)).toBeInTheDocument();
  });

  it("Reconnect now calls onReconnect", () => {
    const onReconnect = vi.fn();
    render(
      <ReconnectOverlay
        sessionId="s1"
        status="dropped"
        latencyMs={null}
        attempt={1}
        onReconnect={onReconnect}
        onDismiss={vi.fn()}
        onGiveUp={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByText("Reconnect now"));
    expect(onReconnect).toHaveBeenCalledOnce();
  });

  it("Dismiss calls onDismiss", () => {
    const onDismiss = vi.fn();
    render(
      <ReconnectOverlay
        sessionId="s1"
        status="dropped"
        latencyMs={null}
        attempt={1}
        onReconnect={vi.fn()}
        onDismiss={onDismiss}
        onGiveUp={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByText("Dismiss"));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("shows Give up once near the max attempts", () => {
    render(
      <ReconnectOverlay
        sessionId="s1"
        status="dropped"
        latencyMs={null}
        attempt={4}
        maxAttempts={5}
        onReconnect={vi.fn()}
        onDismiss={vi.fn()}
        onGiveUp={vi.fn()}
      />,
    );
    expect(screen.getByText("Give up")).toBeInTheDocument();
  });

  it("shows the gave-up state after exhausting attempts", () => {
    render(
      <ReconnectOverlay
        sessionId="s1"
        status="dropped"
        latencyMs={null}
        attempt={5}
        maxAttempts={5}
        onReconnect={vi.fn()}
        onDismiss={vi.fn()}
        onGiveUp={vi.fn()}
      />,
    );
    expect(screen.getByText(/Reconnect failed after/)).toBeInTheDocument();
    expect(screen.getByText("Try again")).toBeInTheDocument();
  });

  it("auto-reconnects when the countdown reaches zero", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const onReconnect = vi.fn();
    render(
      <ReconnectOverlay
        sessionId="s1"
        status="dropped"
        latencyMs={null}
        attempt={1}
        onReconnect={onReconnect}
        onDismiss={vi.fn()}
        onGiveUp={vi.fn()}
      />,
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    expect(onReconnect).toHaveBeenCalled();
  });

  it("shows the last-seen latency note when available", () => {
    render(
      <ReconnectOverlay
        sessionId="s1"
        status="dropped"
        latencyMs={300}
        attempt={1}
        onReconnect={vi.fn()}
        onDismiss={vi.fn()}
        onGiveUp={vi.fn()}
      />,
    );
    expect(screen.getByText(/Last seen at 300ms latency/)).toBeInTheDocument();
  });
});
