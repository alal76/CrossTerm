import { Fragment, useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
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
  ChevronRight,
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
  Square,
  Download,
  Lock,
  Radio,
  Info,
  Pencil,
  Router,
  Camera,
  Printer,
  HardDrive,
  Box,
  Smartphone,
  Laptop,
  Cast,
  HelpCircle,
  Waypoints,
} from 'lucide-react';
import type { ExploreResult, ExploreProgress, ExploreHostFound, ExploreMdnsUpdate, MdnsRecord, ServiceFilter, Session, TailscalePeer } from '@/types';
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
  { id: 'win_rm', label: 'WinRM (5985)', port: 5985 },
  { id: 'win_rm_tls', label: 'WinRM TLS (5986)', port: 5986 },
  { id: 'mqtt', label: 'MQTT (1883)', port: 1883 },
  { id: 'mqtt_tls', label: 'MQTT TLS (8883)', port: 8883 },
  { id: 'netconf', label: 'NETCONF (830)', port: 830 },
  { id: 'grpc', label: 'gRPC (50051)', port: 50051 },
  { id: 'kube_api', label: 'Kubernetes API (6443)', port: 6443 },
  { id: 'docker_api', label: 'Docker API (2375)', port: 2375 },
  { id: 'ws_terminal', label: 'WS Terminal / ttyd (7681)', port: 7681 },
  { id: 'rtsp', label: 'RTSP (554)', port: 554 },
];

// Mirrors the backend's `DEFAULT_EXPLORE_SERVICES` (src-tauri/src/network/mod.rs).
// `network_explore_start` only falls back to that Rust-side default when the
// `services` array sent from here is empty — since this UI always sends a
// concrete list, the two have to be kept in sync explicitly or the backend's
// broader default silently never takes effect. Deliberately excludes the
// database ports (mysql/postgresql/redis/mongodb) and mqtt_tls/docker_api_tls,
// same as the backend default — those stay opt-in via the checkboxes.
const DEFAULT_SELECTED_SERVICES = [
  'ssh', 'rdp', 'vnc', 'http', 'https', 'telnet', 'ftp', 'smb',
  'win_rm', 'win_rm_tls', 'mqtt', 'netconf', 'grpc', 'kube_api',
  'docker_api', 'ws_terminal', 'rtsp',
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
  rtsp: 'bg-pink-500/20 text-pink-400',
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
  winrm: SessionType.WinRM,
  'winrm-tls': SessionType.WinRM,
  'kube-api': SessionType.KubernetesExec,
  'docker-api': SessionType.DockerExec,
  'docker-api-tls': SessionType.DockerExec,
  mqtt: SessionType.MqttClient,
  'mqtt-tls': SessionType.MqttClient,
  grpc: SessionType.GrpcExplorer,
  netconf: SessionType.NetConf,
  wsterm: SessionType.WebSocketTerminal,
  kubernetes_exec: SessionType.KubernetesExec,
  docker_exec: SessionType.DockerExec,
  mqtt_client: SessionType.MqttClient,
  grpc_explorer: SessionType.GrpcExplorer,
  websocket_terminal: SessionType.WebSocketTerminal,
};

type SortKey = 'ip' | 'hostname' | 'name' | 'type' | 'mac' | 'vendor' | 'os' | 'ports' | 'response' | 'session';

/// String compare that always sorts missing values last, regardless of
/// sort direction — `dir` only flips the ordering among present values.
function compareNullable(a: string | null | undefined, b: string | null | undefined, dir: number): number {
  if (a == null && b == null) return 0;
  if (a == null) return 1;
  if (b == null) return -1;
  return dir * a.localeCompare(b);
}

function ipToNum(ip: string): number {
  const parts = ip.split('.').map(Number);
  return ((parts[0] ?? 0) << 24) | ((parts[1] ?? 0) << 16) | ((parts[2] ?? 0) << 8) | (parts[3] ?? 0);
}

function SortIcon({ col, sortBy, sortDir }: { col: SortKey; sortBy: SortKey; sortDir: 'asc' | 'desc' }) {
  if (sortBy !== col) return <ArrowUpDown size={11} className="opacity-30" />;
  return sortDir === 'asc' ? <ArrowUp size={11} /> : <ArrowDown size={11} />;
}

