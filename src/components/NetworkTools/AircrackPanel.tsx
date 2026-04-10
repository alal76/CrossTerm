import { useState, useCallback, useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { clsx } from 'clsx';
import {
  ShieldAlert,
  ShieldCheck,
  ShieldX,
  Wifi,
  WifiOff,
  Radio,
  Loader2,
  AlertTriangle,
  CheckCircle2,
  XCircle,
  Play,
  Square,
  FileText,
  Zap,
  Lock,
  Unlock,
  Eye,
  Users,
  Activity,
  List,
} from 'lucide-react';
import type {
  AircrackToolStatus,
  WirelessInterface,
  AirodumpResult,
  HandshakeCaptureStatus,
  CrackProgress,
  AircrackAuditEntry,
} from '@/types';

type AircrackTab = 'interfaces' | 'scan' | 'test' | 'log';

// ── Disclaimer Modal ────────────────────────────────────────────────────

function DisclaimerModal({
  onAccept,
  onDecline,
  t,
}: Readonly<{
  onAccept: () => void;
  onDecline: () => void;
  t: (key: string) => string;
}>) {
  const [checks, setChecks] = useState([false, false, false, false]);
  const allChecked = checks.every(Boolean);

  const toggle = (i: number) =>
    setChecks((prev) => prev.map((v, idx) => (idx === i ? !v : v)));

  const bullets = [
    t('network.aircrackDisclaimerBullet1'),
    t('network.aircrackDisclaimerBullet2'),
    t('network.aircrackDisclaimerBullet3'),
    t('network.aircrackDisclaimerBullet4'),
  ];

  return (
    <div className="absolute inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="mx-4 flex max-w-lg flex-col gap-4 rounded-xl border-2 border-red-500/50 bg-surface-primary p-6 shadow-2xl">
        {/* Header */}
        <div className="flex items-center gap-3">
          <ShieldAlert size={28} className="text-red-400" />
          <h2 className="text-lg font-bold text-red-400">
            {t('network.aircrackDisclaimerTitle')}
          </h2>
        </div>

        {/* Body */}
        <p className="whitespace-pre-line text-sm leading-relaxed text-text-secondary">
          {t('network.aircrackDisclaimerBody')}
        </p>

        {/* Checkboxes */}
        <div className="flex flex-col gap-2 rounded-lg border border-red-500/30 bg-red-500/5 p-3">
          {bullets.map((text, i) => (
            <label key={i} className="flex cursor-pointer items-start gap-2 text-sm text-text-primary">
              <input
                type="checkbox"
                checked={checks[i]}
                onChange={() => toggle(i)}
                className="mt-0.5 accent-red-500"
              />
              <span>{text}</span>
            </label>
          ))}
        </div>

        {/* Actions */}
        <div className="flex items-center justify-end gap-3">
          <button
            onClick={onDecline}
            className="rounded-md px-4 py-2 text-sm font-medium text-text-secondary hover:text-text-primary"
          >
            {t('network.aircrackDecline')}
          </button>
          <button
            onClick={onAccept}
            disabled={!allChecked}
            className={clsx(
              'rounded-md px-4 py-2 text-sm font-bold transition-colors',
              allChecked
                ? 'bg-red-600 text-white hover:bg-red-700'
                : 'cursor-not-allowed bg-surface-elevated text-text-disabled'
            )}
          >
            {t('network.aircrackAcceptDisclaimer')}
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Main Panel ──────────────────────────────────────────────────────────

export default function AircrackPanel() {
  const { t } = useTranslation();
  const [disclaimerAccepted, setDisclaimerAccepted] = useState(false);
  const [toolStatus, setToolStatus] = useState<AircrackToolStatus | null>(null);
  const [activeTab, setActiveTab] = useState<AircrackTab>('interfaces');

  // Check tool availability on mount
  useEffect(() => {
    invoke<AircrackToolStatus>('network_aircrack_check')
      .then(setToolStatus)
      .catch(() => {/* silently fail */});
  }, []);

  const handleAccept = useCallback(async () => {
    try {
      await invoke('network_aircrack_accept_disclaimer');
      setDisclaimerAccepted(true);
    } catch {
      // handle error
    }
  }, []);

  const handleDecline = useCallback(() => {
    // Just stay on the panel without accepting
  }, []);

  // Not installed
  if (toolStatus && !toolStatus.aircrack_ng) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-8 text-center">
        <WifiOff size={40} className="text-text-disabled" />
        <h3 className="text-sm font-semibold text-text-primary">{t('network.aircrack')}</h3>
        <p className="max-w-md text-xs text-text-secondary">{t('network.aircrackNotInstalled')}</p>
        <code className="rounded bg-surface-elevated px-3 py-1.5 text-xs text-text-secondary">
          {navigator.platform.includes('Mac')
            ? 'brew install aircrack-ng'
            : 'sudo apt install aircrack-ng'}
        </code>
      </div>
    );
  }

  // Disclaimer gate
  if (!disclaimerAccepted) {
    return (
      <div className="relative h-full">
        {/* Background info */}
        <div className="flex h-full flex-col items-center justify-center gap-3 p-8 opacity-30">
          <ShieldAlert size={48} className="text-text-disabled" />
          <h3 className="text-base font-semibold text-text-primary">{t('network.aircrack')}</h3>
          <p className="text-xs text-text-secondary">{t('network.aircrackSubtitle')}</p>
        </div>
        <DisclaimerModal onAccept={handleAccept} onDecline={handleDecline} t={t} />
      </div>
    );
  }

  const tabs: { id: AircrackTab; label: string; icon: typeof Wifi }[] = [
    { id: 'interfaces', label: t('network.aircrackTabInterfaces'), icon: Radio },
    { id: 'scan', label: t('network.aircrackTabScan'), icon: Eye },
    { id: 'test', label: t('network.aircrackTabAttack'), icon: Zap },
    { id: 'log', label: t('network.aircrackTabLog'), icon: FileText },
  ];

  return (
    <div className="flex h-full flex-col gap-3 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <ShieldAlert size={20} className="text-red-400" />
          <h2 className="text-base font-semibold text-text-primary">{t('network.aircrack')}</h2>
          {toolStatus?.version && (
            <span className="rounded bg-surface-elevated px-2 py-0.5 text-[10px] text-text-secondary">
              {toolStatus.version}
            </span>
          )}
        </div>
        {toolStatus?.needs_root && (
          <span className="flex items-center gap-1 rounded bg-yellow-500/20 px-2 py-1 text-[10px] font-medium text-yellow-400">
            <AlertTriangle size={10} /> {t('network.aircrackNeedsRoot')}
          </span>
        )}
      </div>

      {/* Persistent warning banner */}
      <div className="flex items-center gap-2 rounded-lg border border-red-500/30 bg-red-500/5 px-3 py-2 text-[11px] text-red-400">
        <AlertTriangle size={14} className="shrink-0" />
        <span>For authorized testing and education only. All operations are logged.</span>
      </div>

      {/* Tool status badges */}
      {toolStatus && (
        <div className="flex flex-wrap items-center gap-2">
          {[
            { name: 'aircrack-ng', ok: toolStatus.aircrack_ng },
            { name: 'airmon-ng', ok: toolStatus.airmon_ng },
            { name: 'airodump-ng', ok: toolStatus.airodump_ng },
            { name: 'aireplay-ng', ok: toolStatus.aireplay_ng },
          ].map(({ name, ok }) => (
            <span
              key={name}
              className={clsx(
                'flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-medium',
                ok ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400'
              )}
            >
              {ok ? <CheckCircle2 size={10} /> : <XCircle size={10} />}
              {name}
            </span>
          ))}
        </div>
      )}

      {/* Tabs */}
      <div className="flex items-center gap-1 border-b border-border-subtle">
        {tabs.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            onClick={() => setActiveTab(id)}
            className={clsx(
              'flex items-center gap-1.5 border-b-2 px-3 py-1.5 text-xs font-medium transition-colors',
              activeTab === id
                ? 'border-red-500 text-text-primary'
                : 'border-transparent text-text-secondary hover:text-text-primary'
            )}
          >
            <Icon size={14} />
            {label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="flex-1 overflow-y-auto">
        {activeTab === 'interfaces' && <InterfacesTab t={t} />}
        {activeTab === 'scan' && <ScanTab t={t} />}
        {activeTab === 'test' && <TestTab t={t} />}
        {activeTab === 'log' && <AuditLogTab t={t} />}
      </div>
    </div>
  );
}

// ── Interfaces Tab ──────────────────────────────────────────────────────

function InterfacesTab({ t }: Readonly<{ t: (key: string) => string }>) {
  const [interfaces, setInterfaces] = useState<WirelessInterface[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const ifaces = await invoke<WirelessInterface[]>('network_aircrack_interfaces');
      setInterfaces(ifaces);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  const handleMonitorToggle = useCallback(async (iface: WirelessInterface) => {
    setLoading(true);
    try {
      if (iface.monitor_mode) {
        await invoke('network_aircrack_monitor_stop', { interface: iface.name });
      } else {
        await invoke('network_aircrack_monitor_start', { interface: iface.name });
      }
      await refresh();
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [refresh]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-semibold text-text-primary">{t('network.aircrackInterfaces')}</h3>
        <button onClick={refresh} disabled={loading} className="text-xs text-text-secondary hover:text-text-primary">
          {loading ? <Loader2 size={12} className="animate-spin" /> : 'Refresh'}
        </button>
      </div>

      {/* Monitor mode warning */}
      <div className="flex items-center gap-2 rounded-md border border-yellow-500/30 bg-yellow-500/5 px-3 py-2 text-[11px] text-yellow-400">
        <AlertTriangle size={12} className="shrink-0" />
        {t('network.aircrackMonitorWarning')}
      </div>

      {error && (
        <div className="rounded-md border border-status-error bg-red-500/10 px-3 py-2 text-xs text-status-error">
          {error}
        </div>
      )}

      {interfaces.length === 0 && !loading && (
        <div className="py-4 text-center text-xs text-text-secondary">
          {t('network.aircrackNoInterfaces')}
        </div>
      )}

      {interfaces.map((iface) => (
        <div
          key={iface.name}
          className="flex items-center justify-between rounded-lg border border-border-subtle bg-surface-primary px-3 py-2"
        >
          <div className="flex flex-col gap-0.5">
            <div className="flex items-center gap-2">
              <Radio size={14} className="text-text-secondary" />
              <span className="text-sm font-medium text-text-primary">{iface.name}</span>
              {iface.monitor_mode && (
                <span className="rounded bg-red-500/20 px-1.5 py-0.5 text-[10px] font-medium text-red-400">
                  MONITOR
                </span>
              )}
            </div>
            <span className="text-[11px] text-text-secondary">
              {[iface.driver, iface.chipset].filter(Boolean).join(' · ') || 'Unknown adapter'}
            </span>
          </div>

          <button
            onClick={() => handleMonitorToggle(iface)}
            disabled={loading}
            className={clsx(
              'flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-colors',
              iface.monitor_mode
                ? 'bg-surface-elevated text-text-primary hover:bg-surface-secondary'
                : 'bg-red-600 text-white hover:bg-red-700'
            )}
          >
            {iface.monitor_mode ? (
              <><WifiOff size={12} /> {t('network.aircrackMonitorStop')}</>
            ) : (
              <><Wifi size={12} /> {t('network.aircrackMonitorStart')}</>
            )}
          </button>
        </div>
      ))}
    </div>
  );
}

// ── Scan Tab ────────────────────────────────────────────────────────────

function ScanTab({ t }: Readonly<{ t: (key: string) => string }>) {
  const [scanning, setScanning] = useState(false);
  const [result, setResult] = useState<AirodumpResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [duration, setDuration] = useState('15');
  const [channel, setChannel] = useState('');
  const [monInterface, setMonInterface] = useState('');
  const [interfaces, setInterfaces] = useState<WirelessInterface[]>([]);

  // Load monitor interfaces
  useEffect(() => {
    invoke<WirelessInterface[]>('network_aircrack_interfaces')
      .then((ifaces) => {
        setInterfaces(ifaces);
        const mon = ifaces.find((i) => i.monitor_mode);
        if (mon) setMonInterface(mon.name);
      })
      .catch(() => {});
  }, []);

  const handleScan = useCallback(async () => {
    if (!monInterface) return;
    setScanning(true);
    setError(null);
    try {
      const data = await invoke<AirodumpResult>('network_aircrack_scan_start', {
        interface: monInterface,
        durationSecs: parseInt(duration) || 15,
        channel: channel ? parseInt(channel) : null,
      });
      setResult(data);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setScanning(false);
    }
  }, [monInterface, duration, channel]);

  const monInterfaces = interfaces.filter((i) => i.monitor_mode);

  return (
    <div className="flex flex-col gap-3">
      {monInterfaces.length === 0 && (
        <div className="flex items-center gap-2 rounded-md border border-yellow-500/30 bg-yellow-500/5 px-3 py-2 text-xs text-yellow-400">
          <AlertTriangle size={12} />
          Enable monitor mode on an interface first (Interfaces tab).
        </div>
      )}

      {/* Controls */}
      <div className="flex flex-wrap items-end gap-3">
        <div>
          <label className="mb-1 block text-[10px] font-medium text-text-secondary">Interface</label>
          <select
            value={monInterface}
            onChange={(e) => setMonInterface(e.target.value)}
            className="rounded-md border border-border-default bg-surface-primary px-2 py-1.5 text-xs text-text-primary"
          >
            <option value="">Select interface</option>
            {monInterfaces.map((i) => (
              <option key={i.name} value={i.name}>{i.name}</option>
            ))}
          </select>
        </div>
        <div>
          <label className="mb-1 block text-[10px] font-medium text-text-secondary">
            {t('network.aircrackScanDuration')}
          </label>
          <input
            type="number"
            min="5"
            max="120"
            value={duration}
            onChange={(e) => setDuration(e.target.value)}
            className="w-20 rounded-md border border-border-default bg-surface-primary px-2 py-1.5 text-xs text-text-primary"
          />
        </div>
        <div>
          <label className="mb-1 block text-[10px] font-medium text-text-secondary">
            {t('network.aircrackScanChannel')}
          </label>
          <input
            type="number"
            min="1"
            max="165"
            value={channel}
            onChange={(e) => setChannel(e.target.value)}
            placeholder="All"
            className="w-20 rounded-md border border-border-default bg-surface-primary px-2 py-1.5 text-xs text-text-primary placeholder:text-text-disabled"
          />
        </div>
        <button
          onClick={handleScan}
          disabled={scanning || !monInterface}
          className="flex items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1.5 text-xs font-medium text-text-inverse hover:bg-interactive-hover disabled:cursor-not-allowed disabled:bg-interactive-disabled disabled:text-text-disabled"
        >
          {scanning ? <Loader2 size={12} className="animate-spin" /> : <Play size={12} />}
          {scanning ? t('network.aircrackScanning') : t('network.aircrackStartScan')}
        </button>
      </div>

      {error && (
        <div className="rounded-md border border-status-error bg-red-500/10 px-3 py-2 text-xs text-status-error">
          {error}
        </div>
      )}

      {/* Results */}
      {result && (
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-3">
            <span className="rounded bg-surface-elevated px-2 py-1 text-xs text-text-secondary">
              {result.networks.length} networks
            </span>
            <span className="rounded bg-surface-elevated px-2 py-1 text-xs text-text-secondary">
              {result.clients.length} clients
            </span>
            <span className="rounded bg-surface-elevated px-2 py-1 text-xs text-text-secondary">
              {result.scan_time_secs}s scan
            </span>
          </div>

          {/* Networks table */}
          <div className="overflow-x-auto rounded-lg border border-border-subtle">
            <table className="w-full text-[11px]">
              <thead className="bg-surface-elevated text-text-secondary">
                <tr>
                  <th className="px-2 py-1.5 text-left font-medium">ESSID</th>
                  <th className="px-2 py-1.5 text-left font-medium">BSSID</th>
                  <th className="px-2 py-1.5 text-center font-medium">CH</th>
                  <th className="px-2 py-1.5 text-center font-medium">PWR</th>
                  <th className="px-2 py-1.5 text-left font-medium">Encryption</th>
                  <th className="px-2 py-1.5 text-center font-medium">Data</th>
                  <th className="px-2 py-1.5 text-center font-medium">
                    <Users size={10} className="inline" /> Clients
                  </th>
                </tr>
              </thead>
              <tbody>
                {result.networks
                  .sort((a, b) => b.power - a.power)
                  .map((net) => (
                    <tr key={net.bssid} className="border-t border-border-subtle hover:bg-surface-elevated">
                      <td className="px-2 py-1.5 font-medium text-text-primary">
                        {net.essid || '(hidden)'}
                      </td>
                      <td className="px-2 py-1.5 font-mono text-text-secondary">{net.bssid}</td>
                      <td className="px-2 py-1.5 text-center text-text-primary">{net.channel}</td>
                      <td className="px-2 py-1.5 text-center">
                        <span className={clsx(
                          'font-medium',
                          net.power >= -50 ? 'text-green-400' :
                          net.power >= -70 ? 'text-yellow-400' : 'text-red-400'
                        )}>
                          {net.power}
                        </span>
                      </td>
                      <td className="px-2 py-1.5">
                        <span className={clsx(
                          'rounded px-1.5 py-0.5 text-[10px] font-medium',
                          net.privacy.includes('WPA3') ? 'bg-green-500/20 text-green-400' :
                          net.privacy.includes('WPA2') ? 'bg-blue-500/20 text-blue-300' :
                          net.privacy.includes('WPA') ? 'bg-yellow-500/20 text-yellow-400' :
                          net.privacy.includes('WEP') ? 'bg-red-500/20 text-red-400' :
                          net.privacy === 'OPN' ? 'bg-red-500/20 text-red-400' :
                          'bg-surface-elevated text-text-secondary'
                        )}>
                          {net.privacy}{net.cipher ? ` / ${net.cipher}` : ''}
                        </span>
                      </td>
                      <td className="px-2 py-1.5 text-center text-text-secondary">{net.data_frames}</td>
                      <td className="px-2 py-1.5 text-center text-text-secondary">{net.clients}</td>
                    </tr>
                  ))}
              </tbody>
            </table>
          </div>

          {/* Clients */}
          {result.clients.length > 0 && (
            <div className="flex flex-col gap-1">
              <h4 className="text-xs font-semibold text-text-primary">Connected Clients</h4>
              <div className="overflow-x-auto rounded-lg border border-border-subtle">
                <table className="w-full text-[11px]">
                  <thead className="bg-surface-elevated text-text-secondary">
                    <tr>
                      <th className="px-2 py-1.5 text-left font-medium">Station MAC</th>
                      <th className="px-2 py-1.5 text-left font-medium">BSSID</th>
                      <th className="px-2 py-1.5 text-center font-medium">PWR</th>
                      <th className="px-2 py-1.5 text-center font-medium">Packets</th>
                      <th className="px-2 py-1.5 text-left font-medium">Probes</th>
                    </tr>
                  </thead>
                  <tbody>
                    {result.clients.map((client) => (
                      <tr key={client.station_mac} className="border-t border-border-subtle hover:bg-surface-elevated">
                        <td className="px-2 py-1.5 font-mono text-text-primary">{client.station_mac}</td>
                        <td className="px-2 py-1.5 font-mono text-text-secondary">{client.bssid}</td>
                        <td className="px-2 py-1.5 text-center text-text-primary">{client.power}</td>
                        <td className="px-2 py-1.5 text-center text-text-secondary">{client.packets}</td>
                        <td className="px-2 py-1.5 text-text-secondary">{client.probes.join(', ')}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}

      {!result && !scanning && (
        <div className="flex flex-col items-center justify-center gap-2 py-8 text-text-secondary">
          <Eye size={24} />
          <p className="text-xs">Start a scan to discover networks and connected clients.</p>
        </div>
      )}
    </div>
  );
}

// ── Test Tab (Deauth + Handshake Capture + Crack) ───────────────────────

function TestTab({ t }: Readonly<{ t: (key: string) => string }>) {
  const [monInterface, setMonInterface] = useState('');
  const [interfaces, setInterfaces] = useState<WirelessInterface[]>([]);
  const [targetBssid, setTargetBssid] = useState('');
  const [targetChannel, setTargetChannel] = useState('');
  const [clientMac, setClientMac] = useState('');

  // Deauth state
  const [deauthCount, setDeauthCount] = useState('5');
  const [deauthResult, setDeauthResult] = useState<string | null>(null);
  const [deauthRunning, setDeauthRunning] = useState(false);

  // Handshake state
  const [captureStatus, setCaptureStatus] = useState<HandshakeCaptureStatus | null>(null);
  const [capturing, setCapturing] = useState(false);
  const [sendDeauthDuringCapture, setSendDeauthDuringCapture] = useState(false);

  // Crack state
  const [wordlistPath, setWordlistPath] = useState('');
  const [crackProgress, setCrackProgress] = useState<CrackProgress | null>(null);
  const [cracking, setCracking] = useState(false);

  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<WirelessInterface[]>('network_aircrack_interfaces')
      .then((ifaces) => {
        setInterfaces(ifaces);
        const mon = ifaces.find((i) => i.monitor_mode);
        if (mon) setMonInterface(mon.name);
      })
      .catch(() => {});
  }, []);

  const monInterfaces = interfaces.filter((i) => i.monitor_mode);

  // Deauth
  const handleDeauth = useCallback(async () => {
    if (!monInterface || !targetBssid) return;
    setDeauthRunning(true);
    setDeauthResult(null);
    setError(null);
    try {
      const msg = await invoke<string>('network_aircrack_deauth', {
        interface: monInterface,
        targetBssid,
        clientMac: clientMac || null,
        count: parseInt(deauthCount) || 5,
      });
      setDeauthResult(msg);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setDeauthRunning(false);
    }
  }, [monInterface, targetBssid, clientMac, deauthCount]);

  // Handshake capture
  const handleCapture = useCallback(async () => {
    if (!monInterface || !targetBssid || !targetChannel) return;
    setCapturing(true);
    setCaptureStatus(null);
    setError(null);
    try {
      const status = await invoke<HandshakeCaptureStatus>('network_aircrack_capture_handshake', {
        interface: monInterface,
        targetBssid,
        targetChannel: parseInt(targetChannel),
        sendDeauth: sendDeauthDuringCapture,
        timeoutSecs: 60,
      });
      setCaptureStatus(status);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setCapturing(false);
    }
  }, [monInterface, targetBssid, targetChannel, sendDeauthDuringCapture]);

  // Crack
  const handleCrack = useCallback(async () => {
    if (!captureStatus?.capture_file || !wordlistPath || !targetBssid) return;
    setCracking(true);
    setCrackProgress(null);
    setError(null);
    try {
      const progress = await invoke<CrackProgress>('network_aircrack_crack_start', {
        captureFile: captureStatus.capture_file,
        targetBssid,
        wordlistPath,
      });
      setCrackProgress(progress);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setCracking(false);
    }
  }, [captureStatus, targetBssid, wordlistPath]);

  return (
    <div className="flex flex-col gap-4">
      {monInterfaces.length === 0 && (
        <div className="flex items-center gap-2 rounded-md border border-yellow-500/30 bg-yellow-500/5 px-3 py-2 text-xs text-yellow-400">
          <AlertTriangle size={12} />
          Enable monitor mode on an interface first (Interfaces tab).
        </div>
      )}

      {/* Common target fields */}
      <div className="flex flex-wrap items-end gap-3">
        <div>
          <label className="mb-1 block text-[10px] font-medium text-text-secondary">Interface</label>
          <select
            value={monInterface}
            onChange={(e) => setMonInterface(e.target.value)}
            className="rounded-md border border-border-default bg-surface-primary px-2 py-1.5 text-xs text-text-primary"
          >
            <option value="">Select interface</option>
            {monInterfaces.map((i) => (
              <option key={i.name} value={i.name}>{i.name}</option>
            ))}
          </select>
        </div>
        <div>
          <label className="mb-1 block text-[10px] font-medium text-text-secondary">
            {t('network.aircrackTargetBssid')}
          </label>
          <input
            type="text"
            value={targetBssid}
            onChange={(e) => setTargetBssid(e.target.value)}
            placeholder="AA:BB:CC:DD:EE:FF"
            className="w-44 rounded-md border border-border-default bg-surface-primary px-2 py-1.5 text-xs text-text-primary font-mono placeholder:text-text-disabled"
          />
        </div>
        <div>
          <label className="mb-1 block text-[10px] font-medium text-text-secondary">Channel</label>
          <input
            type="number"
            min="1"
            max="165"
            value={targetChannel}
            onChange={(e) => setTargetChannel(e.target.value)}
            className="w-16 rounded-md border border-border-default bg-surface-primary px-2 py-1.5 text-xs text-text-primary"
          />
        </div>
      </div>

      {error && (
        <div className="rounded-md border border-status-error bg-red-500/10 px-3 py-2 text-xs text-status-error">
          {error}
        </div>
      )}

      {/* ── Section 1: Deauth ── */}
      <div className="flex flex-col gap-2 rounded-lg border-2 border-red-500/30 bg-red-500/5 p-3">
        <div className="flex items-center gap-2">
          <Zap size={14} className="text-red-400" />
          <h4 className="text-xs font-semibold text-red-400">{t('network.aircrackDeauth')}</h4>
        </div>
        <p className="text-[11px] text-red-400/80">{t('network.aircrackDeauthWarning')}</p>

        <div className="flex items-end gap-3">
          <div>
            <label className="mb-1 block text-[10px] font-medium text-text-secondary">
              {t('network.aircrackClientMac')}
            </label>
            <input
              type="text"
              value={clientMac}
              onChange={(e) => setClientMac(e.target.value)}
              placeholder="(broadcast)"
              className="w-44 rounded-md border border-border-default bg-surface-primary px-2 py-1.5 text-xs text-text-primary font-mono placeholder:text-text-disabled"
            />
          </div>
          <div>
            <label className="mb-1 block text-[10px] font-medium text-text-secondary">
              {t('network.aircrackDeauthCount')}
            </label>
            <input
              type="number"
              min="1"
              max="50"
              value={deauthCount}
              onChange={(e) => setDeauthCount(e.target.value)}
              className="w-16 rounded-md border border-border-default bg-surface-primary px-2 py-1.5 text-xs text-text-primary"
            />
          </div>
          <button
            onClick={handleDeauth}
            disabled={deauthRunning || !monInterface || !targetBssid}
            className="flex items-center gap-1.5 rounded-md bg-red-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:bg-interactive-disabled disabled:text-text-disabled"
          >
            {deauthRunning ? <Loader2 size={12} className="animate-spin" /> : <Zap size={12} />}
            {t('network.aircrackSendDeauth')}
          </button>
        </div>

        {deauthResult && (
          <div className="rounded bg-surface-elevated px-3 py-2 text-xs text-text-secondary">{deauthResult}</div>
        )}
      </div>

      {/* ── Section 2: Handshake Capture ── */}
      <div className="flex flex-col gap-2 rounded-lg border border-border-subtle bg-surface-primary p-3">
        <div className="flex items-center gap-2">
          <Lock size={14} className="text-text-secondary" />
          <h4 className="text-xs font-semibold text-text-primary">{t('network.aircrackHandshake')}</h4>
        </div>
        <p className="text-[11px] text-text-secondary">{t('network.aircrackHandshakeDesc')}</p>

        <div className="flex items-center gap-3">
          <label className="flex items-center gap-1.5 text-xs text-text-secondary">
            <input
              type="checkbox"
              checked={sendDeauthDuringCapture}
              onChange={(e) => setSendDeauthDuringCapture(e.target.checked)}
              className="accent-red-500"
            />
            {t('network.aircrackSendDeauthDuringCapture')}
          </label>
          <button
            onClick={handleCapture}
            disabled={capturing || !monInterface || !targetBssid || !targetChannel}
            className="flex items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1.5 text-xs font-medium text-text-inverse hover:bg-interactive-hover disabled:cursor-not-allowed disabled:bg-interactive-disabled disabled:text-text-disabled"
          >
            {capturing ? <Loader2 size={12} className="animate-spin" /> : <Lock size={12} />}
            {capturing ? t('network.aircrackCapturing') : t('network.aircrackCaptureHandshake')}
          </button>
        </div>

        {captureStatus && (
          <div className={clsx(
            'flex items-center gap-2 rounded-md px-3 py-2 text-xs font-medium',
            captureStatus.handshake_captured
              ? 'border border-green-500/30 bg-green-500/10 text-green-400'
              : 'border border-yellow-500/30 bg-yellow-500/10 text-yellow-400'
          )}>
            {captureStatus.handshake_captured ? <CheckCircle2 size={14} /> : <AlertTriangle size={14} />}
            {captureStatus.handshake_captured
              ? t('network.aircrackHandshakeCaptured')
              : t('network.aircrackHandshakeFailed')}
          </div>
        )}
      </div>

      {/* ── Section 3: Password Strength Test ── */}
      <div className="flex flex-col gap-2 rounded-lg border border-border-subtle bg-surface-primary p-3">
        <div className="flex items-center gap-2">
          <Unlock size={14} className="text-text-secondary" />
          <h4 className="text-xs font-semibold text-text-primary">{t('network.aircrackCrack')}</h4>
        </div>
        <p className="text-[11px] text-text-secondary">{t('network.aircrackCrackDesc')}</p>

        <div className="flex items-end gap-3">
          <div className="flex-1">
            <label className="mb-1 block text-[10px] font-medium text-text-secondary">
              {t('network.aircrackWordlistPath')}
            </label>
            <input
              type="text"
              value={wordlistPath}
              onChange={(e) => setWordlistPath(e.target.value)}
              placeholder="/usr/share/wordlists/rockyou.txt"
              className="w-full rounded-md border border-border-default bg-surface-primary px-2 py-1.5 text-xs text-text-primary font-mono placeholder:text-text-disabled"
            />
          </div>
          <button
            onClick={handleCrack}
            disabled={cracking || !captureStatus?.capture_file || !wordlistPath}
            className="flex items-center gap-1.5 rounded-md bg-interactive-default px-3 py-1.5 text-xs font-medium text-text-inverse hover:bg-interactive-hover disabled:cursor-not-allowed disabled:bg-interactive-disabled disabled:text-text-disabled"
          >
            {cracking ? <Loader2 size={12} className="animate-spin" /> : <Activity size={12} />}
            {cracking ? t('network.aircrackCracking') : t('network.aircrackStartCrack')}
          </button>
        </div>

        {!captureStatus?.capture_file && (
          <p className="text-[10px] text-text-disabled">Capture a handshake first to test password strength.</p>
        )}

        {crackProgress && (
          <div className={clsx(
            'flex flex-col gap-1 rounded-md px-3 py-2 text-xs',
            crackProgress.key_found
              ? 'border-2 border-red-500/50 bg-red-500/10'
              : 'border border-green-500/30 bg-green-500/10'
          )}>
            {crackProgress.key_found ? (
              <>
                <div className="flex items-center gap-2 font-bold text-red-400">
                  <ShieldX size={16} />
                  {t('network.aircrackKeyFound')}
                </div>
                <div className="flex items-center gap-2 rounded bg-red-500/20 px-2 py-1 font-mono text-sm text-red-300">
                  <Lock size={12} />
                  Key: {crackProgress.key_found}
                </div>
              </>
            ) : (
              <div className="flex items-center gap-2 font-medium text-green-400">
                <ShieldCheck size={16} />
                {t('network.aircrackKeyNotFound')}
              </div>
            )}
            <div className="flex items-center gap-3 text-text-secondary">
              <span>{crackProgress.keys_tested.toLocaleString()} keys tested</span>
              <span>{crackProgress.keys_per_second.toFixed(0)} keys/sec</span>
              <span>{crackProgress.elapsed_secs}s</span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Audit Log Tab ───────────────────────────────────────────────────────

function AuditLogTab({ t }: Readonly<{ t: (key: string) => string }>) {
  const [entries, setEntries] = useState<AircrackAuditEntry[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const log = await invoke<AircrackAuditEntry[]>('network_aircrack_audit_log');
      setEntries(log.reverse());
    } catch {
      // ignore
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  const handleStopAll = useCallback(async () => {
    try {
      await invoke('network_aircrack_stop_all');
      refresh();
    } catch {
      // ignore
    }
  }, [refresh]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-xs font-semibold text-text-primary">{t('network.aircrackAuditLog')}</h3>
          <p className="text-[10px] text-text-secondary">{t('network.aircrackAuditLogDesc')}</p>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={handleStopAll}
            className="flex items-center gap-1 rounded-md bg-red-600 px-2 py-1 text-[10px] font-medium text-white hover:bg-red-700"
          >
            <Square size={10} />
            {t('network.aircrackStopAll')}
          </button>
          <button
            onClick={refresh}
            disabled={loading}
            className="text-xs text-text-secondary hover:text-text-primary"
          >
            {loading ? <Loader2 size={12} className="animate-spin" /> : 'Refresh'}
          </button>
        </div>
      </div>

      {entries.length === 0 ? (
        <div className="py-8 text-center text-xs text-text-secondary">
          <List size={24} className="mx-auto mb-2 text-text-disabled" />
          No operations logged yet.
        </div>
      ) : (
        <div className="flex flex-col gap-1">
          {entries.map((entry, i) => (
            <div
              key={`${entry.timestamp}-${i}`}
              className="flex items-start gap-3 rounded-lg border border-border-subtle bg-surface-primary px-3 py-2"
            >
              <span className="mt-0.5 text-[10px] text-text-disabled">
                {new Date(entry.timestamp).toLocaleTimeString()}
              </span>
              <span className={clsx(
                'rounded px-1.5 py-0.5 text-[10px] font-medium',
                entry.operation === 'deauth' ? 'bg-red-500/20 text-red-400' :
                entry.operation === 'capture_handshake' ? 'bg-yellow-500/20 text-yellow-400' :
                entry.operation === 'crack_wpa' ? 'bg-purple-500/20 text-purple-400' :
                'bg-surface-elevated text-text-secondary'
              )}>
                {entry.operation}
              </span>
              <div className="flex flex-1 flex-col gap-0.5 overflow-hidden">
                <span className="truncate text-[11px] text-text-primary">{entry.result}</span>
                <span className="truncate font-mono text-[10px] text-text-disabled">{entry.command}</span>
                {entry.target && (
                  <span className="text-[10px] text-text-disabled">Target: {entry.target}</span>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
