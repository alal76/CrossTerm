import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
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
  Server,
  ChevronDown,
  ChevronUp,
  Filter,
  Wifi,
  ShieldAlert,
  History,
  CheckCircle2,
  AlertCircle,
  Clock,
  ArrowUp,
  ArrowDown,
  ArrowUpDown,
} from 'lucide-react';
import type { ExploreResult, ExploreProgress, ExploreHostFound, ServiceFilter, Session } from '@/types';
import { SessionType } from '@/types';
import { useSessionStore } from '@/stores/sessionStore';
import { useToast } from '@/components/Shared/Toast';
import WifiScanner from '@/components/NetworkTools/WifiScanner';
import AircrackPanel from '@/components/NetworkTools/AircrackPanel';

interface ConnectionAttempt {
  id: string;
  host: string;
  hostname: string | null;
  serviceType: string;
  status: 'connecting' | 'success' | 'failed';
  timestamp: number;
  error?: string;
}

interface LocalSubnet {
  interface: string;
  cidr: string;
  ip: string;
}

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

const SERVICE_DEFAULT_PORTS: Record<string, number> = {
  ssh: 22, sftp: 22, rdp: 3389, vnc: 5900, telnet: 23, ftp: 21,
};

const SESSION_TYPE_MAP: Record<string, SessionType> = {
  ssh: SessionType.SSH,
  sftp: SessionType.SFTP,
  rdp: SessionType.RDP,
  vnc: SessionType.VNC,
  telnet: SessionType.Telnet,
};

type SortKey = 'ip' | 'hostname' | 'ports' | 'response';

function ipToNum(ip: string): number {
  const parts = ip.split('.').map(Number);
  return ((parts[0] ?? 0) << 24) | ((parts[1] ?? 0) << 16) | ((parts[2] ?? 0) << 8) | (parts[3] ?? 0);
}

function SortIcon({ col, sortBy, sortDir }: { col: SortKey; sortBy: SortKey; sortDir: 'asc' | 'desc' }) {
  if (sortBy !== col) return <ArrowUpDown size={11} className="opacity-30" />;
  return sortDir === 'asc' ? <ArrowUp size={11} /> : <ArrowDown size={11} />;
}