// mDNS instances often carry a device-identifying name that reverse-DNS/
// NetBIOS/ARP/TLS-CN resolution never sees, since these devices don't run a
// resolvable DNS/NetBIOS name at all. Prefer, in order: the Google
// Cast/Chromecast TXT "fn" (friendly name, e.g. "Hall clock" — the most
// human-readable label a device advertises about itself), the advertised
// `hostname` field (a real DNS-style name, but Cast devices often set this
// to an opaque device UUID), then the service instance name as a last resort.
function deriveMdnsHostname(records: MdnsRecord[] | undefined): string | undefined {
  if (!records || records.length === 0) return undefined;
  const withFriendlyName = records.find((r) => r.txt?.fn);
  if (withFriendlyName?.txt.fn) return withFriendlyName.txt.fn;
  const withHost = records.find((r) => r.hostname);
  if (withHost?.hostname) return withHost.hostname;
  return records[0].instance_name || undefined;
}

// Cast-ecosystem TXT "md" (model) strings are self-reported by the device and
// often identify the actual product/brand more specifically than the NIC's
// OUI vendor lookup can — e.g. a MAC registered to "Motorola (Wuhan) Mobility
// Technologies" (Lenovo's OEM/manufacturing arm) shows up here as
// "LenovoCD-24502F", which is what actually answers "whose device is this".
function deriveMdnsEvidence(records: MdnsRecord[] | undefined): string[] {
  if (!records) return [];
  const notes: string[] = [];
  for (const r of records) {
    if (r.txt?.md) {
      notes.push(`mDNS (${r.service_type}): model "${r.txt.md}"${r.txt.fn ? ` — "${r.txt.fn}"` : ''}`);
    }
  }
  return notes;
}

// ── Device type classification ──────────────────────────────────────────
// Best-effort, derived entirely from signals already on the row (open
// ports, mDNS service types, hostname/vendor/OS-guess text, and version
// strings enrich_port already populated e.g. "Jellyfin: ..."). Not
// authoritative — a label the user can eyeball, not a claim of certainty.
type DeviceType = 'router' | 'camera' | 'printer' | 'nas' | 'vm' | 'smart-home' | 'phone' | 'computer' | 'media' | 'unknown';

const DEVICE_TYPE_META: Record<DeviceType, { label: string; icon: typeof Router }> = {
  router: { label: 'Router/Gateway', icon: Router },
  camera: { label: 'Camera', icon: Camera },
  printer: { label: 'Printer', icon: Printer },
  nas: { label: 'NAS/Server', icon: HardDrive },
  vm: { label: 'Virtual Machine', icon: Box },
  'smart-home': { label: 'Smart Home', icon: Radio },
  phone: { label: 'Phone/Tablet', icon: Smartphone },
  computer: { label: 'Computer', icon: Laptop },
  media: { label: 'Media Player', icon: Cast },
  unknown: { label: 'Unknown', icon: HelpCircle },
};

