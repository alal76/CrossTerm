import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { GitCompare, Plus, Minus, File } from 'lucide-react';
import clsx from 'clsx';
import type { DiffResult, DiffLine } from '@/types';

export default function DiffViewer() {
  const { t } = useTranslation();
  const [leftPath, setLeftPath] = useState('');
  const [rightPath, setRightPath] = useState('');
  const [diffResult, setDiffResult] = useState<DiffResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleCompare = async () => {
    if (!leftPath || !rightPath) return;
    try {
      const result = await invoke<DiffResult>('editor_diff', {
        leftPath,
        rightPath,
      });
      setDiffResult(result);
      setError(null);
    } catch (err) {
      setError(String(err));
      setDiffResult(null);
    }
  };

  const handleSelectFile = async (side: 'left' | 'right') => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({});
      if (selected) {
        if (side === 'left') setLeftPath(String(selected));
        else setRightPath(String(selected));
      }
    } catch {
      // ignore
    }
  };

  const lineClass = (lineType: DiffLine['line_type']): string => {
    switch (lineType) {
      case 'added':
        return 'bg-green-500/15 text-green-300';
      case 'removed':
        return 'bg-red-500/15 text-red-300';
      default:
        return 'text-text-secondary';
    }
  };

  const lineIcon = (lineType: DiffLine['line_type']) => {
    switch (lineType) {
      case 'added':
        return <Plus size={10} className="text-green-400" />;
      case 'removed':
        return <Minus size={10} className="text-red-400" />;
      default:
        return <span className="w-2.5" />;
    }
  };

  return (
    <div className="flex flex-col h-full bg-surface-primary">
      {/* Toolbar */}
      <div className="flex items-center gap-2 p-3 border-b border-border-default">
        <GitCompare size={16} className="text-text-secondary" />
        <div className="flex-1 flex gap-2">
          <div className="flex-1 flex gap-1">
            <input
              type="text"
              value={leftPath}
              onChange={(e) => setLeftPath(e.target.value)}
              placeholder="Left file path"
              className="flex-1 px-2 py-1 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
            />
            <button
              onClick={() => handleSelectFile('left')}
              className="px-2 py-1 text-xs rounded bg-surface-elevated border border-border-subtle text-text-secondary hover:text-text-primary transition-colors"
            >
              <File size={12} />
            </button>
          </div>
          <div className="flex-1 flex gap-1">
            <input
              type="text"
              value={rightPath}
              onChange={(e) => setRightPath(e.target.value)}
              placeholder="Right file path"
              className="flex-1 px-2 py-1 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
            />
            <button
              onClick={() => handleSelectFile('right')}
              className="px-2 py-1 text-xs rounded bg-surface-elevated border border-border-subtle text-text-secondary hover:text-text-primary transition-colors"
            >
              <File size={12} />
            </button>
          </div>
        </div>
        <button
          onClick={handleCompare}
          className="px-3 py-1 text-sm rounded bg-interactive-default text-text-inverse hover:bg-interactive-hover transition-colors"
        >
          {t('editor.diff')}
        </button>
      </div>

      {error && (
        <div className="px-3 py-2 text-xs text-red-400 bg-red-500/10">
          {error}
        </div>
      )}

      {/* Diff Content */}
      {diffResult ? (
        <div className="flex-1 overflow-auto">
          {/* Stats */}
          <div className="flex items-center gap-4 px-3 py-2 bg-surface-secondary border-b border-border-default text-xs">
            <span className="text-green-400">
              +{diffResult.stats.additions} {t('editor.additions')}
            </span>
            <span className="text-red-400">
              -{diffResult.stats.deletions} {t('editor.deletions')}
            </span>
          </div>

          {/* Hunks */}
          {diffResult.hunks.map((hunk) => (
            <div key={`${hunk.left_start}-${hunk.right_start}`} className="border-b border-border-subtle">
              {/* Hunk Header */}
              <div className="px-3 py-1 bg-blue-500/10 text-xs text-blue-300 font-mono">
                @@ -{hunk.left_start},{hunk.left_count} +{hunk.right_start},{hunk.right_count} @@
              </div>
              {/* Lines */}
              <div className="font-mono text-xs">
                {hunk.lines.map((line: DiffLine, li: number) => (
                  <div
                    key={`${line.left_line ?? 'x'}-${line.right_line ?? 'x'}-${li}`}
                    className={clsx(
                      'flex items-center px-3 py-0.5',
                      lineClass(line.line_type)
                    )}
                  >
                    <span className="w-10 text-right pr-2 text-text-disabled select-none">
                      {line.left_line ?? ''}
                    </span>
                    <span className="w-10 text-right pr-2 text-text-disabled select-none">
                      {line.right_line ?? ''}
                    </span>
                    <span className="w-4 flex-shrink-0 flex items-center justify-center">
                      {lineIcon(line.line_type)}
                    </span>
                    <span className="flex-1 whitespace-pre">{line.content}</span>
                  </div>
                ))}
              </div>
            </div>
          ))}

          {diffResult.hunks.length === 0 && (
            <div className="flex items-center justify-center p-8 text-text-secondary text-sm">
              Files are identical
            </div>
          )}
        </div>
      ) : (
        <div className="flex-1 flex flex-col items-center justify-center text-text-secondary gap-2">
          <GitCompare size={48} className="opacity-30" />
          <p className="text-sm">Select two files to compare</p>
        </div>
      )}
    </div>
  );
}
