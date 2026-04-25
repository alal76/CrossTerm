import { useState, useEffect, useCallback, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { clsx } from 'clsx';
import {
  Search,
  Radar,
  Loader2,
  Save,
  PlugZap,
  Plus,
  X,
  Monitor,
  Server,
  ChevronDown,
  ChevronUp,
  Filter,
  Wifi,
  ShieldAlert,
} from 'lucide-react';
import type { ExploreResult, ExploreProgress, ExploreHostFound, ServiceFilter } from '@/types';
import WifiScanner from '@/components/NetworkTools/WifiScanner';
import AircrackPanel from '@/components/NetworkTools/AircrackPanel';

const WELL_KNOWN_SERVICES: { id: ServiceFilter; label: string; port: number }[] = [
  { id: 'ssh', label: 'SSH (22)', port: 22 },
  { id: 'rdp', label: 'RDP (3389)', port: 3389 },
  { id: 'vnc', label: 'VNC (5900)', port: 5900 },
  { id: 'http', label: 'HTTP (80)', port: 80 },
  { id: 'https', label: 'HTTPS (443)', port: 443 },
  { id: 'telnet', label: 'Telnet (23)', port: 23 },
  { id: 'ftp', label: 'FTP (21)', port: 21 },
  { id: 'smb', label: 'SMB (445)', port: 445 },
  { id: 'mysql', label: 'MySQL (3306)', port: 3306 },
  { id: 'postgresql', label: 'PostgreSQL (5432)', port: 5432 },
  { id: 'redis', label: 'Redis (6379)', port: 6379 },
  { id: 'mongodb', label: 'MongoDB (27017)', port: 27017 },
];

const SESSION_ICON: Record<string, string> = {
  ssh: '🔒',
  rdp: '🖥',
  vnc: '📺',
  telnet: '📡',
  sftp: '📂',
};

function parseExtraPorts(input: string): number[] {
  if (!input.trim()) return [];
  const ports: number[] = [];
  for (const part of input.split(',')) {
    const trimmed = part.trim();
    if (!trimmed) continue;
    const num = Number(trimmed);
    if (Number.isInteger(num) && num >= 1 && num <= 65535) {
      ports.push(num);
    }
  }
  return [...new Set(ports)];
}

const SERVICE_PORT_COLORS: Record<string, string> = {
  ssh: 'bg-green-500/20 text-green-400',
  rdp: 'bg-blue-500/20 text-blue-400',
  vnc: 'bg-purple-500/20 text-purple-400',
  http: 'bg-yellow-500/20 text-yellow-400',
  https: 'bg-yellow-500/20 text-yellow-400',
  telnet: 'bg-orange-500/20 text-orange-400',
  ftp: 'bg-cyan-500/20 text-cyan-400',
  smb: 'bg-red-500/20 text-red-400',
};

export default function NetworkExplorer() {
  const { t } = useTranslation();
  const [toolTab, setToolTab] = useState<'explore' | 'wifi' | 'aircrack'>('explore');
  const [cidr, setCidr] = useState('');
  const [scanning, setScanning] = useState(false);
  const [scanId, setScanId] = useState<string | null>(null);
  const [results, setResults] = useState<ExploreResult[]>([]);
  const [progress, setProgress] = useState<ExploreProgress | null>(null);
  const [extraPortsInput, setExtraPortsInput] = useState('');
  const [selectedServices, setSelectedServices] = useState<Set<string>>(
    () => new Set(['ssh', 'rdp', 'vnc', 'http', 'https'])
  );
  const [showFilters, setShowFilters] = useState(false);
  const [filterService, setFilterService] = useState<string>('all');
  const [sortBy, setSortBy] = useState<'ip' | 'ports' | 'response'>('ip');

  useEffect(() => {
    const unlistenHost = listen<ExploreHostFound>(
      'network:explore_host_found',
      (event) => {
        if (event.payload.scan_id === scanId) {
          setResults((prev) => [...prev, event.payload.result]);
        }
      }
    );

    const unlistenProgress = listen<ExploreProgress>(
      'network:explore_progress',
      (event) => {
        if (event.payload.scan_id === scanId) {
          setProgress(event.payload);
          if (event.payload.hosts_scanned >= event.payload.total_hosts) {
            setScanning(false);
          }
        }
      }
    );

    return () => {
      unlistenHost.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
    };
  }, [scanId]);

  const toggleService = useCallback((svc: string) => {
    setSelectedServices((prev) => {
      const next = new Set(prev);
      if (next.has(svc)) {
        next.delete(svc);
      } else {
        next.add(svc);
      }
      return next;
    });
  }, []);

  const handleScan = useCallback(async () => {
    if (!cidr.trim()) return;
    setResults([]);
    setScanning(true);
    setProgress(null);

    const services: ServiceFilter[] = Array.from(selectedServices) as ServiceFilter[];
    const extra_ports = parseExtraPorts(extraPortsInput);

    try {
      const id = await invoke<string>('network_explore_start', {
        target: { cidr: cidr.trim(), services, extra_ports },
      });
      setScanId(id);
    } catch {
      setScanning(false);
    }
  }, [cidr, selectedServices, extraPortsInput]);

  const handleConnect = useCallback((_result: ExploreResult) => {
    // TODO: Integrate with sessionStore to create+open a session
  }, []);

  const handleSaveAsSessions = useCallback(async () => {
    if (!scanId) return;
    try {
      await invoke('network_scan_save_as_sessions', {
        scanId,
        folder: 'Discovered Hosts',
      });
    } catch {
      // handle error
    }
  }, [scanId]);

  const filteredResults = useMemo(() => {
    let filtered = results;
    if (filterService !== 'all') {
      filtered = filtered.filter((r) =>
        r.open_ports.some((p) => p.service_name === filterService)
      );
    }
    const sorted = [...filtered];
    switch (sortBy) {
      case 'ports':
        sorted.sort((a, b) => b.open_ports.length - a.open_ports.length);
        break;
      case 'response':
        sorted.sort((a, b) => a.response_time_ms - b.response_time_ms);
        break;
      default:
        sorted.sort((a, b) => a.ip.localeCompare(b.ip, undefined, { numeric: true }));
    }
    return sorted;
  }, [results, filterService, sortBy]);

  const serviceCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const r of results) {
      for (const p of r.open_ports) {
        counts[p.service_name] = (counts[p.service_name] ?? 0) + 1;
      }
    }
    return counts;
  }, [results]);

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Tool tabs */}
      <div className="flex items-center gap-1 border-b border-border-subtle px-3 pt-2">
        <button
          onClick={() => setToolTab('explore')}
          className={clsx(
            'flex items-center gap-1.5 border-b-2 px-3 py-1.5 text-xs font-medium transition-colors',
            toolTab === 'explore'
              ? 'border-interactive-default text-text-primary'
              : 'border-transparent text-text-secondary hover:text-text-primary'
          )}
        >
          <Radar size={14} />
          {t('network.explore')}
        </button>
        <button
          onClick={() => setToolTab('wifi')}
          className={clsx(
            'flex items-center gap-1.5 border-b-2 px-3 py-1.5 text-xs font-medium transition-colors',
            toolTab === 'wifi'
              ? 'border-interactive-default text-text-primary'
              : 'border-transparent text-text-secondary hover:text-text-primary'
          )}
        >
          <Wifi size={14} />
          {t('network.wifi')}
        </button>
        <button
          onClick={() => setToolTab('aircrack')}
          className={clsx(
            'flex items-center gap-1.5 border-b-2 px-3 py-1.5 text-xs font-medium transition-colors',
            toolTab === 'aircrack'
              ? 'border-red-500 text-red-400'
              : 'border-transparent text-text-secondary hover:text-text-primary'
          )}
        >
          <ShieldAlert size={14} />
          {t('network.aircrackTab')}
        </button>
      </div>

      {toolTab === 'wifi' ? (
        <WifiScanner />
      ) : toolTab === 'aircrack' ? (
        <AircrackPanel />
      ) : (
    <div className="flex flex-col gap-3 p-3 flex-1 overflow-y-auto">
      {/* Header */}
      <div className="flex items-center gap-2">
        <Radar size={20} className="text-accent-primary" />
        <h2 className="text-sm font-semibold text-text-primary">
          {t('network.explore')}
        </h2>
      </div>

      {/* CIDR input row */}
      <div className="flex gap-2">
        <input
          type="text"
          value={cidr}
          onChange={(e) => setCidr(e.target.value)}
          placeholder={t('network.cidrPlaceholder')}
          className="flex-1 rounded-md border border-border-default bg-surface-primary px-3 py-1.5 text-sm text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
          onKeyDown={(e) => e.key === 'Enter' && handleScan()}
        />
        <button
          onClick={handleScan}
          disabled={scanning || !cidr.trim()}
          className={clsx(
            'flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors',
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
          {scanning ? t('network.scanning') : t('network.explore')}
        </button>
      </div>

      {/* Service filter toggles */}
      <div>
        <button
          onClick={() => setShowFilters(!showFilters)}
          className="flex items-center gap-1.5 text-xs text-text-secondary hover:text-text-primary transition-colors"
        >
          <Filter size={12} />
          {t('network.serviceFilters')}
          {showFilters ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
        </button>

        {showFilters && (
          <div className="mt-2 flex flex-col gap-2 rounded-md border border-border-subtle bg-surface-secondary p-3">
            <div className="flex flex-wrap gap-1.5">
              {WELL_KNOWN_SERVICES.map((svc) => (
                <button
                  key={typeof svc.id === 'string' ? svc.id : `custom-${svc.port}`}
                  onClick={() => toggleService(typeof svc.id === 'string' ? svc.id : String(svc.port))}
                  className={clsx(
                    'rounded-full px-2.5 py-1 text-xs font-medium transition-colors border',
                    selectedServices.has(typeof svc.id === 'string' ? svc.id : String(svc.port))
                      ? 'bg-accent-primary/20 text-accent-primary border-accent-primary/40'
                      : 'bg-surface-primary text-text-secondary border-border-subtle hover:border-border-default'
                  )}
                >
                  {svc.label}
                </button>
              ))}
            </div>

            {/* Extra ports input */}
            <div className="flex items-center gap-2">
              <Plus size={12} className="text-text-secondary shrink-0" />
              <input
                type="text"
                value={extraPortsInput}
                onChange={(e) => setExtraPortsInput(e.target.value)}
                placeholder={t('network.extraPortsPlaceholder')}
                className="flex-1 rounded border border-border-default bg-surface-primary px-2 py-1 text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
              />
            </div>
          </div>
        )}
      </div>

      {/* Progress */}
      {scanning && progress && progress.total_hosts > 0 && (
        <div className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <div className="h-1.5 flex-1 rounded-full bg-surface-sunken">
              <div
                className="h-full rounded-full bg-accent-primary transition-all"
                style={{
                  width: `${(progress.hosts_scanned / progress.total_hosts) * 100}%`,
                }}
              />
            </div>
            <span className="text-xs text-text-secondary tabular-nums">
              {progress.hosts_scanned}/{progress.total_hosts}
            </span>
          </div>
          <span className="text-xs text-text-secondary">
            {t('network.hostsFound', { count: progress.hosts_found })}
          </span>
        </div>
      )}

      {/* Summary badges */}
      {results.length > 0 && !scanning && (
        <div className="flex items-center gap-2 flex-wrap">
          <span className="flex items-center gap-1 rounded bg-surface-elevated px-2 py-1 text-xs text-text-primary">
            <Server size={11} />
            {t('network.hostsFound', { count: results.length })}
          </span>
          {Object.entries(serviceCounts).map(([svc, count]) => (
            <button
              key={svc}
              onClick={() => setFilterService(filterService === svc ? 'all' : svc)}
              className={clsx(
                'rounded px-2 py-1 text-xs font-medium transition-colors',
                filterService === svc
                  ? 'bg-accent-primary/20 text-accent-primary'
                  : SERVICE_PORT_COLORS[svc] ?? 'bg-surface-elevated text-text-secondary'
              )}
            >
              {svc}: {count}
            </button>
          ))}
          {filterService !== 'all' && (
            <button
              onClick={() => setFilterService('all')}
              className="flex items-center gap-1 rounded px-2 py-1 text-xs text-text-secondary hover:text-text-primary"
            >
              <X size={10} />
              {t('network.clearFilter')}
            </button>
          )}
        </div>
      )}

      {/* Sort controls */}
      {results.length > 0 && (
        <div className="flex items-center gap-2 text-xs text-text-secondary">
          <span>{t('network.sortBy')}:</span>
          {(['ip', 'ports', 'response'] as const).map((key) => (
            <button
              key={key}
              onClick={() => setSortBy(key)}
              className={clsx(
                'rounded px-2 py-0.5 transition-colors',
                sortBy === key
                  ? 'bg-surface-elevated text-text-primary'
                  : 'hover:text-text-primary'
              )}
            >
              {t(`network.sort_${key}`)}
            </button>
          ))}
        </div>
      )}

      {/* Results table */}
      {filteredResults.length > 0 && (
        <div className="overflow-auto rounded-md border border-border-default">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border-subtle bg-surface-secondary">
                <th className="px-3 py-2 text-left font-medium text-text-secondary">{t('network.ip')}</th>
                <th className="px-3 py-2 text-left font-medium text-text-secondary">{t('network.hostname')}</th>
                <th className="px-3 py-2 text-left font-medium text-text-secondary">{t('network.os')}</th>
                <th className="px-3 py-2 text-left font-medium text-text-secondary">{t('network.openPorts')}</th>
                <th className="px-3 py-2 text-left font-medium text-text-secondary">{t('network.responseTime')}</th>
                <th className="px-3 py-2 text-left font-medium text-text-secondary">{t('network.sessionType')}</th>
                <th className="px-3 py-2 text-right font-medium text-text-secondary"></th>
              </tr>
            </thead>
            <tbody>
              {filteredResults.map((result) => (
                <tr key={result.ip} className="border-b border-border-subtle last:border-0 hover:bg-surface-secondary">
                  <td className="px-3 py-2 text-text-primary">{result.ip}</td>
                  <td className="px-3 py-2 text-text-secondary">{result.hostname ?? '—'}</td>
                  <td className="px-3 py-2 text-text-secondary">{result.os_guess ?? '—'}</td>
                  <td className="px-3 py-2">
                    <div className="flex flex-wrap gap-1">
                      {result.open_ports.map((p) => (
                        <span
                          key={p.port}
                          className={clsx(
                            'rounded bg-surface-elevated px-1.5 py-0.5 text-xs text-text-secondary',
                            SERVICE_PORT_COLORS[p.service_name] ?? ''
                          )}
                        >
                          {p.port}/{p.service_name}
                        </span>
                      ))}
                    </div>
                  </td>
                  <td className="px-3 py-2 text-text-secondary">{result.response_time_ms.toFixed(0)}ms</td>
                  <td className="px-3 py-2 text-text-secondary">{result.suggested_session_type ? SESSION_ICON[result.suggested_session_type] ?? result.suggested_session_type : '—'}</td>
                  <td className="px-3 py-2 text-right">
                    {result.suggested_session_type && (
                      <button
                        onClick={() => handleConnect(result)}
                        className="flex items-center gap-1 rounded px-2 py-1 text-xs font-medium text-accent-primary hover:bg-surface-elevated transition-colors"
                      >
                        <PlugZap size={12} />
                        {t('network.connect')}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Save all */}
      {results.length > 0 && !scanning && (
        <button
          onClick={handleSaveAsSessions}
          className="flex items-center gap-2 self-start rounded-md bg-surface-elevated px-3 py-1.5 text-xs text-text-primary hover:bg-surface-secondary transition-colors"
        >
          <Save size={12} />
          {t('network.saveAsSessions')}
        </button>
      )}

      {/* Empty state */}
      {!scanning && results.length === 0 && (
        <div className="flex flex-col items-center justify-center gap-3 py-8 text-center flex-1">
          <Radar size={32} className="text-text-disabled" />
          <p className="text-xs text-text-secondary px-4">
            {t('network.exploreEmptyState')}
          </p>
        </div>
      )}
    </div>
      )}
    </div>
  );
}
