import { useState, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Monitor, FolderOpen, Settings, LayoutList } from 'lucide-react';
import clsx from 'clsx';

type AndroidTab = 'sessions' | 'terminal' | 'files' | 'settings';

interface AndroidShellProps {
  readonly children?: React.ReactNode;
  readonly onDrawerToggle?: () => void;
}

export default function AndroidShell({ children, onDrawerToggle }: Readonly<AndroidShellProps>) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<AndroidTab>('terminal');
  const [isAndroid, setIsAndroid] = useState(false);

  useEffect(() => {
    async function detectPlatform() {
      try {
        const { platform } = await import('@tauri-apps/plugin-os');
        const os = platform();
        setIsAndroid(os === 'android');
      } catch {
        setIsAndroid(false);
      }
    }
    detectPlatform();
  }, []);

  if (!isAndroid) {
    return <>{children}</>;
  }

  const tabs: { key: AndroidTab; icon: React.ReactNode; label: string }[] = [
    { key: 'sessions', icon: <LayoutList size={20} />, label: t('android.sessions') },
    { key: 'terminal', icon: <Monitor size={20} />, label: t('android.terminal') },
    { key: 'files', icon: <FolderOpen size={20} />, label: t('android.files') },
    { key: 'settings', icon: <Settings size={20} />, label: t('android.settings') },
  ];

  return (
    <div className="flex flex-col h-full bg-surface-primary">
      {/* Top App Bar */}
      <header className="flex items-center h-14 px-4 bg-surface-elevated border-b border-border-default">
        <button
          onClick={onDrawerToggle}
          className="p-2 mr-2 rounded hover:bg-interactive-hover"
          aria-label="Toggle drawer"
        >
          <LayoutList size={20} />
        </button>
        <span className="text-text-primary font-semibold text-lg">
          {t('app.name')}
        </span>
      </header>

      {/* Content Area */}
      <main className="flex-1 overflow-auto">
        {children}
      </main>

      {/* Bottom Navigation */}
      <nav className="flex items-center justify-around h-16 bg-surface-elevated border-t border-border-default">
        {tabs.map((tab) => (
          <button
            key={tab.key}
            onClick={() => setActiveTab(tab.key)}
            className={clsx(
              'flex flex-col items-center justify-center gap-1 flex-1 h-full',
              activeTab === tab.key
                ? 'text-accent-primary'
                : 'text-text-secondary hover:text-text-primary'
            )}
          >
            {tab.icon}
            <span className="text-xs">{tab.label}</span>
          </button>
        ))}
      </nav>
    </div>
  );
}