export default function NetworkExplorer() {
  const { t } = useTranslation();
  const { addSession, openTab } = useSessionStore();
  const { toast } = useToast();
  const [toolTab, setToolTab] = useState<'explore' | 'wifi' | 'aircrack'>('explore');
  const [cidr, setCidr] = useState('');
  const [scanning, setScanning] = useState(false);
  const [results, setResults] = useState<ExploreResult[]>([]);
  const [progressMap, setProgressMap] = useState<Map<string, ExploreProgress>>(new Map());
  const [extraPortsInput, setExtraPortsInput] = useState('');
  const [selectedServices, setSelectedServices] = useState<Set<string>>(
    () => new Set(['ssh', 'rdp', 'vnc', 'http', 'https'])
  );
  const [showFilters, setShowFilters] = useState(false);
  const [filterService, setFilterService] = useState<string>('all');
  const [searchFilter, setSearchFilter] = useState('');
  const [sortBy, setSortBy] = useState<SortKey>('ip');
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('asc');
  const [saveCount, setSaveCount] = useState<number | null>(null);
  const [connectionHistory, setConnectionHistory] = useState<ConnectionAttempt[]>([]);
  const [showHistory, setShowHistory] = useState(false);

  // Set of currently active scan IDs (one per CIDR)
  const activeScanIdsRef = useRef<Set<string>>(new Set());

  // Auto-detect local subnets on mount
  useEffect(() => {
    invoke<LocalSubnet[]>('network_local_subnets')
      .then((subnets) => {
        if (Array.isArray(subnets) && subnets.length > 0) {
          setCidr(subnets.map((s) => s.cidr).join(', '));
        }
      })
      .catch(() => {}); // graceful degradation in browser/stub mode
  }, []);

  useEffect(() => {
    const unlistenHost = listen<ExploreHostFound>(
      'network:explore_host_found',
      (event) => {
        if (activeScanIdsRef.current.has(event.payload.scan_id)) {
          setResults((prev) => [...prev, event.payload.result]);
        }
      }
    );

    const unlistenProgress = listen<ExploreProgress>(
      'network:explore_progress',
      (event) => {
        if (!activeScanIdsRef.current.has(event.payload.scan_id)) return;
        setProgressMap((prev) => new Map(prev).set(event.payload.scan_id, event.payload));
        if (event.payload.hosts_scanned >= event.payload.total_hosts) {
          activeScanIdsRef.current.delete(event.payload.scan_id);
          if (activeScanIdsRef.current.size === 0) {
            setScanning(false);
          }
        }
      }
    );

    return () => {
      unlistenHost.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
    };
  }, []);

  // Aggregate progress across all active scans
  const aggregateProgress = useMemo(() => {
    if (progressMap.size === 0) return null;
    let total_hosts = 0, hosts_scanned = 0, hosts_found = 0;
    for (const p of progressMap.values()) {
      total_hosts += p.total_hosts;
      hosts_scanned += p.hosts_scanned;
      hosts_found += p.hosts_found;
    }
    return { scan_id: 'aggregate', total_hosts, hosts_scanned, hosts_found };
  }, [progressMap]);

  const toggleService = useCallback((svc: string) => {
    setSelectedServices((prev) => {
      const next = new Set(prev);
      if (next.has(svc)) { next.delete(svc); } else { next.add(svc); }
      return next;
    });
  }, []);

  const handleScan = useCallback(async () => {
    const cidrs = cidr.split(',').map((c) => c.trim()).filter(Boolean);
    if (cidrs.length === 0) return;

    setResults([]);
    setScanning(true);
    setProgressMap(new Map());
    activeScanIdsRef.current = new Set();

    const services: ServiceFilter[] = Array.from(selectedServices) as ServiceFilter[];
    const extra_ports = parseExtraPorts(extraPortsInput);

    let launched = 0;
    for (const c of cidrs) {
      try {
        const id = await invoke<string>('network_explore_start', {
          target: { cidr: c, services, extra_ports },
        });
        if (id) {
          activeScanIdsRef.current.add(id);
          launched++;
        }
      } catch {
        // skip invalid CIDRs
      }
    }

    if (launched === 0) {
      setScanning(false);
    }
  }, [cidr, selectedServices, extraPortsInput]);

  const handleConnect = useCallback(async (result: ExploreResult) => {
    const svcType = result.suggested_session_type;
    if (!svcType) {
      toast('warning', `No connectable service detected for ${result.hostname ?? result.ip}`);
      return;
    }
    const sessionType = SESSION_TYPE_MAP[svcType];
    if (!sessionType) {
      toast('info', `${svcType.toUpperCase()} is not directly connectable from CrossTerm`);
      return;
    }
    const port =
      result.open_ports.find((p) => p.service_name === svcType)?.port ??
      SERVICE_DEFAULT_PORTS[svcType] ??
      22;

    const attemptId = crypto.randomUUID();
    const attempt: ConnectionAttempt = {
      id: attemptId,
      host: result.ip,
      hostname: result.hostname ?? null,
      serviceType: svcType,
      status: 'connecting',
      timestamp: Date.now(),
    };
    setConnectionHistory((prev) => [attempt, ...prev.slice(0, 49)]);
    setShowHistory(true);

    // Build session locally — avoids backend "Profile not found" error
    const now = new Date().toISOString();
    const session: Session = {
      id: crypto.randomUUID(),
      name: `${result.hostname ?? result.ip} (${svcType.toUpperCase()})`,
      type: sessionType,
      group: 'Discovered',
      tags: [],
      connection: { host: result.ip, port },
      createdAt: now,
      updatedAt: now,
      autoReconnect: false,
      keepAliveIntervalSeconds: 0,
    };

    try {
      addSession(session);
      openTab(session);
      setConnectionHistory((prev) =>
        prev.map((a) => a.id === attemptId ? { ...a, status: 'success' } : a)
      );
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setConnectionHistory((prev) =>
        prev.map((a) => a.id === attemptId ? { ...a, status: 'failed', error: msg } : a)
      );
      toast('error', `Failed to open tab for ${result.hostname ?? result.ip}: ${msg}`);
    }
  }, [addSession, openTab, toast]);

  const handleSaveAsSessions = useCallback(async () => {
    const candidates = results.filter(
      (r) => r.suggested_session_type && SESSION_TYPE_MAP[r.suggested_session_type]
    );
    if (candidates.length === 0) return;
    setSaveCount(null);
    let saved = 0;
    const now = new Date().toISOString();
    for (const result of candidates) {
      const svcType = result.suggested_session_type!;
      const sessionType = SESSION_TYPE_MAP[svcType];
      const port =
        result.open_ports.find((p) => p.service_name === svcType)?.port ??
        SERVICE_DEFAULT_PORTS[svcType] ??
        22;
      const session: Session = {
        id: crypto.randomUUID(),
        name: `${result.hostname ?? result.ip} (${svcType.toUpperCase()})`,
        type: sessionType,
        group: 'Discovered Hosts',
        tags: [],
        connection: { host: result.ip, port },
        createdAt: now,
        updatedAt: now,
        autoReconnect: false,
        keepAliveIntervalSeconds: 0,
      };
      try {
        addSession(session);
        saved++;
      } catch {
        // skip
      }
    }
    setSaveCount(saved);
  }, [results, addSession]);

  // Toggle column sort — same column: flip direction; new column: set asc
  const handleSortColumn = useCallback((col: SortKey) => {
    setSortBy((prev) => {
      if (prev === col) {
        setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'));
        return prev;
      }
      setSortDir('asc');
      return col;
    });
  }, []);

  const filteredResults = useMemo(() => {
    let filtered = results;

    // Service filter
    if (filterService !== 'all') {
      filtered = filtered.filter((r) =>
        r.open_ports.some((p) => p.service_name === filterService)
      );
    }

    // Text search: IP or hostname
    if (searchFilter.trim()) {
      const q = searchFilter.toLowerCase();
      filtered = filtered.filter(
        (r) =>
          r.ip.includes(q) ||
          (r.hostname ?? '').toLowerCase().includes(q) ||
          r.open_ports.some((p) => p.service_name.includes(q))
      );
    }

    const sorted = [...filtered];
    const dir = sortDir === 'asc' ? 1 : -1;
    switch (sortBy) {
      case 'hostname':
        sorted.sort((a, b) => dir * (a.hostname ?? a.ip).localeCompare(b.hostname ?? b.ip));
        break;
      case 'ports':
        sorted.sort((a, b) => dir * (a.open_ports.length - b.open_ports.length));
        break;
      case 'response':
        sorted.sort((a, b) => dir * (a.response_time_ms - b.response_time_ms));
        break;
      default: // 'ip'
        sorted.sort((a, b) => dir * (ipToNum(a.ip) - ipToNum(b.ip)));
    }
    return sorted;
  }, [results, filterService, searchFilter, sortBy, sortDir]);

  const serviceCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const r of results) {
      for (const p of r.open_ports) {
        counts[p.service_name] = (counts[p.service_name] ?? 0) + 1;
      }
    }
    return counts;
  }, [results]);

  const cidrPlaceholder = 'e.g. 192.168.1.0/24, 10.0.0.0/8';

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

          {/* CIDR input row — supports comma-separated subnets */}
          <div className="flex gap-2">
            <input
              type="text"
              value={cidr}
              onChange={(e) => setCidr(e.target.value)}
              placeholder={cidrPlaceholder}
              className="flex-1 rounded-md border border-border-default bg-surface-primary px-3 py-1.5 text-sm text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
              onKeyDown={(e) => e.key === 'Enter' && handleScan()}
            />
            <button
              data-testid="scan-start-btn"
              onClick={handleScan}
              disabled={scanning || !cidr.trim()}
              className={clsx(
                'flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors shrink-0',
                scanning
                  ? 'cursor-not-allowed bg-interactive-disabled text-text-disabled'
                  : 'bg-interactive-default text-text-inverse hover:bg-interactive-hover'
              )}
            >
              {scanning ? <Loader2 size={14} className="animate-spin" /> : <Search size={14} />}
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
          {scanning && aggregateProgress && aggregateProgress.total_hosts > 0 && (
            <div className="flex flex-col gap-1">
              <div className="flex items-center gap-2">
                <div className="h-1.5 flex-1 rounded-full bg-surface-sunken">
                  <div
                    className="h-full rounded-full bg-accent-primary transition-all"
                    style={{ width: `${(aggregateProgress.hosts_scanned / aggregateProgress.total_hosts) * 100}%` }}
                  />
                </div>
                <span className="text-xs text-text-secondary tabular-nums">
                  {aggregateProgress.hosts_scanned}/{aggregateProgress.total_hosts}
                </span>
              </div>
              <span className="text-xs text-text-secondary">
                {t('network.hostsFound', { count: aggregateProgress.hosts_found })}
                {activeScanIdsRef.current.size > 1 && (
                  <span className="ml-1 text-text-disabled">({activeScanIdsRef.current.size} subnets)</span>
                )}
              </span>
            </div>
          )}

          {/* Summary badges + search */}
          {results.length > 0 && (
            <div className="flex flex-col gap-2">
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
              {/* Search input */}
              <div className="relative">
                <Search size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-text-disabled pointer-events-none" />
                <input
                  type="text"
                  value={searchFilter}
                  onChange={(e) => setSearchFilter(e.target.value)}
                  placeholder="Filter by IP, hostname or service…"
                  className="w-full rounded border border-border-subtle bg-surface-primary pl-7 pr-3 py-1.5 text-xs text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
                />
                {searchFilter && (
                  <button
                    onClick={() => setSearchFilter('')}
                    className="absolute right-2 top-1/2 -translate-y-1/2 text-text-disabled hover:text-text-primary"
                  >
                    <X size={11} />
                  </button>
                )}
              </div>
            </div>
          )}

          {/* Results table */}
          {filteredResults.length > 0 && (
            <div className="overflow-auto rounded-md border border-border-default">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-border-subtle bg-surface-secondary">
                    {(
                      [
                        { key: 'ip' as SortKey, label: t('network.ip') },
                        { key: 'hostname' as SortKey, label: t('network.hostname') },
                        { key: null, label: t('network.os') },
                        { key: 'ports' as SortKey, label: t('network.openPorts') },
                        { key: 'response' as SortKey, label: t('network.responseTime') },
                        { key: null, label: t('network.sessionType') },
                        { key: null, label: '' },
                      ] as { key: SortKey | null; label: string }[]
                    ).map((col, i) => (
                      <th
                        key={i}
                        className={clsx(
                          'px-3 py-2 text-left font-medium text-text-secondary select-none',
                          col.key && 'cursor-pointer hover:text-text-primary transition-colors'
                        )}
                        onClick={col.key ? () => handleSortColumn(col.key!) : undefined}
                      >
                        <span className="flex items-center gap-1">
                          {col.label}
                          {col.key && <SortIcon col={col.key} sortBy={sortBy} sortDir={sortDir} />}
                        </span>
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {filteredResults.map((result) => (
                    <tr key={result.ip} className="border-b border-border-subtle last:border-0 hover:bg-surface-secondary">
                      <td className="px-3 py-2 text-text-primary font-mono text-xs">{result.ip}</td>
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
                      <td className="px-3 py-2 text-text-secondary">
                        {result.suggested_session_type
                          ? SESSION_ICON[result.suggested_session_type] ?? result.suggested_session_type
                          : '—'}
                      </td>
                      <td className="px-3 py-2 text-right">
                        {result.suggested_session_type && SESSION_TYPE_MAP[result.suggested_session_type] && (
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
            <div className="flex items-center gap-3">
              <button
                onClick={handleSaveAsSessions}
                className="flex items-center gap-2 rounded-md bg-surface-elevated px-3 py-1.5 text-xs text-text-primary hover:bg-surface-secondary transition-colors"
              >
                <Save size={12} />
                {t('network.saveAsSessions')}
              </button>
              {saveCount !== null && (
                <span className="text-xs text-text-secondary">
                  {saveCount} {saveCount === 1 ? 'session' : 'sessions'} saved
                </span>
              )}
            </div>
          )}

          {/* Connection history */}
          {connectionHistory.length > 0 && (
            <div className="flex flex-col gap-1.5 rounded-md border border-border-subtle bg-surface-secondary p-3">
              <button
                onClick={() => setShowHistory((v) => !v)}
                className="flex items-center gap-1.5 text-xs font-medium text-text-secondary hover:text-text-primary transition-colors"
              >
                <History size={12} />
                Connection History ({connectionHistory.length})
                {showHistory ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
              </button>
              {showHistory && (
                <div className="flex flex-col gap-1 mt-1">
                  {connectionHistory.map((attempt) => {
                    const label = attempt.hostname ?? attempt.host;
                    let statusIcon;
                    if (attempt.status === 'success') {
                      statusIcon = <CheckCircle2 size={12} className="text-status-connected shrink-0" />;
                    } else if (attempt.status === 'failed') {
                      statusIcon = <AlertCircle size={12} className="text-status-disconnected shrink-0" />;
                    } else {
                      statusIcon = <Clock size={12} className="text-text-disabled shrink-0 animate-pulse" />;
                    }
                    const time = new Date(attempt.timestamp).toLocaleTimeString([], {
                      hour: '2-digit', minute: '2-digit', second: '2-digit',
                    });
                    return (
                      <div key={attempt.id} className="flex items-start gap-2 rounded px-2 py-1.5 hover:bg-surface-primary text-xs">
                        {statusIcon}
                        <div className="flex-1 min-w-0">
                          <span className="text-text-primary">{label}</span>
                          <span className="text-text-disabled mx-1">·</span>
                          <span className="text-text-secondary uppercase">{attempt.serviceType}</span>
                          {attempt.error && (
                            <p className="text-status-disconnected truncate mt-0.5">{attempt.error}</p>
                          )}
                        </div>
                        <span className="text-text-disabled tabular-nums shrink-0">{time}</span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
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
