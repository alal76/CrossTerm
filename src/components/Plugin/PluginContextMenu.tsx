import { useEffect, useRef } from 'react';
import clsx from 'clsx';

interface ContextMenuItem {
  id: string;
  pluginId: string;
  label: string;
  icon?: React.ReactNode;
  priority: number;
  group?: string;
  onClick: () => void;
}

interface PluginContextMenuProps {
  items: ContextMenuItem[];
  position: { x: number; y: number };
  onClose: () => void;
}

export default function PluginContextMenu({
  items,
  position,
  onClose,
}: Readonly<PluginContextMenuProps>) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [onClose]);

  const sorted = [...items].sort((a, b) => a.priority - b.priority);

  // Group items and insert separators
  const grouped: (ContextMenuItem | 'separator')[] = [];
  let lastGroup: string | undefined;
  for (const item of sorted) {
    if (lastGroup !== undefined && item.group !== lastGroup) {
      grouped.push('separator');
    }
    grouped.push(item);
    lastGroup = item.group;
  }

  return (
    <div
      ref={menuRef}
      className={clsx(
        'fixed z-50 min-w-[180px] py-1 rounded-lg shadow-lg',
        'bg-surface-overlay border border-border-default'
      )}
      style={{ left: position.x, top: position.y }}
      role="menu"
    >
      {grouped.map((item, idx) => {
        if (item === 'separator') {
          return (
            <hr
              key={`sep-${item}-${String(idx)}`}
              className="my-1 border-t border-border-subtle"
            />
          );
        }
        return (
          <button
            key={item.id}
            role="menuitem"
            onClick={() => {
              item.onClick();
              onClose();
            }}
            className="flex items-center gap-2 w-full px-3 py-1.5 text-sm text-text-primary hover:bg-interactive-hover"
          >
            {item.icon}
            <span>{item.label}</span>
          </button>
        );
      })}
    </div>
  );
}
