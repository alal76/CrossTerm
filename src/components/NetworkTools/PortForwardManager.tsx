import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { clsx } from 'clsx';
import {
  ArrowUpDown,
  Plus,
  Trash2,
  ToggleLeft,
  ToggleRight,
  Circle,
} from 'lucide-react';
import type { TunnelRule, TunnelStatus } from '@/types';

interface TunnelEntry {
  rule: TunnelRule;
  status: TunnelStatus;
}

export default function PortForwardManager() {
  const { t } = useTranslation();
  const [tunnels, setTunnels] = useState<TunnelEntry[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [formData, setFormData] = useState({
    name: '',
    local_port: '',
    remote_host: '',
    remote_port: '',
    tunnel_type: 'local' as 'local' | 'remote' | 'dynamic',
    auto_start: false,
  });

  const loadTunnels = useCallback(async () => {
    try {
      const list = await invoke<[TunnelRule, TunnelStatus][]>(
        'network_tunnel_list'
      );
      setTunnels(list.map(([rule, status]) => ({ rule, status })));
    } catch {
      // handle error
    }
  }, []);

  useEffect(() => {
    loadTunnels();
    const unlisten = listen<{ rule_id: string; status: TunnelStatus }>(
      'network:tunnel_status',
      () => {
        loadTunnels();
      }
    );
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loadTunnels]);

  const handleCreate = useCallback(async () => {
    const localPort = Number.parseInt(formData.local_port, 10);
    const remotePort = Number.parseInt(formData.remote_port, 10);
    if (!formData.name || Number.isNaN(localPort) || Number.isNaN(remotePort)) return;

    try {
      await invoke('network_tunnel_create', {
        rule: {
          id: '',
          name: formData.name,
          local_port: localPort,
          remote_host: formData.remote_host,
          remote_port: remotePort,
          tunnel_type: formData.tunnel_type,
          ssh_session_ref: null,
          auto_start: formData.auto_start,
          enabled: false,
        },
      });
      setShowForm(false);
      setFormData({
        name: '',
        local_port: '',
        remote_host: '',
        remote_port: '',
        tunnel_type: 'local',
        auto_start: false,
      });
      loadTunnels();
    } catch {
      // handle error
    }
  }, [formData, loadTunnels]);

  const handleToggle = useCallback(
    async (ruleId: string, enabled: boolean) => {
      try {
        await invoke('network_tunnel_toggle', { ruleId, enabled });
        loadTunnels();
      } catch {
        // handle error
      }
    },
    [loadTunnels]
  );

  const handleRemove = useCallback(
    async (ruleId: string) => {
      try {
        await invoke('network_tunnel_remove', { ruleId });
        loadTunnels();
      } catch {
        // handle error
      }
    },
    [loadTunnels]
  );

  const activeCount = tunnels.filter((t) => t.status === 'active').length;

  return (
    <div className="flex flex-col gap-4 p-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <ArrowUpDown size={20} className="text-text-secondary" />
          <h2 className="text-base font-semibold text-text-primary">
            {t('network.tunnels')}
          </h2>
          {activeCount > 0 && (
            <span className="rounded-full bg-status-connected px-2 py-0.5 text-xs text-text-inverse">
              {activeCount} {t('network.tunnelActive').toLowerCase()}
            </span>
          )}
        </div>
        <button
          onClick={() => setShowForm(!showForm)}
          className="flex items-center gap-1 rounded-md bg-interactive-default px-3 py-1.5 text-sm text-text-inverse hover:bg-interactive-hover"
        >
          <Plus size={14} />
          {t('network.addTunnel')}
        </button>
      </div>

      {showForm && (
        <div className="rounded-md border border-border-default bg-surface-secondary p-3">
          <div className="grid grid-cols-2 gap-3">
            <input
              placeholder="Name"
              value={formData.name}
              onChange={(e) =>
                setFormData((p) => ({ ...p, name: e.target.value }))
              }
              className="rounded border border-border-default bg-surface-primary px-2 py-1.5 text-sm text-text-primary focus:border-border-focus focus:outline-none"
            />
            <select
              value={formData.tunnel_type}
              onChange={(e) =>
                setFormData((p) => ({
                  ...p,
                  tunnel_type: e.target.value as 'local' | 'remote' | 'dynamic',
                }))
              }
              className="rounded border border-border-default bg-surface-primary px-2 py-1.5 text-sm text-text-primary focus:border-border-focus focus:outline-none"
            >
              <option value="local">Local</option>
              <option value="remote">Remote</option>
              <option value="dynamic">Dynamic</option>
            </select>
            <input
              placeholder="Local Port"
              type="number"
              value={formData.local_port}
              onChange={(e) =>
                setFormData((p) => ({ ...p, local_port: e.target.value }))
              }
              className="rounded border border-border-default bg-surface-primary px-2 py-1.5 text-sm text-text-primary focus:border-border-focus focus:outline-none"
            />
            <input
              placeholder="Remote Host"
              value={formData.remote_host}
              onChange={(e) =>
                setFormData((p) => ({ ...p, remote_host: e.target.value }))
              }
              className="rounded border border-border-default bg-surface-primary px-2 py-1.5 text-sm text-text-primary focus:border-border-focus focus:outline-none"
            />
            <input
              placeholder="Remote Port"
              type="number"
              value={formData.remote_port}
              onChange={(e) =>
                setFormData((p) => ({ ...p, remote_port: e.target.value }))
              }
              className="rounded border border-border-default bg-surface-primary px-2 py-1.5 text-sm text-text-primary focus:border-border-focus focus:outline-none"
            />
            <label className="flex items-center gap-2 text-sm text-text-secondary">
              <input
                type="checkbox"
                checked={formData.auto_start}
                onChange={(e) =>
                  setFormData((p) => ({ ...p, auto_start: e.target.checked }))
                }
              />{' '}
              Auto-start
            </label>
          </div>
          <div className="mt-3 flex gap-2">
            <button
              onClick={handleCreate}
              className="rounded bg-interactive-default px-3 py-1.5 text-sm text-text-inverse hover:bg-interactive-hover"
            >
              {t('actions.create')}
            </button>
            <button
              onClick={() => setShowForm(false)}
              className="rounded bg-surface-elevated px-3 py-1.5 text-sm text-text-primary hover:bg-surface-secondary"
            >
              {t('actions.cancel')}
            </button>
          </div>
        </div>
      )}

      <div className="flex flex-col gap-1">
        {tunnels.map(({ rule, status }) => (
          <div
            key={rule.id}
            className="flex items-center justify-between rounded-md border border-border-subtle px-3 py-2 hover:bg-surface-secondary"
          >
            <div className="flex items-center gap-3">
              <Circle
                size={8}
                className={clsx(
                  'fill-current',
                  status === 'active'
                    ? 'text-status-connected'
                    : 'text-status-disconnected'
                )}
              />
              <div>
                <div className="text-sm font-medium text-text-primary">
                  {rule.name}
                </div>
                <div className="text-xs text-text-secondary">
                  {rule.tunnel_type} · :{rule.local_port} → {rule.remote_host}:
                  {rule.remote_port}
                </div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={() => handleToggle(rule.id, !rule.enabled)}
                className="p-1 text-text-secondary hover:text-text-primary"
                title={
                  rule.enabled
                    ? t('network.tunnelActive')
                    : t('network.tunnelInactive')
                }
              >
                {rule.enabled ? (
                  <ToggleRight size={20} className="text-status-connected" />
                ) : (
                  <ToggleLeft size={20} />
                )}
              </button>
              <button
                onClick={() => handleRemove(rule.id)}
                className="p-1 text-text-secondary hover:text-status-disconnected"
              >
                <Trash2 size={14} />
              </button>
            </div>
          </div>
        ))}

        {tunnels.length === 0 && (
          <div className="py-8 text-center text-sm text-text-disabled">
            {t('network.tunnels')} — {t('sessions.emptyState')}
          </div>
        )}
      </div>
    </div>
  );
}
