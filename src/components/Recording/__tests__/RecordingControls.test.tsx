import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import "@/i18n";
import RecordingControls from "@/components/Recording/RecordingControls";
import { invoke } from "@tauri-apps/api/core";

const mockInvoke = vi.mocked(invoke);

describe("RecordingControls", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("starts recording and shows the elapsed timer", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    mockInvoke.mockResolvedValue("rec-1");
    render(<RecordingControls sessionId="s1" width={80} height={24} />);

    await act(async () => {
      fireEvent.click(screen.getByText("Start Recording"));
    });
    expect(mockInvoke).toHaveBeenCalledWith("recording_start", {
      sessionId: "s1", title: null, width: 80, height: 24,
    });
    expect(screen.getByText("Recording")).toBeInTheDocument();
    expect(screen.getByText("00:00")).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3000);
    });
    expect(screen.getByText("00:03")).toBeInTheDocument();
  });

  it("stops recording and resets state", async () => {
    mockInvoke.mockResolvedValue("rec-1");
    render(<RecordingControls sessionId="s1" width={80} height={24} />);
    fireEvent.click(screen.getByText("Start Recording"));
    await screen.findByText("Recording");

    mockInvoke.mockResolvedValueOnce(undefined);
    fireEvent.click(screen.getByText("Stop Recording"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("recording_stop", { recordingId: "rec-1" });
    });
    expect(await screen.findByText("Start Recording")).toBeInTheDocument();
  });
});
