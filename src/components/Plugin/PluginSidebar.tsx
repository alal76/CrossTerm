import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Puzzle, ChevronDown, ChevronRight } from 'lucide-react';
import clsx from 'clsx';

interface PluginPanel {
  id: string;
  pluginId: string;
  icon?: React.ReactNode;
  title: string;
  content: React.ReactNode;
}

interface PluginSidebarProps {
  panels?: PluginPanel[];
}

export default function PluginSidebar({ panels = [] }: Readonly<PluginSidebarProps>) {
  const { t } = useTranslation();
  const [expandedPanels, setExpandedPanels] = useState<Set<string>>(new Set());

  const togglePanel = (id: string) => {
    setExpandedPanels((prev) => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };

  if (panels.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center p-6 text-text-secondary">
        <Puzzle size={32} className="mb-2 opacity-50" />
        <p className="text-sm">{t('plugin.sidebar')}</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-y-auto">
      {panels.map((panel) => {
        const isExpanded = expandedPanels.has(panel.id);
        return (
          <div key={panel.id} className="border-b border-border-subtle">
            <button
              onClick={() => togglePanel(panel.id)}
              className="flex items-center gap-2 w-full px-3 py-2 text-sm font-medium text-text-primary hover:bg-interactive-hover"
            >
              {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              {panel.icon ?? <Puzzle size={14} />}
              <span className="truncate">{panel.title}</span>
            </button>
            {isExpanded && (
              <div className={clsx('px-3 pb-3')}>
                {panel.content}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
