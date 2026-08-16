import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Film, Trash2 } from 'lucide-react';
import clsx from 'clsx';
import RecordingPlayer from './RecordingPlayer';
import type { RecordingInfo } from '@/types';

export default function RecordingsPanel() {
  const { t } = useTranslation();
  const [recordings, setRecordings] = useState<RecordingInfo[]>([]);
  const [selected, setSelected] = useState<RecordingInfo | null>(null);

  const loadRecordings = useCallback(async () => {
    try {
      const list = await invoke<RecordingInfo[]>('recording_list');
      setRecordings(list);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadRecordings();
  }, [loadRecordings]);

  const handleDelete = async (id: string) => {
    try {
      await invoke('recording_delete', { recordingId: id });
      if (selected?.id === id) setSelected(null);
      await loadRecordings();
    } catch {
      // ignore
    }
  };

  return (
    <div className="flex h-full bg-surface-primary">
      <div className="w-56 border-r border-border-default flex flex-col">
        <div className="flex items-center gap-1 p-3 border-b border-border-default">
          <h3 className="text-sm font-medium text-text-primary flex items-center gap-1">
            <Film size={14} />
            {t('recording.library')}
          </h3>
        </div>
        <div className="flex-1 overflow-auto">
          {recordings.length === 0 ? (
            <p className="p-3 text-xs text-text-secondary">{t('recording.noRecordings')}</p>
          ) : (
            recordings.map((r) => (
              <div
                key={r.id}
                className={clsx(
                  'w-full flex items-center justify-between px-3 py-2 text-sm border-b border-border-subtle transition-colors',
                  selected?.id === r.id
                    ? 'bg-interactive-default/10 text-text-primary'
                    : 'text-text-secondary hover:bg-surface-elevated'
                )}
              >
                <button
                  onClick={() => setSelected(r)}
                  className="flex-1 text-left truncate"
                >
                  {r.title ?? r.id}
                  <span className="block text-xs text-text-disabled">
                    {r.duration_secs.toFixed(0)}s
                  </span>
                </button>
                <button
                  onClick={() => handleDelete(r.id)}
                  className="p-1 text-red-400 hover:text-red-300 transition-colors"
                  title={t('actions.delete')}
                >
                  <Trash2 size={12} />
                </button>
              </div>
            ))
          )}
        </div>
      </div>

      <div className="flex-1 overflow-auto p-4">
        {selected ? (
          <RecordingPlayer recording={selected} />
        ) : (
          <p className="text-sm text-text-secondary text-center py-8">
            {t('recording.selectToPlay')}
          </p>
        )}
      </div>
    </div>
  );
}
