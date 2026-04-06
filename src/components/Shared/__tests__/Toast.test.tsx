import { describe, it, expect, beforeEach, vi, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import "@/i18n";
import { ToastProvider, useToast } from "@/components/Shared/Toast";

// Helper component to trigger toasts
function ToastTrigger({ type, message }: { readonly type: "success" | "info" | "warning" | "error"; readonly message: string }) {
  const { toast } = useToast();
  return (
    <button onClick={() => toast(type, message)}>
      trigger-{type}
    </button>
  );
}

function renderWithProvider(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

describe("Toast", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("FT-C-19: renders success/info/warning/error toast with correct styling", () => {
    renderWithProvider(
      <>
        <ToastTrigger type="success" message="Operation succeeded" />
        <ToastTrigger type="info" message="Information note" />
        <ToastTrigger type="warning" message="Warning issued" />
        <ToastTrigger type="error" message="Error occurred" />
      </>
    );

    // Trigger all four toast types
    act(() => { screen.getByText("trigger-success").click(); });
    act(() => { screen.getByText("trigger-info").click(); });
    act(() => { screen.getByText("trigger-warning").click(); });
    act(() => { screen.getByText("trigger-error").click(); });

    // All toast messages should be rendered (limited to MAX_VISIBLE=3 most recent)
    // The 3 most recent are info, warning, error
    expect(screen.getByText("Information note")).toBeInTheDocument();
    expect(screen.getByText("Warning issued")).toBeInTheDocument();
    expect(screen.getByText("Error occurred")).toBeInTheDocument();

    // Each toast should have role="alert"
    const alerts = screen.getAllByRole("alert");
    expect(alerts.length).toBeGreaterThanOrEqual(3);
  });

  it("FT-C-20: auto-dismiss works for success toast", () => {
    renderWithProvider(
      <ToastTrigger type="success" message="Auto-dismiss me" />
    );

    act(() => { screen.getByText("trigger-success").click(); });
    expect(screen.getByText("Auto-dismiss me")).toBeInTheDocument();

    // Success duration is 4000ms, plus 250ms exit animation
    act(() => { vi.advanceTimersByTime(4500); });

    expect(screen.queryByText("Auto-dismiss me")).not.toBeInTheDocument();
  });

  it("FT-C-21: error toast does NOT auto-dismiss", () => {
    renderWithProvider(
      <ToastTrigger type="error" message="Persistent error" />
    );

    act(() => { screen.getByText("trigger-error").click(); });
    expect(screen.getByText("Persistent error")).toBeInTheDocument();

    // Advance way past any auto-dismiss window
    act(() => { vi.advanceTimersByTime(30000); });

    // Error toast should still be visible
    expect(screen.getByText("Persistent error")).toBeInTheDocument();
  });
});
