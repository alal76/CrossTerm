import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { clsx } from 'clsx';
import { Search, Wifi, Loader2, Save, PlugZap } from 'lucide-react';
import type { ScanResult } from '@/types';

export default function NetworkScanner() {
  const { t } = useTranslation();
  const [cidr, setCidr] = useState('');
  const [scanning, setScanning] = useState(false);
  const [scanId, setScanId] = useState<string | null>(null);
  const [results, setResults] = useState<ScanResult[]>([]);
  const [progress, setProgress] = useState({ scanned: 0, total: 0 });

  useEffect(() => {
    const unlistenHost = listen<{ scan_id: string; result: ScanResult }>(
      'network:scan_host_found',
      (event) => {
        if (event.payload.scan_id === scanId) {
          setResults((prev) => [...prev, event.payload.result]);
        }
      }
    );

    const unlistenProgress = listen<{
      scan_id: string;
      hosts_scanned: number;
      total_hosts: number;
    }>('network:scan_progress', (event) => {
      if (event.payload.scan_id === scanId) {
        setProgress({
          scanned: event.payload.hosts_scanned,
          total: event.payload.total_hosts,
        });
        if (event.payload.hosts_scanned >= event.payload.total_hosts) {
          setScanning(false);
        }
      }
    });

    return () => {
      unlistenHost.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
    };
  }, [scanId]);

  const handleScan = useCallback(async () => {
    if (!cidr.trim()) return;
    setResults([]);
    setScanning(true);
    setProgress({ scanned: 0, total: 0 });

    try {
      const id = await invoke<string>('network_scan_start', {
        target: { cidr: cidr.trim() },
      });
      setScanId(id);
    } catch {
      setScanning(false);
    }
  }, [cidr]);

  const handleSaveAsSessions = useCallback(async () => {
    if (!scanId) return;
    try {
      await invoke('network_scan_save_as_sessions', {
        scanId,
        folder: 'Scanned Hosts',
      });
    } catch {
      // handle error
    }
  }, [scanId]);

  const handleConnect = useCallback((_result: ScanResult) => {
    // Detect session type by open ports and create session
    // This would integrate with sessionStore
  }, []);

  return (
    <div className="flex flex-col gap-4 p-4">
      <div className="flex items-center gap-2">
        <Wifi size={20} className="text-text-secondary" />
        <h2 className="text-base font-semibold text-text-primary">
          {t('network.scanner')}
        </h2>
      </div>

      <div className="flex gap-2">
        <input
          type="text"
          value={cidr}
          onChange={(e) => setCidr(e.target.value)}
          placeholder={t('network.cidrPlaceholder')}
          className="flex-1 rounded-md border border-border-default bg-surface-primary px-3 py-2 text-sm text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
          onKeyDown={(e) => e.key === 'Enter' && handleScan()}
        />
        <button
          onClick={handleScan}
          disabled={scanning || !cidr.trim()}
          className={clsx(
            'flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium transition-colors',
            scanning
              ? 'cursor-not-allowed bg-interactive-disabled text-text-disabled'
              : 'bg-interactive-default text-text-inverse hover:bg-interactive-hover'
          )}
        >
          {scanning ? (
            <Loader2 size={14} className="animate-spin" />
          ) : (
            <Search size={14} />
          )}
          {scanning ? t('network.scanning') : t('network.scan')}
        </button>
      </div>

      {scanning && progress.total > 0 && (
        <div className="flex items-center gap-2">
          <div className="h-1.5 flex-1 rounded-full bg-surface-sunken">
            <div
              className="h-full rounded-full bg-accent-primary transition-all"
              style={{
                width: `${(progress.scanned / progress.total) * 100}%`,
              }}
            />
          </div>
          <span className="text-xs text-text-secondary">
            {progress.scanned}/{progress.total}
          </span>
        </div>
      )}

      {results.length > 0 && (
        <>
          <div className="overflow-auto rounded-md border border-border-default">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border-subtle bg-surface-secondary">
                  <th className="px-3 py-2 text-left font-medium text-text-secondary">
                    {t('network.ip')}
                  </th>
                  <th className="px-3 py-2 text-left font-medium text-text-secondary">
                    {t('network.hostname')}
                  </th>
                  <th className="px-3 py-2 text-left font-medium text-text-secondary">
                    {t('network.openPorts')}
                  </th>
                  <th className="px-3 py-2 text-right font-medium text-text-secondary" />
                </tr>
              </thead>
              <tbody>
                {results.map((result, i) => (
                  <tr
                    key={result.ip + i}
                    className="border-b border-border-subtle last:border-0 hover:bg-surface-secondary"
                  >
                    <td className="px-3 py-2 text-text-primary">{result.ip}</td>
                    <td className="px-3 py-2 text-text-secondary">
                      {result.hostname ?? '—'}
                    </td>
                    <td className="px-3 py-2">
                      <div className="flex flex-wrap gap-1">
                        {result.open_ports.map((p) => (
                          <span
                            key={p.port}
                            className="rounded bg-surface-elevated px-1.5 py-0.5 text-xs text-text-secondary"
                          >
                            {p.port}/{p.service_name}
                          </span>
                        ))}
                      </div>
                    </td>
                    <td className="px-3 py-2 text-right">
                      <button
                        onClick={() => handleConnect(result)}
                        className="inline-flex items-center gap-1 rounded px-2 py-1 text-xs text-accent-primary hover:bg-surface-elevated"
                      >
                        <PlugZap size={12} />
                        {t('network.connect')}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <button
            onClick={handleSaveAsSessions}
            className="flex items-center gap-2 self-start rounded-md bg-surface-elevated px-3 py-2 text-sm text-text-primary hover:bg-surface-secondary"
          >
            <Save size={14} />
            {t('network.saveAsSessions')}
          </button>
        </>
      )}
    </div>
  );
}
