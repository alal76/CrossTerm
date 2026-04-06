import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Puzzle, ToggleLeft, ToggleRight, Trash2, Upload, Shield, AlertCircle } from 'lucide-react';
import clsx from 'clsx';
import type { PluginInfo, PluginPermission } from '@/types';

export default function PluginManager() {
  const { t } = useTranslation();
  const [plugins, setPlugins] = useState<PluginInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadPlugins = useCallback(async () => {
    try {
      setLoading(true);
      const list = await invoke<PluginInfo[]>('plugin_list');
      setPlugins(list);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadPlugins();
  }, [loadPlugins]);

  const handleToggle = async (pluginId: string, currentEnabled: boolean) => {
    try {
      if (currentEnabled) {
        await invoke('plugin_disable', { pluginId });
      } else {
        await invoke('plugin_enable', { pluginId });
        await invoke('plugin_load', { pluginId });
      }
      await loadPlugins();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleUninstall = async (pluginId: string) => {
    if (!globalThis.confirm(t('plugin.uninstall') + '?')) return;
    try {
      await invoke('plugin_uninstall', { pluginId });
      await loadPlugins();
    } catch (err) {
      setError(String(err));
    }
  };

  const handleInstall = async () => {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({
        filters: [{ name: 'WASM Plugin', extensions: ['wasm'] }],
      });
      if (selected) {
        await invoke('plugin_install', { path: selected });
        await loadPlugins();
      }
    } catch (err) {
      setError(String(err));
    }
  };

  const permissionLabel = (perm: PluginPermission): string => {
    const labels: Record<PluginPermission, string> = {
      network: 'Network',
      file_system: 'File System',
      terminal: 'Terminal',
      clipboard: 'Clipboard',
      notifications: 'Notifications',
      settings: 'Settings',
    };
    return labels[perm] || perm;
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full text-text-secondary">
        <Puzzle size={20} className="mr-2 animate-spin" />
        {t('settings.loading')}
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full bg-surface-primary p-4 overflow-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-text-primary flex items-center gap-2">
          <Puzzle size={20} />
          {t('plugin.manager')}
        </h2>
        <button
          onClick={handleInstall}
          className="flex items-center gap-1 px-3 py-1.5 rounded bg-interactive-default text-text-inverse text-sm hover:bg-interactive-hover transition-colors"
        >
          <Upload size={14} />
          {t('plugin.install')}
        </button>
      </div>

      {error && (
        <div className="mb-3 px-3 py-2 rounded bg-red-500/10 text-red-400 text-sm flex items-center gap-2">
          <AlertCircle size={14} />
          {error}
        </div>
      )}

      {/* Plugin Grid */}
      {plugins.length === 0 ? (
        <div className="flex flex-col items-center justify-center flex-1 text-text-secondary gap-2">
          <Puzzle size={48} className="opacity-30" />
          <p className="text-sm">{t('plugin.noPlugins')}</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {plugins.map((plugin) => (
            <div
              key={plugin.manifest.id}
              className={clsx(
                'rounded-lg border p-4 transition-colors',
                plugin.enabled
                  ? 'border-accent-primary bg-surface-elevated'
                  : 'border-border-default bg-surface-secondary'
              )}
            >
              {/* Plugin Header */}
              <div className="flex items-start justify-between mb-2">
                <div>
                  <h3 className="font-medium text-text-primary">{plugin.manifest.name}</h3>
                  <p className="text-xs text-text-secondary">
                    v{plugin.manifest.version} &middot; {plugin.manifest.author}
                  </p>
                </div>
                <button
                  onClick={() => handleToggle(plugin.manifest.id, plugin.enabled)}
                  className="text-text-secondary hover:text-text-primary transition-colors"
                  title={plugin.enabled ? t('plugin.disable') : t('plugin.enable')}
                >
                  {plugin.enabled ? (
                    <ToggleRight size={24} className="text-accent-primary" />
                  ) : (
                    <ToggleLeft size={24} />
                  )}
                </button>
              </div>

              {/* Description */}
              <p className="text-sm text-text-secondary mb-3">
                {plugin.manifest.description}
              </p>

              {/* Permissions */}
              {plugin.manifest.permissions.length > 0 && (
                <div className="mb-3">
                  <div className="flex items-center gap-1 mb-1 text-xs text-text-secondary">
                    <Shield size={11} />
                    {t('plugin.permissions')}
                  </div>
                  <div className="flex flex-wrap gap-1">
                    {plugin.manifest.permissions.map((perm) => (
                      <span
                        key={perm}
                        className="px-1.5 py-0.5 rounded text-xs bg-surface-sunken text-text-secondary"
                      >
                        {permissionLabel(perm)}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {/* Error */}
              {plugin.error && (
                <p className="text-xs text-red-400 mb-2">{plugin.error}</p>
              )}

              {/* Actions */}
              <div className="flex justify-end">
                <button
                  onClick={() => handleUninstall(plugin.manifest.id)}
                  className="flex items-center gap-1 px-2 py-1 text-xs text-red-400 hover:text-red-300 hover:bg-red-500/10 rounded transition-colors"
                >
                  <Trash2 size={12} />
                  {t('plugin.uninstall')}
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
