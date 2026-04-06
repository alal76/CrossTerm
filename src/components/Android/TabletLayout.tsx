import { useState, useEffect } from 'react';
import clsx from 'clsx';

interface TabletLayoutProps {
  readonly sidebar?: React.ReactNode;
  readonly main: React.ReactNode;
  readonly secondaryPanel?: React.ReactNode;
  readonly minWidthDp?: number;
}

export default function TabletLayout({
  sidebar,
  main,
  secondaryPanel,
  minWidthDp = 600,
}: Readonly<TabletLayoutProps>) {
  const [isTablet, setIsTablet] = useState(false);
  const [hasKeyboard, setHasKeyboard] = useState(false);

  useEffect(() => {
    const checkLayout = () => {
      setIsTablet(window.innerWidth >= minWidthDp);
    };
    checkLayout();
    window.addEventListener('resize', checkLayout);
    return () => window.removeEventListener('resize', checkLayout);
  }, [minWidthDp]);

  useEffect(() => {
    const handleKeyboard = (e: KeyboardEvent) => {
      if (e.type === 'keydown') setHasKeyboard(true);
    };
    globalThis.addEventListener('keydown', handleKeyboard);
    return () => globalThis.removeEventListener('keydown', handleKeyboard);
  }, []);

  if (!isTablet) {
    return <div className="h-full">{main}</div>;
  }

  return (
    <div
      className={clsx('flex h-full', hasKeyboard && 'keyboard-visible')}
    >
      {sidebar && (
        <aside className="w-64 flex-shrink-0 border-r border-border-default bg-surface-secondary overflow-y-auto">
          {sidebar}
        </aside>
      )}
      <main className="flex-1 overflow-auto">
        {main}
      </main>
      {secondaryPanel && (
        <aside className="w-80 flex-shrink-0 border-l border-border-default bg-surface-secondary overflow-y-auto">
          {secondaryPanel}
        </aside>
      )}
    </div>
  );
}
