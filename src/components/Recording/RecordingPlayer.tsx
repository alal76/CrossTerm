import { useState, useEffect, useCallback, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { clsx } from 'clsx';
import { Play, Pause, Download } from 'lucide-react';
import type { RecordingInfo, PlaybackState } from '@/types';

interface RecordingPlayerProps {
  readonly recording: RecordingInfo;
}

const SPEED_OPTIONS = [0.5, 1, 2, 4];

export default function RecordingPlayer({
  recording,
}: RecordingPlayerProps) {
  const { t } = useTranslation();
  const [playback, setPlayback] = useState<PlaybackState | null>(null);
  const [output, setOutput] = useState('');
  const outputRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    const unlistenFrame = listen<{
      recording_id: string;
      data: string;
      position: number;
    }>('recording:playback_frame', (event) => {
      if (event.payload.recording_id === recording.id) {
        setOutput((prev) => prev + event.payload.data);
        setPlayback((prev: PlaybackState | null) =>
          prev
            ? { ...prev, position: event.payload.position }
            : null
        );
      }
    });

    const unlistenComplete = listen<{ recording_id: string }>(
      'recording:playback_complete',
      (event) => {
        if (event.payload.recording_id === recording.id) {
          setPlayback((prev: PlaybackState | null) => (prev ? { ...prev, playing: false } : null));
        }
      }
    );

    return () => {
      unlistenFrame.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
    };
  }, [recording.id]);

  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [output]);

  const handlePlay = useCallback(async () => {
    setOutput('');
    try {
      const state = await invoke<PlaybackState>('recording_playback_start', {
        recordingId: recording.id,
        speed: playback?.speed ?? 1,
      });
      setPlayback(state);
    } catch {
      // handle error
    }
  }, [recording.id, playback?.speed]);

  const handleSeek = useCallback(
    async (position: number) => {
      try {
        const state = await invoke<PlaybackState>('recording_playback_seek', {
          recordingId: recording.id,
          position,
        });
        setPlayback(state);
      } catch {
        // handle error
      }
    },
    [recording.id]
  );

  const handleSpeedChange = useCallback(
    async (speed: number) => {
      try {
        const state = await invoke<PlaybackState>(
          'recording_playback_set_speed',
          { recordingId: recording.id, speed }
        );
        setPlayback(state);
      } catch {
        // handle error
      }
    },
    [recording.id]
  );

  const handleExport = useCallback(
    async (format: 'gif' | 'mp4') => {
      try {
        await invoke('recording_export', {
          recordingId: recording.id,
          format,
        });
      } catch {
        // handle export error — expected stub
      }
    },
    [recording.id]
  );

  const currentPosition = playback?.position ?? 0;
  const isPlaying = playback?.playing ?? false;
  const currentSpeed = playback?.speed ?? 1;

  return (
    <div className="flex flex-col gap-3 rounded-md border border-border-default bg-surface-primary p-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-text-primary">
          {recording.title ?? recording.id}
        </h3>
        <span className="text-xs text-text-secondary">
          {recording.width}×{recording.height}
        </span>
      </div>

      <pre
        ref={outputRef}
        className="h-64 overflow-auto rounded bg-terminal-background p-3 font-mono text-xs text-terminal-foreground"
      >
        {output || '\n'}
      </pre>

      <div className="flex items-center gap-3">
        <button
          onClick={handlePlay}
          disabled={isPlaying}
          className="rounded p-1.5 text-text-primary hover:bg-surface-elevated disabled:text-text-disabled"
        >
          {isPlaying ? <Pause size={16} /> : <Play size={16} />}
        </button>

        <input
          type="range"
          min={0}
          max={recording.duration_secs}
          step={0.1}
          value={currentPosition}
          onChange={(e) => handleSeek(Number.parseFloat(e.target.value))}
          className="flex-1"
        />

        <span className="min-w-[80px] text-right text-xs text-text-secondary">
          {currentPosition.toFixed(1)}s / {recording.duration_secs.toFixed(1)}s
        </span>
      </div>

      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1">
          <span className="text-xs text-text-secondary">
            {t('recording.speed')}:
          </span>
          {SPEED_OPTIONS.map((s) => (
            <button
              key={s}
              onClick={() => handleSpeedChange(s)}
              className={clsx(
                'rounded px-2 py-0.5 text-xs',
                currentSpeed === s
                  ? 'bg-accent-primary text-text-inverse'
                  : 'bg-surface-elevated text-text-secondary hover:text-text-primary'
              )}
            >
              {s}x
            </button>
          ))}
        </div>

        <div className="flex items-center gap-1">
          <button
            onClick={() => handleExport('gif')}
            className="flex items-center gap-1 rounded px-2 py-1 text-xs text-text-secondary hover:bg-surface-elevated hover:text-text-primary"
          >
            <Download size={12} />
            {t('recording.exportGif')}
          </button>
          <button
            onClick={() => handleExport('mp4')}
            className="flex items-center gap-1 rounded px-2 py-1 text-xs text-text-secondary hover:bg-surface-elevated hover:text-text-primary"
          >
            <Download size={12} />
            {t('recording.exportMp4')}
          </button>
        </div>
      </div>
    </div>
  );
}
