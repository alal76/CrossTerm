import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import "@/i18n";
import RecordingPlayer from "@/components/Recording/RecordingPlayer";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { RecordingInfo, PlaybackState } from "@/types";

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

const recording: RecordingInfo = {
  id: "rec-1",
  path: "/tmp/rec-1.cast",
  title: "My Session",
  duration_secs: 42,
  size_bytes: 1024,
  width: 80,
  height: 24,
  created_at: "2026-01-01T00:00:00Z",
};

function playbackState(overrides: Partial<PlaybackState> = {}): PlaybackState {
  return { recording_id: "rec-1", position: 0, speed: 1, playing: true, ...overrides };
}

describe("RecordingPlayer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
  });

  it("renders the recording title and dimensions", () => {
    render(<RecordingPlayer recording={recording} />);
    expect(screen.getByText("My Session")).toBeInTheDocument();
    expect(screen.getByText("80×24")).toBeInTheDocument();
  });

  it("starts playback and shows the pause icon", async () => {
    mockInvoke.mockResolvedValue(playbackState());
    render(<RecordingPlayer recording={recording} />);

    fireEvent.click(screen.getByRole("button", { name: "" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("recording_playback_start", { recordingId: "rec-1", speed: 1 });
    });
  });

  it("appends streamed frame data to the output", async () => {
    mockInvoke.mockResolvedValue(playbackState());
    render(<RecordingPlayer recording={recording} />);
    fireEvent.click(screen.getAllByRole("button")[0]);
    await waitFor(() => expect(mockInvoke).toHaveBeenCalled());

    handlers["recording:playback_frame"]({
      payload: { recording_id: "rec-1", data: "hello\n", position: 1.2 },
    });

    expect(await screen.findByText("hello")).toBeInTheDocument();
  });

  it("ignores frame events for a different recording id", async () => {
    render(<RecordingPlayer recording={recording} />);
    handlers["recording:playback_frame"]({
      payload: { recording_id: "other", data: "nope\n", position: 1 },
    });
    expect(screen.queryByText("nope")).not.toBeInTheDocument();
  });

  it("changes playback speed", async () => {
    mockInvoke.mockResolvedValue(playbackState({ speed: 2 }));
    render(<RecordingPlayer recording={recording} />);

    fireEvent.click(screen.getByText("2x"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("recording_playback_set_speed", { recordingId: "rec-1", speed: 2 });
    });
  });

  it("seeks via the range input", async () => {
    mockInvoke.mockResolvedValue(playbackState({ position: 5 }));
    render(<RecordingPlayer recording={recording} />);

    fireEvent.change(screen.getByRole("slider"), { target: { value: "5" } });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("recording_playback_seek", { recordingId: "rec-1", position: 5 });
    });
  });

  it("exports as gif and mp4", async () => {
    mockInvoke.mockResolvedValue(undefined);
    render(<RecordingPlayer recording={recording} />);

    fireEvent.click(screen.getByText("Export as GIF"));
    fireEvent.click(screen.getByText("Export as MP4"));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("recording_export", { recordingId: "rec-1", format: "gif" });
      expect(mockInvoke).toHaveBeenCalledWith("recording_export", { recordingId: "rec-1", format: "mp4" });
    });
  });
});