function classifyDeviceType(result: ExploreResult): DeviceType {
  const ports = new Set(result.open_ports.map((p) => p.port));
  const mdnsTypes = result.mdns.map((m) => m.service_type.toLowerCase());
  const versions = result.open_ports.map((p) => (p.version ?? '').toLowerCase()).join(' ');
  const hostname = (result.hostname ?? '').toLowerCase();
  const vendor = (result.mac_vendor ?? '').toLowerCase();
  const osGuess = (result.os_guess ?? '').toLowerCase();

  if (
    ports.has(554) ||
    mdnsTypes.some((t) => t.includes('onvif') || t.includes('rtsp')) ||
    /\b(cam|camera|dcs-|ipcam)\b/.test(hostname)
  ) {
    return 'camera';
  }
  if (mdnsTypes.some((t) => t.includes('_ipp') || t.includes('_printer')) || ports.has(631) || ports.has(9100)) {
    return 'printer';
  }
  // Conventional home-router address, with an admin UI or a known
  // router-vendor OUI — not foolproof (plenty of non-routers sit at .1),
  // but a reasonable default given no stronger signal exists.
  const isDotOne = /\.1$/.test(result.ip);
  const routerVendors = ['sagemcom', 'technicolor', 'arris', 'netgear', 'ubiquiti', 'mikrotik', 'compal', 'huawei', 'zyxel'];
  if (isDotOne && (routerVendors.some((v) => vendor.includes(v)) || ports.has(80) || ports.has(443))) {
    return 'router';
  }
  if (
    mdnsTypes.some((t) => t.includes('googlecast') || t.includes('home-assistant') || t.includes('matter') || t.includes('_hap') || t.includes('esphome')) ||
    ports.has(1883) || ports.has(8883)
  ) {
    return 'smart-home';
  }
  if (mdnsTypes.some((t) => t.includes('airplay') || t.includes('raop') || t.includes('spotify-connect'))) {
    return 'media';
  }
  if (vendor.includes('proxmox') || osGuess.includes('proxmox')) {
    return 'vm';
  }
  if (
    /\bnas\b|jellyfin|plex|pi\.?hole|homeassistant/.test(hostname) ||
    vendor.includes('synology') || vendor.includes('qnap') ||
    versions.includes('jellyfin') || versions.includes('plex')
  ) {
    return 'nas';
  }
  if (/iphone|ipad|android|pixel|galaxy|redmi/.test(hostname) || osGuess === 'ios' || osGuess === 'android') {
    return 'phone';
  }
  if (
    /macbook|imac|-pc\b|desktop|laptop/.test(hostname) ||
    ['windows', 'macos', 'linux', 'ubuntu', 'debian'].some((s) => osGuess.includes(s))
  ) {
    return 'computer';
  }
  return 'unknown';
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
    () => new Set(DEFAULT_SELECTED_SERVICES)
  );
  const [showFilters, setShowFilters] = useState(false);
  const [filterService, setFilterService] = useState<string>('all');
  const [searchFilter, setSearchFilter] = useState('');
  const [sortBy, setSortBy] = useState<SortKey>('ip');
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('asc');
  const [saveCount, setSaveCount] = useState<number | null>(null);
  const [tailscaleLoading, setTailscaleLoading] = useState(false);
  const [connectionHistory, setConnectionHistory] = useState<ConnectionAttempt[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const [expandedIp, setExpandedIp] = useState<string | null>(null);

  // User-assigned friendly device names, keyed by MAC address (IP as
  // fallback for the rare host with no resolvable MAC) — persisted in
  // per-profile Settings (network_device_labels), not the credential vault:
  // labels aren't secrets, and the vault's CredentialType enum has no
  // generic-metadata variant, so storing them there would mean modifying
  // security-sensitive code just to gate a UI label behind vault unlock for
  // no security benefit. Settings has neither restriction.
  const [deviceLabels, setDeviceLabels] = useState<Record<string, string>>({});
  const [editingLabelKey, setEditingLabelKey] = useState<string | null>(null);
  const [editingLabelValue, setEditingLabelValue] = useState('');
  const settingsRef = useRef<Record<string, unknown> | null>(null);
  const cancelLabelEditRef = useRef(false);
  // Mirrors `deviceLabels` for the listener-registration effect below, which
  // runs once on mount ([] deps) and would otherwise close over a stale,
  // always-empty `deviceLabels` when migrating a label's key (see the
  // explore_host_enriched handler).
  const deviceLabelsRef = useRef<Record<string, string>>({});
  useEffect(() => {
    deviceLabelsRef.current = deviceLabels;
  }, [deviceLabels]);

  useEffect(() => {
    invoke<Record<string, unknown>>('settings_get')
      .then((s) => {
        settingsRef.current = s;
        const labels = s.network_device_labels;
        if (labels && typeof labels === 'object') {
          setDeviceLabels(labels as Record<string, string>);
        }
      })
      .catch(() => {}); // graceful degradation in browser/stub mode
  }, []);

  const deviceLabelKey = useCallback((result: ExploreResult) => result.mac_address ?? result.ip, []);

  const commitDeviceLabel = useCallback(async (key: string, rawValue: string) => {
    const trimmed = rawValue.trim();
    if ((deviceLabels[key] ?? '') === trimmed) return; // unchanged, skip the write
    const next = { ...deviceLabels };
    if (trimmed) {
      next[key] = trimmed;
    } else {
      delete next[key];
    }
    setDeviceLabels(next); // optimistic
    try {
      const base = settingsRef.current ?? (await invoke<Record<string, unknown>>('settings_get'));
      const updated = { ...base, network_device_labels: next };
      settingsRef.current = updated;
      await invoke('settings_update', { settings: updated });
    } catch {
      toast('error', 'Failed to save device label');
    }
  }, [deviceLabels, toast]);

  // Set of currently active scan IDs (one per CIDR)
  const activeScanIdsRef = useRef<Set<string>>(new Set());
  // mDNS discovery's 4s window typically closes well before a full CIDR
  // sweep finishes, so `network:explore_mdns_update` usually arrives before
  // most `network:explore_host_found` events for the same scan. Buffer
  // records by IP here so a host row picks up its mDNS data (and derived
  // hostname) whichever event lands first.
  const mdnsRecordsRef = useRef<Record<string, MdnsRecord[]>>({});
  // Mirrors `results` for the enrichment handler below: a setState updater
  // function doesn't run synchronously, so code right after `setResults(...)`
  // can't rely on reading the previous row from inside it — this ref gives a
  // synchronously-readable snapshot of the prior row instead.
  const resultsRef = useRef<ExploreResult[]>([]);
  useEffect(() => {
    resultsRef.current = results;
  }, [results]);

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
          let result = event.payload.result;
          const pendingMdns = mdnsRecordsRef.current[result.ip];
          if (pendingMdns && pendingMdns.length > 0) {
            result = {
              ...result,
              mdns: pendingMdns,
              hostname: result.hostname || deriveMdnsHostname(pendingMdns),
              evidence: [...result.evidence, ...deriveMdnsEvidence(pendingMdns)],
            };
          }
          setResults((prev) => [...prev, result]);
        }
      }
    );

    // The backend now surfaces a host as soon as the port scan/ping finishes
    // (fast — just open ports, no banner/hostname/MAC/vendor yet) and streams
    // the slower enrichment (banners, TLS, 9-method hostname resolution, ARP/
    // OUI vendor lookup) in afterward via this event. Not gated on
    // activeScanIdsRef: progress (and therefore `scanning`) advances at the
    // fast port-scan pass, so by the time the last few hosts' enrichment
    // lands, their scan_id may already be gone from the active set — same
    // reasoning as the mDNS-update merge below.
    const unlistenEnriched = listen<ExploreHostFound>(
      'network:explore_host_enriched',
      (event) => {
        const enriched = event.payload.result;
        // A device's label key switches from its IP to its MAC as soon as a
        // MAC first resolves (see deviceLabelKey below) — enrichment is what
        // resolves it, arriving after the fast host-found pass. If a label
        // was saved while the row still only had an IP, it must move to the
        // MAC key now or it becomes permanently unreachable under the old key.
        // Read the prior row from the ref (not `results`/a setResults updater)
        // since both are unavailable synchronously at this point.
        const prevRow = resultsRef.current.find((r) => r.ip === enriched.ip);
        const labelMigration =
          prevRow && !prevRow.mac_address && enriched.mac_address
            ? { from: prevRow.ip, to: enriched.mac_address }
            : null;
        setResults((prev) =>
          prev.map((r) =>
            r.ip === enriched.ip
              ? {
                  ...enriched,
                  mdns: r.mdns, // preserve mDNS data merged onto this row separately
                  hostname: enriched.hostname || r.hostname, // resolver wins; else keep mDNS-derived fallback
                  evidence: Array.from(new Set([...enriched.evidence, ...r.evidence])),
                }
              : r
          )
        );
        if (labelMigration) {
          const { from, to } = labelMigration;
          const current = deviceLabelsRef.current;
          if (current[from] && !current[to]) {
            const next = { ...current };
            next[to] = next[from];
            delete next[from];
            deviceLabelsRef.current = next;
            setDeviceLabels(next);
            (async () => {
              try {
                const base = settingsRef.current ?? (await invoke<Record<string, unknown>>('settings_get'));
                const updated = { ...base, network_device_labels: next };
                settingsRef.current = updated;
                await invoke('settings_update', { settings: updated });
              } catch {
                // Best-effort background migration; a manual rename will retry the write.
              }
            })();
          }
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

    // mDNS/Bonjour results arrive after the concurrent browse window closes,
    // which may be after some (or all) hosts have already been reported —
    // merge into whichever rows match by IP rather than gating on scan_id.
    const unlistenMdns = listen<ExploreMdnsUpdate>(
      'network:explore_mdns_update',
      (event) => {
        const { records } = event.payload;
        mdnsRecordsRef.current = { ...mdnsRecordsRef.current, ...records };
        setResults((prev) =>
          prev.map((r) =>
            records[r.ip]
              ? {
                  ...r,
                  mdns: records[r.ip],
                  hostname: r.hostname || deriveMdnsHostname(records[r.ip]),
                  evidence: [...r.evidence, ...deriveMdnsEvidence(records[r.ip])],
                }
              : r
          )
        );
      }
    );

    return () => {
      unlistenHost.then((fn) => fn());
      unlistenEnriched.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
      unlistenMdns.then((fn) => fn());
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
    setExpandedIp(null);
    activeScanIdsRef.current = new Set();
    mdnsRecordsRef.current = {};

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

  const handleStop = useCallback(async () => {
    const ids = Array.from(activeScanIdsRef.current);
    activeScanIdsRef.current = new Set();
    setScanning(false);
    await Promise.allSettled(
      ids.map((scanId) => invoke('network_explore_stop', { scanId }))
    );
  }, []);

  // Tailscale peers live in a different address space (100.64.0.0/10) from
  // whatever LAN CIDR is being scanned, so they never turn up on their own —
  // `tailscale status` already knows exactly who they are, no probing
  // needed. Merges by IP: an already-discovered LAN host that's also on the
  // tailnet keeps its richer scan data, just gets tagged with Tailscale
  // provenance rather than being duplicated as a second row.
  const handleIncludeTailscale = useCallback(async () => {
    setTailscaleLoading(true);
    try {
      const peers = await invoke<TailscalePeer[]>('network_tailscale_peers');
      setResults((prev) => {
        const byIp = new Map(prev.map((r) => [r.ip, r]));
        for (const peer of peers) {
          const note = `Tailscale: ${peer.hostname}${peer.online ? '' : ' (offline)'}${peer.is_self ? ' — this device' : ''}`;
          const existing = byIp.get(peer.ip);
          if (existing) {
            byIp.set(peer.ip, {
              ...existing,
              hostname: existing.hostname || peer.hostname,
              os_guess: existing.os_guess || peer.os,
              evidence: Array.from(new Set([...existing.evidence, note])),
            });
          } else {
            byIp.set(peer.ip, {
              ip: peer.ip,
              hostname: peer.hostname,
              open_ports: [],
              os_guess: peer.os,
              response_time_ms: 0,
              mdns: [],
              evidence: [note],
            });
          }
        }
        return Array.from(byIp.values());
      });
      toast('success', `Added ${peers.length} Tailscale peer${peers.length === 1 ? '' : 's'}`);
    } catch (e) {
      toast('error', `Couldn't reach Tailscale: ${String(e)}`);
    } finally {
      setTailscaleLoading(false);
    }
  }, [toast]);

  const handleExport = useCallback(async () => {
    if (results.length === 0) return;
    try {
      const dest = await save({
        title: 'Export Network Scan',
        defaultPath: 'crossterm-network-scan.json',
        filters: [{ name: 'JSON', extensions: ['json'] }],
      });
      if (!dest) return;
      await writeTextFile(dest, JSON.stringify(results, null, 2));
      toast('success', `Exported ${results.length} host${results.length === 1 ? '' : 's'} to ${dest}`);
    } catch {
      /* ignore user cancellations */
    }
  }, [results, toast]);

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
          (r.mac_address ?? '').toLowerCase().includes(q) ||
          (r.mac_vendor ?? '').toLowerCase().includes(q) ||
          r.open_ports.some((p) => p.service_name.includes(q))
      );
    }

    const sorted = [...filtered];
    const dir = sortDir === 'asc' ? 1 : -1;
    switch (sortBy) {
      case 'hostname':
        sorted.sort((a, b) => dir * (a.hostname ?? a.ip).localeCompare(b.hostname ?? b.ip));
        break;
      case 'name':
        sorted.sort((a, b) => {
          const nameA = deviceLabels[deviceLabelKey(a)] ?? a.hostname ?? a.ip;
          const nameB = deviceLabels[deviceLabelKey(b)] ?? b.hostname ?? b.ip;
          return dir * nameA.localeCompare(nameB);
        });
        break;
      case 'type':
        sorted.sort((a, b) => dir * DEVICE_TYPE_META[classifyDeviceType(a)].label.localeCompare(DEVICE_TYPE_META[classifyDeviceType(b)].label));
        break;
      case 'mac':
        sorted.sort((a, b) => compareNullable(a.mac_address, b.mac_address, dir));
        break;
      case 'vendor':
        sorted.sort((a, b) => compareNullable(a.mac_vendor, b.mac_vendor, dir));
        break;
      case 'os':
        sorted.sort((a, b) => compareNullable(a.os_guess, b.os_guess, dir));
        break;
      case 'ports':
        sorted.sort((a, b) => dir * (a.open_ports.length - b.open_ports.length));
        break;
      case 'response':
        sorted.sort((a, b) => dir * (a.response_time_ms - b.response_time_ms));
        break;
      case 'session':
        sorted.sort((a, b) => compareNullable(a.suggested_session_type, b.suggested_session_type, dir));
        break;
      default: // 'ip'
        sorted.sort((a, b) => dir * (ipToNum(a.ip) - ipToNum(b.ip)));
    }
    return sorted;
  }, [results, filterService, searchFilter, sortBy, sortDir, deviceLabels, deviceLabelKey]);

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
            {scanning && (
              <button
                data-testid="scan-stop-btn"
                onClick={handleStop}
                className="flex items-center gap-1.5 rounded-md border border-border-default px-3 py-1.5 text-sm font-medium text-text-secondary hover:text-status-disconnected hover:border-status-disconnected transition-colors shrink-0"
              >
                <Square size={12} />
                {t('network.stop')}
              </button>
            )}
            <button
              data-testid="tailscale-include-btn"
              onClick={handleIncludeTailscale}
              disabled={tailscaleLoading}
              title="Add your Tailscale tailnet peers — a different address space this scan can't reach on its own, identified via `tailscale status` instead of probing"
              className="flex items-center gap-1.5 rounded-md border border-border-default px-3 py-1.5 text-sm font-medium text-text-secondary hover:text-text-primary hover:border-border-focus transition-colors shrink-0 disabled:cursor-not-allowed disabled:opacity-60"
            >
              {tailscaleLoading ? <Loader2 size={12} className="animate-spin" /> : <Waypoints size={12} />}
              Tailscale
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
            // Horizontal-only overflow: vertical scrolling bubbles up to the
            // panel's own overflow-y-auto container, which is what sticky
            // headers below stick relative to — a nested vertical
            // overflow here would give the `<th>`s their own (non-scrolling)
            // containing block instead and the sticky effect would be inert.
            <div className="overflow-x-auto rounded-md border border-border-default">
              <table className="w-full text-sm">
                <thead>
                  <tr>
                    {(
                      [
                        { key: 'ip' as SortKey, label: t('network.ip') },
                        { key: 'hostname' as SortKey, label: t('network.hostname') },
                        { key: 'name' as SortKey, label: 'Name' },
                        { key: 'type' as SortKey, label: 'Type' },
                        { key: 'mac' as SortKey, label: 'MAC Address' },
                        { key: 'vendor' as SortKey, label: 'Vendor' },
                        { key: 'os' as SortKey, label: t('network.os') },
                        { key: 'ports' as SortKey, label: t('network.openPorts') },
                        { key: 'response' as SortKey, label: t('network.responseTime') },
                        { key: 'session' as SortKey, label: t('network.sessionType') },
                        { key: null, label: '' },
                      ] as { key: SortKey | null; label: string }[]
                    ).map((col, i) => (
                      <th
                        key={i}
                        className={clsx(
                          'sticky top-0 z-10 border-b border-border-subtle bg-surface-secondary px-3 py-2 text-left font-medium text-text-secondary select-none',
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
                  {filteredResults.map((result) => {
                    const hasDetail =
                      result.open_ports.some((p) => p.banner || p.version || p.http_title || p.tls) ||
                      result.mdns.length > 0 ||
                      result.evidence.length > 0;
                    const isExpanded = expandedIp === result.ip;
                    return (
                    <Fragment key={result.ip}>
                      <tr className="border-b border-border-subtle last:border-0 hover:bg-surface-secondary">
                        <td className="px-3 py-2 text-text-primary font-mono text-xs">
                          <button
                            onClick={() => hasDetail && setExpandedIp(isExpanded ? null : result.ip)}
                            disabled={!hasDetail}
                            className={clsx(
                              'flex items-center gap-1',
                              hasDetail ? 'cursor-pointer hover:text-accent-primary' : 'cursor-default'
                            )}
                          >
                            {hasDetail ? (
                              isExpanded ? <ChevronDown size={11} className="shrink-0" /> : <ChevronRight size={11} className="shrink-0" />
                            ) : (
                              <span className="w-[11px] shrink-0" />
                            )}
                            {result.ip}
                          </button>
                        </td>
                        <td className="px-3 py-2 text-text-secondary">{result.hostname ?? '—'}</td>
                        <td className="px-3 py-2 text-text-secondary">
                          {editingLabelKey === deviceLabelKey(result) ? (
                            <input
                              autoFocus
                              value={editingLabelValue}
                              onChange={(e) => setEditingLabelValue(e.target.value)}
                              onKeyDown={(e) => {
                                if (e.key === 'Enter') e.currentTarget.blur();
                                if (e.key === 'Escape') {
                                  cancelLabelEditRef.current = true;
                                  setEditingLabelKey(null);
                                }
                              }}
                              onBlur={() => {
                                const key = deviceLabelKey(result);
                                setEditingLabelKey(null);
                                if (cancelLabelEditRef.current) {
                                  cancelLabelEditRef.current = false;
                                  return;
                                }
                                commitDeviceLabel(key, editingLabelValue);
                              }}
                              className="w-full min-w-0 rounded border border-accent-primary bg-surface-primary px-1 py-0.5 text-xs text-text-primary outline-none"
                            />
                          ) : (
                            <button
                              type="button"
                              onClick={() => {
                                const key = deviceLabelKey(result);
                                setEditingLabelKey(key);
                                setEditingLabelValue(deviceLabels[key] ?? result.hostname ?? '');
                              }}
                              title="Click to set a friendly name"
                              className="group flex w-full items-center gap-1 text-left hover:text-text-primary"
                            >
                              <span className="truncate">{deviceLabels[deviceLabelKey(result)] ?? result.hostname ?? '—'}</span>
                              <Pencil size={10} className="shrink-0 opacity-0 group-hover:opacity-50" />
                            </button>
                          )}
                        </td>
                        <td className="px-3 py-2 text-text-secondary">
                          {(() => {
                            const { label, icon: Icon } = DEVICE_TYPE_META[classifyDeviceType(result)];
                            return (
                              <span className="flex items-center gap-1.5 text-xs" title={label}>
                                <Icon size={12} className="shrink-0 text-text-disabled" />
                                <span className="truncate">{label}</span>
                              </span>
                            );
                          })()}
                        </td>
                        <td className="px-3 py-2 text-text-secondary font-mono text-xs">{result.mac_address ?? '—'}</td>
                        <td className="px-3 py-2 text-text-secondary text-xs">{result.mac_vendor ?? '—'}</td>
                        <td className="px-3 py-2 text-text-secondary">{result.os_guess ?? '—'}</td>
                        <td className="px-3 py-2">
                          <div className="flex flex-wrap gap-1">
                            {result.open_ports.map((p) => (
                              <span
                                key={p.port}
                                title={p.version ?? undefined}
                                className={clsx(
                                  'rounded bg-surface-elevated px-1.5 py-0.5 text-xs text-text-secondary max-w-[16rem] truncate inline-block align-bottom',
                                  SERVICE_PORT_COLORS[p.service_name] ?? ''
                                )}
                              >
                                {p.port}/{p.service_name}
                                {p.version ? ` · ${p.version}` : ''}
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
                      {isExpanded && (
                        <tr className="border-b border-border-subtle bg-surface-sunken">
                          <td colSpan={11} className="px-6 py-3">
                            <div className="flex flex-col gap-3 text-xs">
                              {result.open_ports.some((p) => p.banner || p.version || p.http_title || p.tls) && (
                                <div className="flex flex-col gap-2">
                                  {result.open_ports
                                    .filter((p) => p.banner || p.version || p.http_title || p.tls)
                                    .map((p) => (
                                      <div key={p.port} className="flex flex-col gap-0.5 rounded border border-border-subtle bg-surface-primary p-2">
                                        <span className="font-mono font-medium text-text-primary">
                                          {p.port}/{p.protocol} — {p.service_name}
                                        </span>
                                        {p.banner && (
                                          <span className="font-mono text-text-secondary break-all">banner: {p.banner}</span>
                                        )}
                                        {p.version && !p.banner && (
                                          <span className="text-text-secondary">{p.version}</span>
                                        )}
                                        {p.http_title && (
                                          <span className="text-text-secondary">title: "{p.http_title}"</span>
                                        )}
                                        {p.tls && (
                                          <div className="mt-1 flex flex-col gap-0.5 border-l-2 border-border-subtle pl-2 text-text-secondary">
                                            <span className="flex items-center gap-1 text-text-primary">
                                              <Lock size={10} /> TLS certificate
                                            </span>
                                            {p.tls.subject_cn && <span>subject CN: {p.tls.subject_cn}</span>}
                                            {p.tls.subject_org && <span>subject org: {p.tls.subject_org}</span>}
                                            {p.tls.issuer_org && <span>issuer org: {p.tls.issuer_org}</span>}
                                            {p.tls.san.length > 0 && <span>SAN: {p.tls.san.join(', ')}</span>}
                                            {p.tls.not_after && <span>expires: {p.tls.not_after}</span>}
                                          </div>
                                        )}
                                      </div>
                                    ))}
                                </div>
                              )}
                              {result.mdns.length > 0 && (
                                <div className="flex flex-col gap-1">
                                  <span className="flex items-center gap-1 font-medium text-text-primary">
                                    <Radio size={11} /> mDNS / Bonjour
                                  </span>
                                  {result.mdns.map((m, i) => (
                                    <span key={i} className="font-mono text-text-secondary">
                                      {m.service_type} — "{m.instance_name}"
                                    </span>
                                  ))}
                                </div>
                              )}
                              {result.evidence.length > 0 && (
                                <div className="flex flex-col gap-1">
                                  <span className="flex items-center gap-1 font-medium text-text-primary">
                                    <Info size={11} /> Evidence
                                  </span>
                                  {result.evidence.map((e, i) => (
                                    <span key={i} className="text-text-secondary">• {e}</span>
                                  ))}
                                </div>
                              )}
                            </div>
                          </td>
                        </tr>
                      )}
                    </Fragment>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}

          {/* Save all / export */}
          {results.length > 0 && !scanning && (
            <div className="flex items-center gap-3">
              <button
                onClick={handleSaveAsSessions}
                className="flex items-center gap-2 rounded-md bg-surface-elevated px-3 py-1.5 text-xs text-text-primary hover:bg-surface-secondary transition-colors"
              >
                <Save size={12} />
                {t('network.saveAsSessions')}
              </button>
              <button
                onClick={handleExport}
                className="flex items-center gap-2 rounded-md bg-surface-elevated px-3 py-1.5 text-xs text-text-primary hover:bg-surface-secondary transition-colors"
              >
                <Download size={12} />
                {t('network.export')}
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
