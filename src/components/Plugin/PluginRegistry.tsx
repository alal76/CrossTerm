import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Search, Download, RefreshCw, Check, ArrowUpCircle } from 'lucide-react';
import clsx from 'clsx';
import type { PluginRegistryEntry } from '@/types';

const REGISTRY_URL = 'https://registry.crossterm.dev/plugins.json';

type FilterCategory = 'all' | 'terminal' | 'network' | 'ui' | 'security';

interface PluginRegistryProps {
  className?: string;
}

export default function PluginRegistry({ className }: Readonly<PluginRegistryProps>) {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<PluginRegistryEntry[]>([]);
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState<FilterCategory>('all');
  const [loading, setLoading] = useState(false);

  const fetchRegistry = useCallback(async () => {
    setLoading(true);
    try {
      const res = await fetch(REGISTRY_URL);
      if (res.ok) {
        const data: PluginRegistryEntry[] = await res.json();
        setEntries(data);
      }
    } catch {
      // Stub: registry not available
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchRegistry();
  }, [fetchRegistry]);

  const filtered = entries.filter((e) => {
    const matchesSearch =
      !search ||
      e.name.toLowerCase().includes(search.toLowerCase()) ||
      e.description.toLowerCase().includes(search.toLowerCase());
    const matchesCategory = category === 'all' || e.category === category;
    return matchesSearch && matchesCategory;
  });

  const categories: FilterCategory[] = ['all', 'terminal', 'network', 'ui', 'security'];

  return (
    <div className={clsx('flex flex-col h-full', className)}>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border-default">
        <h2 className="text-lg font-semibold text-text-primary">
          {t('plugin.registry')}
        </h2>
        <button
          onClick={fetchRegistry}
          className="p-2 rounded hover:bg-interactive-hover text-text-secondary"
          disabled={loading}
        >
          <RefreshCw size={16} className={clsx(loading && 'animate-spin')} />
        </button>
      </div>

      {/* Search */}
      <div className="px-4 py-2">
        <div className="flex items-center gap-2 px-3 py-1.5 rounded bg-surface-secondary border border-border-subtle">
          <Search size={14} className="text-text-secondary" />
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t('sessions.search')}
            className="flex-1 bg-transparent text-sm text-text-primary outline-none"
          />
        </div>
      </div>

      {/* Category Filters */}
      <div className="flex gap-1 px-4 pb-2 overflow-x-auto">
        {categories.map((cat) => (
          <button
            key={cat}
            onClick={() => setCategory(cat)}
            className={clsx(
              'px-3 py-1 rounded-full text-xs font-medium whitespace-nowrap',
              category === cat
                ? 'bg-accent-primary text-text-inverse'
                : 'bg-surface-secondary text-text-secondary hover:bg-interactive-hover'
            )}
          >
            {cat.charAt(0).toUpperCase() + cat.slice(1)}
          </button>
        ))}
      </div>

      {/* Plugin List */}
      <div className="flex-1 overflow-y-auto px-4">
        {filtered.length === 0 && !loading && (
          <p className="text-sm text-text-secondary py-4 text-center">
            {t('plugin.noPlugins')}
          </p>
        )}
        {filtered.map((entry) => (
          <div
            key={entry.id}
            className="flex items-center gap-3 py-3 border-b border-border-subtle"
          >
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium text-text-primary truncate">
                  {entry.name}
                </span>
                <span className="text-xs text-text-secondary">v{entry.version}</span>
              </div>
              <p className="text-xs text-text-secondary truncate">{entry.description}</p>
              <p className="text-xs text-text-disabled">{entry.author}</p>
            </div>
            <div className="flex-shrink-0">
              {entry.installed && entry.update_available && (
                <button className="flex items-center gap-1 px-3 py-1 rounded bg-accent-primary text-text-inverse text-xs">
                  <ArrowUpCircle size={12} />
                  {t('plugin.update')}
                </button>
              )}
              {entry.installed && !entry.update_available && (
                <span className="flex items-center gap-1 text-xs text-status-connected">
                  <Check size={12} />
                  {t('plugin.installed')}
                </span>
              )}
              {!entry.installed && (
                <button className="flex items-center gap-1 px-3 py-1 rounded bg-interactive-default text-text-primary text-xs hover:bg-interactive-hover">
                  <Download size={12} />
                  {t('plugin.install')}
                </button>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
