import { useState, useCallback, useEffect, useRef, useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import {
  Wifi,
  WifiOff,
  RefreshCw,
  Loader2,
  Shield,
  ShieldAlert,
  ShieldCheck,
  ShieldX,
  BarChart3,
  AlertTriangle,
  Info,
  CheckCircle2,
  Signal,
  SignalLow,
  SignalMedium,
  SignalHigh,
  SignalZero,
} from 'lucide-react';
import type {
  WifiScanResult,
  WifiNetwork,
  WifiSecurityIssue,
  WifiChannelCongestion,
  WifiBand,
  WifiSecurity,
} from '@/types';

type TabId = 'networks' | 'channels' | 'security';

function signalQuality(dbm: number | undefined): { label: string; color: string; icon: typeof Signal } {
  if (dbm === undefined) return { label: 'wifiNoSignal', color: 'text-text-disabled', icon: SignalZero };
  if (dbm >= -50) return { label: 'wifiExcellent', color: 'text-status-connected', icon: SignalHigh };
  if (dbm >= -60) return { label: 'wifiGood', color: 'text-status-connected', icon: SignalHigh };
  if (dbm >= -70) return { label: 'wifiFair', color: 'text-status-warning', icon: SignalMedium };
  if (dbm >= -80) return { label: 'wifiWeak', color: 'text-status-warning', icon: SignalLow };
  return { label: 'wifiDeadSpot', color: 'text-status-error', icon: SignalZero };
}

function securityLabel(sec: WifiSecurity): string {
  if (typeof sec === 'object' && 'unknown' in sec) return sec.unknown;
  const map: Record<string, string> = {
    open: 'Open',
    wep: 'WEP',
    wpa_psk: 'WPA-PSK',
    wpa2_psk: 'WPA2-PSK',
    wpa3_sae: 'WPA3-SAE',
    wpa3_transition: 'WPA3 Transition',
    wpa2_enterprise: 'WPA2 Enterprise',
    wpa3_enterprise: 'WPA3 Enterprise',
  };
  return map[sec as string] ?? String(sec);
}

function securityBadgeColor(sec: WifiSecurity): string {
  if (typeof sec === 'object') return 'bg-surface-elevated text-text-secondary';
  switch (sec) {
    case 'open': return 'bg-red-500/20 text-red-400';
    case 'wep': return 'bg-red-500/20 text-red-400';
    case 'wpa_psk': return 'bg-orange-500/20 text-orange-400';
    case 'wpa2_psk': return 'bg-blue-500/20 text-blue-300';
    case 'wpa2_enterprise': return 'bg-blue-500/20 text-blue-300';
    case 'wpa3_sae':
    case 'wpa3_transition':
    case 'wpa3_enterprise':
      return 'bg-green-500/20 text-green-400';
    default: return 'bg-surface-elevated text-text-secondary';
  }
}

function bandLabel(band: WifiBand): string {
  const map: Record<WifiBand, string> = {
    '2.4GHz': '2.4 GHz',
    '5GHz': '5 GHz',
    '6GHz': '6 GHz',
    unknown: '?',
  };
  return map[band];
}

function congestionColor(level: string): string {
  switch (level) {
    case 'low': return 'bg-green-500/20 text-green-400';
    case 'medium': return 'bg-yellow-500/20 text-yellow-400';
    case 'high': return 'bg-red-500/20 text-red-400';
    default: return 'bg-surface-elevated text-text-secondary';
  }
}

function severityIcon(severity: string) {
  switch (severity) {
    case 'critical': return <ShieldX size={16} className="text-red-400" />;
    case 'high': return <ShieldAlert size={16} className="text-orange-400" />;
    case 'warning': return <AlertTriangle size={16} className="text-yellow-400" />;
    case 'info': return <Info size={16} className="text-blue-400" />;
    default: return <Info size={16} className="text-text-secondary" />;
  }
}

function severityBadgeColor(severity: string): string {
  switch (severity) {
    case 'critical': return 'bg-red-500/20 text-red-400';
    case 'high': return 'bg-orange-500/20 text-orange-400';
    case 'warning': return 'bg-yellow-500/20 text-yellow-400';
    case 'info': return 'bg-blue-500/20 text-blue-300';
    default: return 'bg-surface-elevated text-text-secondary';
  }
}

// Simple bar chart for channel congestion
function ChannelBar({ item, maxCount }: { item: WifiChannelCongestion; maxCount: number }) {
  const pct = maxCount > 0 ? (item.network_count / maxCount) * 100 : 0;
  return (
    <div className="flex items-end gap-1" style={{ minWidth: 32 }}>
      <div className="flex w-full flex-col items-center gap-1">
        <span className="text-[10px] text-text-secondary">{item.network_count}</span>
        <div
          className={clsx(
            'w-5 rounded-t',
            item.congestion_level === 'high' ? 'bg-red-500' :
            item.congestion_level === 'medium' ? 'bg-yellow-500' : 'bg-green-500'
          )}
          style={{ height: Math.max(4, (pct / 100) * 80) }}
        />
        <span className="text-[10px] font-medium text-text-primary">{item.channel}</span>
      </div>
    </div>
  );
}

export default function WifiScanner() {
  const { t } = useTranslation();
  const [scanning, setScanning] = useState(false);
  const [result, setResult] = useState<WifiScanResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<TabId>('networks');
  const [bandFilter, setBandFilter] = useState<WifiBand | 'all'>('all');
  const [sortBy, setSortBy] = useState<'signal' | 'channel' | 'ssid'>('signal');
  const [autoRefresh, setAutoRefresh] = useState(false);
  const autoRefreshRef = useRef(false);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const doScan = useCallback(async () => {
    setScanning(true);
    setError(null);
    try {
      const data = await invoke<WifiScanResult>('network_wifi_scan');
      setResult(data);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      setError(msg);
    } finally {
      setScanning(false);
    }
  }, []);

  // Auto-refresh
  useEffect(() => {
    autoRefreshRef.current = autoRefresh;
    if (autoRefresh) {
      doScan();
      timerRef.current = setInterval(() => {
        if (autoRefreshRef.current) doScan();
      }, 10000);
    }
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [autoRefresh, doScan]);

  const filteredNetworks = useMemo(() => {
    if (!result) return [];
    let nets = result.networks;
    if (bandFilter !== 'all') {
      nets = nets.filter((n) => n.band === bandFilter);
    }
    const sorted = [...nets];
    switch (sortBy) {
      case 'signal':
        sorted.sort((a, b) => (b.signal_dbm ?? -999) - (a.signal_dbm ?? -999));
        break;
      case 'channel':
        sorted.sort((a, b) => a.channel - b.channel);
        break;
      case 'ssid':
        sorted.sort((a, b) => a.ssid.localeCompare(b.ssid));
        break;
    }
    return sorted;
  }, [result, bandFilter, sortBy]);

  const filteredCongestion = useMemo(() => {
    if (!result) return [];
    if (bandFilter === 'all') return result.channel_congestion;
    return result.channel_congestion.filter((c) => c.band === bandFilter);
  }, [result, bandFilter]);

  const filteredIssues = useMemo(() => {
    if (!result) return [];
    return result.security_issues;
  }, [result]);

  const criticalCount = filteredIssues.filter((i) => i.severity === 'critical').length;
  const highCount = filteredIssues.filter((i) => i.severity === 'high').length;

  const tabs: { id: TabId; label: string; icon: typeof Wifi; badge?: number }[] = [
    { id: 'networks', label: t('network.wifiNetworks'), icon: Wifi, badge: filteredNetworks.length },
    { id: 'channels', label: t('network.wifiChannels'), icon: BarChart3 },
    { id: 'security', label: t('network.wifiSecurity'), icon: Shield, badge: criticalCount + highCount || undefined },
  ];

  return (
    <div className="flex h-full flex-col gap-3 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Wifi size={20} className="text-text-secondary" />
          <h2 className="text-base font-semibold text-text-primary">{t('network.wifi')}</h2>
          {result?.interface_name && (
            <span className="rounded bg-surface-elevated px-2 py-0.5 text-[11px] text-text-secondary">
              {result.interface_name}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <label className="flex items-center gap-1.5 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={autoRefresh}
              onChange={(e) => setAutoRefresh(e.target.checked)}
              className="accent-interactive-default"
            />
            {t('network.wifiAutoRefresh')}
          </label>
          <button
            onClick={doScan}
            disabled={scanning}
            className="flex items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1.5 text-xs font-medium text-text-inverse hover:bg-interactive-hover disabled:cursor-not-allowed disabled:bg-interactive-disabled disabled:text-text-disabled"
          >
            {scanning ? <Loader2 size={14} className="animate-spin" /> : <RefreshCw size={14} />}
            {scanning ? t('network.wifiScanning') : t('network.wifiScan')}
          </button>
        </div>
      </div>

      {/* Current Network Summary */}
      {result?.current_network && (
        <div className="flex items-center gap-3 rounded-lg border border-border-default bg-surface-elevated px-3 py-2">
          <CheckCircle2 size={16} className="text-status-connected" />
          <div className="text-xs">
            <span className="font-medium text-text-primary">{result.current_network.ssid}</span>
            <span className="ml-2 text-text-secondary">
              Ch {result.current_network.channel} · {bandLabel(result.current_network.band)} · {securityLabel(result.current_network.security)}
              {result.current_network.signal_dbm !== undefined && ` · ${result.current_network.signal_dbm} dBm`}
            </span>
          </div>
        </div>
      )}

      {error && (
        <div className="rounded-md border border-status-error bg-red-500/10 px-3 py-2 text-xs text-status-error">
          {error}
        </div>
      )}

      {/* Tabs */}
      <div className="flex items-center gap-1 border-b border-border-subtle">
        {tabs.map(({ id, label, icon: Icon, badge }) => (
          <button
            key={id}
            onClick={() => setActiveTab(id)}
            className={clsx(
              'flex items-center gap-1.5 border-b-2 px-3 py-1.5 text-xs font-medium transition-colors',
              activeTab === id
                ? 'border-interactive-default text-text-primary'
                : 'border-transparent text-text-secondary hover:text-text-primary'
            )}
          >
            <Icon size={14} />
            {label}
            {badge !== undefined && badge > 0 && (
              <span className={clsx(
                'ml-1 rounded-full px-1.5 py-0.5 text-[10px] font-medium',
                id === 'security' ? 'bg-red-500/20 text-red-400' : 'bg-surface-elevated text-text-secondary'
              )}>
                {badge}
              </span>
            )}
          </button>
        ))}

        {/* Band Filter */}
        <div className="ml-auto flex items-center gap-1">
          {(['all', '2.4GHz', '5GHz', '6GHz'] as const).map((b) => (
            <button
              key={b}
              onClick={() => setBandFilter(b)}
              className={clsx(
                'rounded px-2 py-0.5 text-[10px] font-medium transition-colors',
                bandFilter === b
                  ? 'bg-interactive-default text-text-inverse'
                  : 'bg-surface-elevated text-text-secondary hover:text-text-primary'
              )}
            >
              {b === 'all' ? 'All' : bandLabel(b)}
            </button>
          ))}
        </div>
      </div>

      {/* Content */}
      {!result && !scanning && (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 text-text-secondary">
          <WifiOff size={32} />
          <p className="text-sm">{t('network.wifiNoNetworks')}</p>
          <button
            onClick={doScan}
            className="mt-2 rounded-md bg-interactive-default px-4 py-2 text-sm font-medium text-text-inverse hover:bg-interactive-hover"
          >
            {t('network.wifiScan')}
          </button>
        </div>
      )}

      {scanning && !result && (
        <div className="flex flex-1 items-center justify-center gap-2 text-text-secondary">
          <Loader2 size={20} className="animate-spin" />
          <span className="text-sm">{t('network.wifiScanning')}</span>
        </div>
      )}

      {result && activeTab === 'networks' && (
        <NetworksTab
          networks={filteredNetworks}
          sortBy={sortBy}
          onSortChange={setSortBy}
          t={t}
        />
      )}

      {result && activeTab === 'channels' && (
        <ChannelsTab
          congestion={filteredCongestion}
          recommended2g={result.recommended_channels_2g}
          recommended5g={result.recommended_channels_5g}
          t={t}
        />
      )}

      {result && activeTab === 'security' && (
        <SecurityTab issues={filteredIssues} t={t} />
      )}
    </div>
  );
}

// ── Networks Tab ────────────────────────────────────────────────────────

function NetworksTab({
  networks,
  sortBy,
  onSortChange,
  t,
}: {
  networks: WifiNetwork[];
  sortBy: string;
  onSortChange: (s: 'signal' | 'channel' | 'ssid') => void;
  t: (key: string) => string;
}) {
  const [details, setDetails] = useState<Record<string, unknown> | { error?: string } | null>(null);
  const [detailsOpen, setDetailsOpen] = useState(false);
  const [detailsLoading, setDetailsLoading] = useState(false);
  const handleShowDetails = async (net: WifiNetwork) => {
    setDetailsLoading(true);
    setDetailsOpen(true);
    try {
      const res = await invoke('network_analyze_wifi_details', {
        ssid: net.ssid,
        bssid: net.bssid ?? '',
        channelRaw: String(net.channel),
        signalNoiseRaw: typeof net.noise_dbm === 'number' ? `${net.signal_dbm ?? ''} / ${net.noise_dbm}` : undefined,
        securityRaw: typeof net.security === 'string' ? net.security : undefined,
      });
      setDetails(res);
    } catch (e) {
      setDetails({ error: String(e) });
    } finally {
      setDetailsLoading(false);
    }
  };
  return (
    <div className="flex flex-1 flex-col gap-2 overflow-y-auto">
      {/* Sort controls */}
      <div className="flex items-center gap-2 text-xs text-text-secondary">
        <span>{t('network.sortBy')}:</span>
        {(['signal', 'channel', 'ssid'] as const).map((s) => (
          <button
            key={s}
            onClick={() => onSortChange(s)}
            className={clsx(
              'rounded px-2 py-0.5 text-[11px]',
              sortBy === s
                ? 'bg-interactive-default text-text-inverse'
                : 'bg-surface-elevated hover:text-text-primary'
            )}
          >
            {(() => {
              if (s === 'signal') return t('network.wifiSignal');
              if (s === 'channel') return t('network.wifiChannel');
              return t('network.wifiSSID');
            })()}
          </button>
        ))}
      </div>

      {/* Network list */}
      <div className="flex flex-col gap-1">
        {networks.map((net, i) => {
          const sq = signalQuality(net.signal_dbm);
          const SigIcon = sq.icon;
          return (
            <div
              key={`${net.ssid}-${net.bssid ?? ''}-${i}`}
              className={clsx(
                'flex items-center gap-3 rounded-lg border px-3 py-2',
                net.is_current
                  ? 'border-status-connected bg-green-500/5'
                  : 'border-border-subtle bg-surface-primary'
              )}
            >
              {/* Signal indicator */}
              <div className="flex flex-col items-center gap-0.5" style={{ minWidth: 36 }}>
                <SigIcon size={16} className={sq.color} />
                <span className={clsx('text-[10px] font-medium', sq.color)}>
                  {typeof net.signal_dbm === 'number' ? `${net.signal_dbm}` : '—'}
                </span>
              </div>

              {/* Network info */}
              <div className="flex flex-1 flex-col gap-0.5 overflow-hidden">
                <div className="flex items-center gap-2">
                  <span className="truncate text-sm font-medium text-text-primary">
                    {net.ssid || '(hidden)'}
                  </span>
                  {net.is_current && (
                    <span className="rounded bg-green-500/20 px-1.5 py-0.5 text-[10px] font-medium text-green-400">
                      {t('network.wifiCurrentNetwork')}
                    </span>
                  )}
                  <button
                    className="ml-2 rounded bg-surface-elevated px-2 py-0.5 text-[10px] text-text-secondary hover:bg-surface-overlay"
                    onClick={() => handleShowDetails(net)}
                  >
                    {t('network.wifiAdvancedDetails')}
                  </button>
                </div>
                <div className="flex items-center gap-2 text-[11px] text-text-secondary">
                  <span>Ch {net.channel}</span>
                  <span>·</span>
                  <span>{bandLabel(net.band)}</span>
                  {net.channel_width_mhz && (
                    <>
                      <span>·</span>
                      <span>{net.channel_width_mhz} MHz</span>
                    </>
                  )}
                  {net.phy_mode && (
                    <>
                      <span>·</span>
                      <span>{net.phy_mode}</span>
                    </>
                  )}
                  {net.noise_dbm !== undefined && (
                    <>
                      <span>·</span>
                      <span>Noise: {net.noise_dbm} dBm</span>
                    </>
                  )}
                </div>
              </div>

              {/* Security badge */}
              <span className={clsx('rounded px-2 py-0.5 text-[10px] font-medium', securityBadgeColor(net.security))}>
                {securityLabel(net.security)}
              </span>
            </div>
          );
        })}
      </div>

      {/* Advanced Details Modal */}
      {detailsOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
          <div className="rounded-lg bg-surface-primary p-6 shadow-xl min-w-[320px] max-w-[90vw]">
            <div className="flex items-center justify-between mb-2">
              <h3 className="text-base font-semibold text-text-primary">{t('network.wifiAdvancedDetails')}</h3>
              <button onClick={() => setDetailsOpen(false)} className="text-text-secondary hover:text-text-primary">✕</button>
            </div>
            {detailsLoading && (
              <div className="flex items-center gap-2 text-text-secondary"><Loader2 size={16} className="animate-spin" /> {t('loading')}</div>
            )}
            {!detailsLoading && details && !details.error && (
              <pre className="text-xs text-text-primary whitespace-pre-wrap break-all">{JSON.stringify(details, null, 2)}</pre>
            )}
            {!detailsLoading && details?.error && (
              <div className="text-xs text-status-error">{details.error || t('error')}</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

// ── Channels Tab ────────────────────────────────────────────────────────

function ChannelsTab({
  congestion,
  recommended2g,
  recommended5g,
  t,
}: Readonly<{
  congestion: WifiChannelCongestion[];
  recommended2g: number[];
  recommended5g: number[];
  t: (key: string) => string;
}>) {
  const maxCount = Math.max(...congestion.map((c) => c.network_count), 1);

  const band24 = congestion.filter((c) => c.band === '2.4GHz');
  const band5 = congestion.filter((c) => c.band === '5GHz');
  const band6 = congestion.filter((c) => c.band === '6GHz');

  return (
    <div className="flex flex-1 flex-col gap-4 overflow-y-auto">
      {/* 2.4 GHz */}
      {band24.length > 0 && (
        <ChannelBandSection
          title={t('network.wifi2GHz')}
          channels={band24}
          maxCount={maxCount}
          recommended={recommended2g}
          t={t}
        />
      )}

      {/* 5 GHz */}
      {band5.length > 0 && (
        <ChannelBandSection
          title={t('network.wifi5GHz')}
          channels={band5}
          maxCount={maxCount}
          recommended={recommended5g}
          t={t}
        />
      )}

      {/* 6 GHz */}
      {band6.length > 0 && (
        <ChannelBandSection
          title={t('network.wifi6GHz')}
          channels={band6}
          maxCount={maxCount}
          recommended={[]}
          t={t}
        />
      )}

      {congestion.length === 0 && (
        <div className="flex flex-1 items-center justify-center text-sm text-text-secondary">
          No channel data available
        </div>
      )}
    </div>
  );
}

function ChannelBandSection({
  title,
  channels,
  maxCount,
  recommended,
  t,
}: Readonly<{
  title: string;
  channels: WifiChannelCongestion[];
  maxCount: number;
  recommended: number[];
  t: (key: string) => string;
}>) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-text-primary">{title}</h3>
        {recommended.length > 0 && (
          <span className="text-[10px] text-text-secondary">
            {t('network.wifiRecommendedChannels')}: {recommended.join(', ')}
          </span>
        )}
      </div>

      {/* Bar chart */}
      <div className="flex items-end gap-0.5 rounded-lg border border-border-subtle bg-surface-primary px-3 py-2" style={{ minHeight: 120 }}>
        {channels.map((item) => (
          <ChannelBar key={item.channel} item={item} maxCount={maxCount} />
        ))}
      </div>

      {/* Congestion table */}
      <div className="grid grid-cols-[auto_1fr_auto_auto] gap-x-4 gap-y-1 text-[11px]">
        <span className="font-medium text-text-secondary">{t('network.wifiChannel')}</span>
        <span className="font-medium text-text-secondary">{t('network.wifiBand')}</span>
        <span className="font-medium text-text-secondary">#</span>
        <span className="font-medium text-text-secondary">Level</span>
        {channels.map((c) => (
          <div key={c.channel} className="contents">
            <span className="text-text-primary">{c.channel}</span>
            <span className="text-text-secondary">{bandLabel(c.band)}</span>
            <span className="text-text-primary">{c.network_count}</span>
            <span className={clsx('rounded px-1.5 py-0.5 text-center text-[10px] font-medium', congestionColor(c.congestion_level))}>
              {c.congestion_level}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Security Tab ────────────────────────────────────────────────────────

function SecurityTab({
  issues,
  t,
}: Readonly<{
  issues: WifiSecurityIssue[];
  t: (key: string) => string;
}>) {
  const grouped = useMemo(() => {
    const groups: Record<string, WifiSecurityIssue[]> = {
      critical: [],
      high: [],
      warning: [],
      info: [],
    };
    for (const issue of issues) {
      (groups[issue.severity] ?? groups.info).push(issue);
    }
    return groups;
  }, [issues]);

  if (issues.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-2 text-text-secondary">
        <ShieldCheck size={32} className="text-status-connected" />
        <p className="text-sm">No security issues detected</p>
      </div>
    );
  }

  return (
    <div className="flex flex-1 flex-col gap-3 overflow-y-auto">
      {/* Summary badges */}
      <div className="flex items-center gap-2">
        {grouped.critical.length > 0 && (
          <span className="flex items-center gap-1 rounded bg-red-500/20 px-2 py-1 text-xs font-medium text-red-400">
            <ShieldX size={12} /> {grouped.critical.length} {t('network.wifiSecurityCritical')}
          </span>
        )}
        {grouped.high.length > 0 && (
          <span className="flex items-center gap-1 rounded bg-orange-500/20 px-2 py-1 text-xs font-medium text-orange-400">
            <ShieldAlert size={12} /> {grouped.high.length} {t('network.wifiSecurityHigh')}
          </span>
        )}
        {grouped.warning.length > 0 && (
          <span className="flex items-center gap-1 rounded bg-yellow-500/20 px-2 py-1 text-xs font-medium text-yellow-400">
            <AlertTriangle size={12} /> {grouped.warning.length} {t('network.wifiSecurityWarning')}
          </span>
        )}
        {grouped.info.length > 0 && (
          <span className="flex items-center gap-1 rounded bg-blue-500/20 px-2 py-1 text-xs font-medium text-blue-300">
            <Info size={12} /> {grouped.info.length} {t('network.wifiSecurityInfo')}
          </span>
        )}
      </div>

      {/* Issue cards */}
      {issues.map((issue, i) => (
        <div
          key={`${issue.ssid}-${i}`}
          className="flex gap-3 rounded-lg border border-border-subtle bg-surface-primary px-3 py-2"
        >
          <div className="mt-0.5">{severityIcon(issue.severity)}</div>
          <div className="flex flex-1 flex-col gap-1">
            <div className="flex items-center gap-2">
              <span className="text-sm font-medium text-text-primary">{issue.ssid}</span>
              <span className={clsx('rounded px-1.5 py-0.5 text-[10px] font-medium', severityBadgeColor(issue.severity))}>
                {issue.severity}
              </span>
            </div>
            <p className="text-xs text-text-secondary">{issue.issue}</p>
            <p className="text-xs text-text-secondary">
              <span className="font-medium text-text-primary">Fix: </span>
              {issue.recommendation}
            </p>
          </div>
        </div>
      ))}
    </div>
  );
}
