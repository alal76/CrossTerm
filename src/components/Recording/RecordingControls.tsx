import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Circle, Square } from 'lucide-react';

interface RecordingControlsProps {
  readonly sessionId: string;
  readonly width: number;
  readonly height: number;
}

export default function RecordingControls({
  sessionId,
  width,
  height,
}: RecordingControlsProps) {
  const { t } = useTranslation();
  const [recording, setRecording] = useState(false);
  const [recordingId, setRecordingId] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, []);

  const handleStart = useCallback(async () => {
    try {
      const id = await invoke<string>('recording_start', {
        sessionId,
        title: null,
        width,
        height,
      });
      setRecordingId(id);
      setRecording(true);
      setElapsed(0);
      timerRef.current = setInterval(() => {
        setElapsed((prev) => prev + 1);
      }, 1000);
    } catch {
      // handle error
    }
  }, [sessionId, width, height]);

  const handleStop = useCallback(async () => {
    if (!recordingId) return;
    try {
      await invoke('recording_stop', { recordingId });
    } catch {
      // handle error
    } finally {
      setRecording(false);
      setRecordingId(null);
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    }
  }, [recordingId]);

  const formatTime = (secs: number) => {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
  };

  return (
    <div className="flex items-center gap-2">
      {recording ? (
        <>
          <div className="flex items-center gap-1.5">
            <Circle
              size={8}
              className="animate-pulse fill-current text-red-500"
            />
            <span className="text-xs font-medium text-red-500">
              {t('recording.recording')}
            </span>
            <span className="text-xs text-text-secondary">
              {formatTime(elapsed)}
            </span>
          </div>
          <button
            onClick={handleStop}
            className="flex items-center gap-1 rounded px-2 py-1 text-xs text-text-primary hover:bg-surface-elevated"
            title={t('recording.stop')}
          >
            <Square size={11} className="fill-current" />
            {t('recording.stop')}
          </button>
        </>
      ) : (
        <button
          onClick={handleStart}
          className="flex items-center gap-1 rounded px-2 py-1 text-xs text-text-secondary hover:bg-surface-elevated hover:text-text-primary"
          title={t('recording.start')}
        >
          <Circle size={11} className="text-red-500" />
          {t('recording.start')}
        </button>
      )}
    </div>
  );
}
