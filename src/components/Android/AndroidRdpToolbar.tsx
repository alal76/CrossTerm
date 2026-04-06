import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { MousePointer, Hand, Crosshair } from 'lucide-react';
import clsx from 'clsx';

type MouseMode = 'touchpad' | 'touch' | 'direct';

interface AndroidRdpToolbarProps {
  readonly onModeChange?: (mode: MouseMode) => void;
  readonly className?: string;
}

export default function AndroidRdpToolbar({ onModeChange, className }: Readonly<AndroidRdpToolbarProps>) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<MouseMode>('touchpad');

  const modes: { key: MouseMode; icon: React.ReactNode; label: string }[] = [
    { key: 'touchpad', icon: <MousePointer size={16} />, label: t('android.touchpad') },
    { key: 'touch', icon: <Hand size={16} />, label: t('android.touch') },
    { key: 'direct', icon: <Crosshair size={16} />, label: t('android.direct') },
  ];

  const handleModeChange = (newMode: MouseMode) => {
    setMode(newMode);
    onModeChange?.(newMode);
  };

  return (
    <div
      className={clsx(
        'fixed bottom-20 right-4 flex flex-col gap-1 p-2 rounded-xl',
        'bg-surface-overlay shadow-lg border border-border-default',
        className
      )}
    >
      <span className="text-xs text-text-secondary px-2 pb-1">
        {t('android.mouseMode')}
      </span>
      {modes.map((m) => (
        <button
          key={m.key}
          onClick={() => handleModeChange(m.key)}
          className={clsx(
            'flex items-center gap-2 px-3 py-2 rounded-lg text-sm',
            mode === m.key
              ? 'bg-accent-primary text-text-inverse'
              : 'text-text-primary hover:bg-interactive-hover'
          )}
        >
          {m.icon}
          <span>{m.label}</span>
        </button>
      ))}
    </div>
  );
}
