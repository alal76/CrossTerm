import { useTranslation } from 'react-i18next';
import { Star, Clock, FolderTree, X } from 'lucide-react';
import clsx from 'clsx';
import type { Session } from '@/types';

interface SessionDrawerProps {
  open: boolean;
  onClose: () => void;
  favorites?: Session[];
  recent?: Session[];
  sessions?: Session[];
  onSessionSelect?: (session: Session) => void;
}

export default function SessionDrawer({
  open,
  onClose,
  favorites = [],
  recent = [],
  sessions = [],
  onSessionSelect,
}: Readonly<SessionDrawerProps>) {
  const { t } = useTranslation();

  return (
    <>
      {/* Backdrop */}
      {open && (
        <div
          role="button"
          tabIndex={0}
          className="fixed inset-0 bg-black/40 z-40"
          onClick={onClose}
          onKeyDown={(e) => { if (e.key === 'Escape') onClose(); }}
        />
      )}

      {/* Drawer */}
      <aside
        className={clsx(
          'fixed top-0 left-0 h-full w-72 z-50 bg-surface-primary',
          'border-r border-border-default shadow-xl',
          'transform transition-transform duration-200',
          open ? 'translate-x-0' : '-translate-x-full'
        )}
      >
        <div className="flex items-center justify-between p-4 border-b border-border-default">
          <span className="text-text-primary font-semibold text-lg">
            {t('sessions.title')}
          </span>
          <button
            onClick={onClose}
            className="p-1 rounded hover:bg-interactive-hover"
          >
            <X size={20} />
          </button>
        </div>

        <div className="overflow-y-auto h-full pb-20">
          {/* Favorites */}
          {favorites.length > 0 && (
            <section className="p-4">
              <h3 className="flex items-center gap-2 text-text-secondary text-xs font-medium uppercase mb-2">
                <Star size={14} />
                {t('sessions.favorites')}
              </h3>
              <div className="flex flex-wrap gap-2">
                {favorites.map((s) => (
                  <button
                    key={s.id}
                    onClick={() => onSessionSelect?.(s)}
                    className="px-3 py-1 rounded-full bg-surface-secondary text-text-primary text-sm border border-border-subtle hover:bg-interactive-hover"
                  >
                    {s.name}
                  </button>
                ))}
              </div>
            </section>
          )}

          {/* Recent */}
          {recent.length > 0 && (
            <section className="p-4">
              <h3 className="flex items-center gap-2 text-text-secondary text-xs font-medium uppercase mb-2">
                <Clock size={14} />
                {t('sessions.recent')}
              </h3>
              <ul className="space-y-1">
                {recent.map((s) => (
                  <li key={s.id}>
                    <button
                      onClick={() => onSessionSelect?.(s)}
                      className="w-full text-left px-3 py-2 rounded text-sm text-text-primary hover:bg-interactive-hover"
                    >
                      {s.name}
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {/* All Sessions */}
          <section className="p-4">
            <h3 className="flex items-center gap-2 text-text-secondary text-xs font-medium uppercase mb-2">
              <FolderTree size={14} />
              {t('sessions.allSessions')}
            </h3>
            <ul className="space-y-1">
              {sessions.map((s) => (
                <li key={s.id}>
                  <button
                    onClick={() => onSessionSelect?.(s)}
                    className="w-full text-left px-3 py-2 rounded text-sm text-text-primary hover:bg-interactive-hover"
                  >
                    {s.name}
                  </button>
                </li>
              ))}
            </ul>
          </section>
        </div>
      </aside>
    </>
  );
}
