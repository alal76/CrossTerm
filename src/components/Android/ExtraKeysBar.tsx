import { useState, useCallback } from 'react';
import clsx from 'clsx';

interface ExtraKey {
  label: string;
  value: string;
  secondary?: string;
}

const DEFAULT_KEYS: ExtraKey[] = [
  { label: 'Esc', value: '\x1b' },
  { label: 'Tab', value: '\t' },
  { label: 'Ctrl', value: 'ctrl' },
  { label: 'Alt', value: 'alt' },
  { label: '←', value: '\x1b[D' },
  { label: '→', value: '\x1b[C' },
  { label: '↑', value: '\x1b[A' },
  { label: '↓', value: '\x1b[B' },
  { label: '|', value: '|', secondary: '\\' },
  { label: '~', value: '~', secondary: '`' },
  { label: '/', value: '/', secondary: '?' },
];

interface ExtraKeysBarProps {
  keys?: ExtraKey[];
  onKeyPress?: (value: string) => void;
  className?: string;
}

export default function ExtraKeysBar({
  keys = DEFAULT_KEYS,
  onKeyPress,
  className,
}: Readonly<ExtraKeysBarProps>) {
  const [pressTimer, setPressTimer] = useState<ReturnType<typeof setTimeout> | null>(null);

  const handlePress = useCallback(
    (key: ExtraKey) => {
      onKeyPress?.(key.value);
    },
    [onKeyPress]
  );

  const handleLongPressStart = useCallback(
    (key: ExtraKey) => {
      const timer = setTimeout(() => {
        if (key.secondary) {
          onKeyPress?.(key.secondary);
        }
      }, 500);
      setPressTimer(timer);
    },
    [onKeyPress]
  );

  const handleLongPressEnd = useCallback(() => {
    if (pressTimer) {
      clearTimeout(pressTimer);
      setPressTimer(null);
    }
  }, [pressTimer]);

  return (
    <div
      className={clsx(
        'flex overflow-x-auto gap-1 px-2 py-1 bg-surface-elevated border-t border-border-default',
        className
      )}
    >
      {keys.map((key) => (
        <button
          key={key.label}
          onClick={() => handlePress(key)}
          onTouchStart={() => handleLongPressStart(key)}
          onTouchEnd={handleLongPressEnd}
          onTouchCancel={handleLongPressEnd}
          className={clsx(
            'flex-shrink-0 min-w-[40px] h-9 px-3 rounded',
            'bg-surface-secondary text-text-primary text-sm font-mono',
            'active:bg-interactive-active hover:bg-interactive-hover',
            'border border-border-subtle'
          )}
        >
          {key.label}
        </button>
      ))}
    </div>
  );
}
