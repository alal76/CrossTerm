import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import '@/i18n';
import NetworkExplorer from '@/components/NetworkTools/NetworkExplorer';
import { ToastProvider } from '@/components/Shared/Toast';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useSessionStore } from '@/stores/sessionStore';
import { SessionType } from '@/types';
import type { ExploreResult, TailscalePeer } from '@/types';

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: vi.fn(),
}));
vi.mock('@tauri-apps/plugin-fs', () => ({
  writeTextFile: vi.fn(),
}));

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

type EventHandler = (event: { payload: unknown }) => void;
let handlers: Record<string, EventHandler>;

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

const mockResult: ExploreResult = {
  ip: '192.168.1.11',
  hostname: 'homelab.local',
  mac_address: 'BC:24:11:A0:F2:1F',
  mac_vendor: 'Proxmox Server Solutions GmbH',
  os_guess: 'Linux/Unix',
  response_time_ms: 4.5,
  suggested_session_type: 'ssh',
  ttl: 64,
  open_ports: [
    {
      port: 22,
      service_name: 'ssh',
      protocol: 'tcp',
      banner: 'SSH-2.0-OpenSSH_10.0p2 Debian-7',
      version: 'OpenSSH 10.0p2 Debian-7',
    },
    {
      port: 443,
      service_name: 'https',
      protocol: 'tcp',
      http_title: 'Heimdall',
      tls: {
        subject_cn: '*',
        subject_org: 'Linuxserver.io',
        issuer_org: 'Linuxserver.io',
        san: ['localhost'],
        not_after: '2036-01-20T07:43:08',
      },
    },
  ],
  mdns: [{ service_type: '_home-assistant._tcp.local.', instance_name: 'Home', txt: {} }],
  evidence: ['port 22: "OpenSSH 10.0p2 Debian-7"'],
};

describe('NetworkExplorer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    handlers = {};
    mockListen.mockImplementation((event, cb) => {
      handlers[event as string] = cb as EventHandler;
      return Promise.resolve(() => {});
    });
    // Default: network_local_subnets returns empty (no auto-populate)
    mockInvoke.mockResolvedValue([]);
  });

  it('renders the explore heading and CIDR input', () => {
    renderWithToast(<NetworkExplorer />);
    expect(screen.getByRole('heading', { level: 2 })).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(/192\.168/)
    ).toBeInTheDocument();
  });

  it('disables scan button when CIDR is empty', () => {
    renderWithToast(<NetworkExplorer />);
    expect(screen.getByTestId('scan-start-btn')).toBeDisabled();
  });

  it('enables scan button when CIDR is entered', () => {
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '192.168.1.0/24' } });
    expect(screen.getByTestId('scan-start-btn')).not.toBeDisabled();
  });

  it('invokes network_explore_start on scan', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', {
        target: {
          cidr: '10.0.0.0/28',
          services: expect.arrayContaining(['ssh', 'rdp', 'vnc']),
          extra_ports: [],
          snmp_communities: [],
          extra_vendor_hints: [],
        },
      });
    });
  });

  it('does not launch a second overlapping scan when Enter is pressed again while one is already running', async () => {
    // Regression: the Scan button's disabled={scanning} only guarded
    // clicks — the CIDR input's Enter-key handler called handleScan()
    // directly with no such check, so pressing Enter twice launched a
    // second full scan on top of the first. Both then split the shared
    // backend concurrency pool, degrading results for both (confirmed: the
    // same subnet scanned alone found 31-32 real hosts, scanned again on
    // top of itself found as few as 0-11).
    let exploreStartCalls = 0;
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') {
        exploreStartCalls++;
        return Promise.resolve(`scan-id-${String(exploreStartCalls)}`);
      }
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    const input = screen.getByPlaceholderText(/192\.168/);
    fireEvent.change(input, { target: { value: '10.0.0.0/28' } });

    fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(exploreStartCalls).toBe(1));
    fireEvent.keyDown(input, { key: 'Enter' });

    // Give any accidental second call a chance to fire before asserting.
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(exploreStartCalls).toBe(1);
  });

  it('shows empty state when no results', () => {
    renderWithToast(<NetworkExplorer />);
    expect(
      screen.getByText(/Enter a CIDR range to discover/)
    ).toBeInTheDocument();
  });

  it('toggles service filters panel', () => {
    renderWithToast(<NetworkExplorer />);
    const filterButton = screen.getByText('Service Filters');
    fireEvent.click(filterButton);
    expect(screen.getByText('SSH (22)')).toBeInTheDocument();
    expect(screen.getByText('RDP (3389)')).toBeInTheDocument();
    expect(screen.getByText('VNC (5900)')).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(/Extra ports/)
    ).toBeInTheDocument();
  });

  // Regression coverage: NetworkScanner.tsx and WakeOnLan.tsx were fully
  // built and tested but had no tab to render under in NetworkExplorer.
  it('Quick Scan and Wake on LAN tabs render the real NetworkScanner and WakeOnLan components', () => {
    renderWithToast(<NetworkExplorer />);

    fireEvent.click(screen.getByText('Quick Scan'));
    expect(screen.getByText('Network Scanner')).toBeInTheDocument();

    fireEvent.click(screen.getByText('Wake on LAN'));
    expect(screen.getByText('Wake-on-LAN')).toBeInTheDocument();
    expect(screen.getByText('MAC Address')).toBeInTheDocument();
  });

  it('accepts extra ports input', () => {
    renderWithToast(<NetworkExplorer />);
    fireEvent.click(screen.getByText('Service Filters'));
    const extraInput = screen.getByPlaceholderText(/Extra ports/);
    fireEvent.change(extraInput, { target: { value: '2222, 8080' } });
    expect(extraInput).toHaveValue('2222, 8080');
  });

  it('persists extra ports, SNMP communities, and vendor hints to settings on blur, and reloads them on mount', async () => {
    let savedSettings: Record<string, unknown> = {};
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'settings_get') return Promise.resolve(savedSettings);
      if (cmd === 'settings_update') {
        savedSettings = (args as { settings?: Record<string, unknown> } | undefined)?.settings ?? savedSettings;
        return Promise.resolve(undefined);
      }
      return Promise.resolve(undefined);
    });
    const { unmount } = renderWithToast(<NetworkExplorer />);
    fireEvent.click(screen.getByText('Service Filters'));

    const extraPortsField = screen.getByPlaceholderText(/Extra ports/);
    fireEvent.change(extraPortsField, { target: { value: '9000, 9001' } });
    fireEvent.blur(extraPortsField);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('settings_update', {
        settings: expect.objectContaining({ network_scan_extra_ports: '9000, 9001' }),
      }),
    );

    const communitiesField = screen.getByPlaceholderText(/SNMP communities/);
    fireEvent.change(communitiesField, { target: { value: 'private' } });
    fireEvent.blur(communitiesField);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('settings_update', {
        settings: expect.objectContaining({ network_scan_snmp_communities: 'private' }),
      }),
    );

    const vendorHintsField = screen.getByPlaceholderText(/vendor keywords/);
    fireEvent.change(vendorHintsField, { target: { value: 'HIKVISION, QNAP' } });
    fireEvent.blur(vendorHintsField);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('settings_update', {
        settings: expect.objectContaining({ network_scan_vendor_hints: 'HIKVISION, QNAP' }),
      }),
    );

    unmount();
    renderWithToast(<NetworkExplorer />);
    fireEvent.click(screen.getByText('Service Filters'));
    expect(await screen.findByPlaceholderText(/Extra ports/)).toHaveValue('9000, 9001');
    expect(screen.getByPlaceholderText(/SNMP communities/)).toHaveValue('private');
    expect(screen.getByPlaceholderText(/vendor keywords/)).toHaveValue('HIKVISION, QNAP');
  });

  it('shows a Stop button while scanning and calls network_explore_stop', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));

    fireEvent.click(await screen.findByTestId('scan-stop-btn'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('network_explore_stop', { scanId: 'scan-id-123' });
    });
    // Stopping clears scanning state immediately.
    expect(screen.queryByTestId('scan-stop-btn')).not.toBeInTheDocument();
  });

  it('renders a discovered host and expands it to show banner, TLS, mDNS and evidence detail', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    act(() => {
      handlers['network:explore_host_found']({
        payload: { scan_id: 'scan-id-123', result: mockResult },
      });
    });

    expect(await screen.findByText('192.168.1.11')).toBeInTheDocument();
    // Detail is collapsed by default.
    expect(screen.queryByText(/banner: SSH-2\.0-OpenSSH/)).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('192.168.1.11'));

    expect(await screen.findByText(/banner: SSH-2\.0-OpenSSH/)).toBeInTheDocument();
    expect(screen.getByText(/subject org: Linuxserver\.io/)).toBeInTheDocument();
    expect(screen.getByText(/_home-assistant\._tcp\.local\./)).toBeInTheDocument();
    expect(screen.getByText(/port 22: "OpenSSH 10\.0p2 Debian-7"/)).toBeInTheDocument();
  });

  it('does not drop a straggling host whose explore_host_found lands after progress hits 100%', async () => {
    // Regression test: `hosts_scanned` advances at each host's fast pass,
    // which races that same host's own `explore_host_found` emission fired
    // right after. A host still mid-flight when the *last* (often a fast,
    // not-found) host ticks the counter to total_hosts must not be dropped
    // just because its scan_id would otherwise have already been retired.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    act(() => {
      handlers['network:explore_host_found']({
        payload: { scan_id: 'scan-id-123', result: mockResult },
      });
    });
    expect(await screen.findByText('192.168.1.11')).toBeInTheDocument();

    // Progress reaches 100% — this used to retire the scan_id from the
    // active set and silently swallow any still-in-flight found events.
    act(() => {
      handlers['network:explore_progress']({
        payload: { scan_id: 'scan-id-123', hosts_scanned: 16, total_hosts: 16, hosts_found: 2 },
      });
    });

    const strayResult: ExploreResult = { ...mockResult, ip: '192.168.1.99', hostname: undefined, mdns: [] };
    act(() => {
      handlers['network:explore_host_found']({
        payload: { scan_id: 'scan-id-123', result: strayResult },
      });
    });

    expect(await screen.findByText('192.168.1.99')).toBeInTheDocument();
  });

  it('merges an mDNS update event into an already-rendered row by IP', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    const noMdnsResult: ExploreResult = { ...mockResult, open_ports: [], mdns: [], evidence: [] };
    act(() => {
      handlers['network:explore_host_found']({
        payload: { scan_id: 'scan-id-123', result: noMdnsResult },
      });
    });
    expect(await screen.findByText('192.168.1.11')).toBeInTheDocument();
    // No detail available yet, so the row isn't expandable.
    fireEvent.click(screen.getByText('192.168.1.11'));
    expect(screen.queryByText(/_home-assistant/)).not.toBeInTheDocument();

    act(() => {
      handlers['network:explore_mdns_update']({
        payload: {
          scan_id: 'scan-id-123',
          records: { '192.168.1.11': [{ service_type: '_home-assistant._tcp.local.', instance_name: 'Home', txt: {} }] },
        },
      });
    });

    fireEvent.click(screen.getByText('192.168.1.11'));
    expect(await screen.findByText(/_home-assistant\._tcp\.local\./)).toBeInTheDocument();
  });

  it('applies a buffered mDNS update to a host row that arrives afterward, deriving its hostname', async () => {
    // Regression test: the mDNS browse window (~4s) typically closes well
    // before a full CIDR sweep finishes, so explore_mdns_update usually
    // arrives *before* explore_host_found for most hosts in the scan. The
    // records must not be dropped just because there's no row yet to merge
    // into, and a host with no reverse-DNS/NetBIOS/ARP/TLS hostname should
    // fall back to the mDNS-advertised name.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    act(() => {
      handlers['network:explore_mdns_update']({
        payload: {
          scan_id: 'scan-id-123',
          records: {
            '192.168.1.50': [{ service_type: '_googlecast._tcp.local.', instance_name: 'Living Room TV', hostname: 'living-room-tv.local', txt: {} }],
          },
        },
      });
    });

    const noHostnameResult: ExploreResult = {
      ...mockResult,
      ip: '192.168.1.50',
      hostname: undefined,
      open_ports: [],
      mdns: [],
      evidence: [],
    };
    act(() => {
      handlers['network:explore_host_found']({
        payload: { scan_id: 'scan-id-123', result: noHostnameResult },
      });
    });

    // Shows in both the read-only Hostname column and the Name column
    // (which defaults to the hostname until a custom label is set).
    await waitFor(() => expect(screen.getAllByText('living-room-tv.local')).toHaveLength(2));
    fireEvent.click(screen.getByText('192.168.1.50'));
    expect(await screen.findByText(/_googlecast\._tcp\.local\./)).toBeInTheDocument();
  });

  it('prefers a Cast friendly name over an opaque mDNS hostname and surfaces the self-reported model as evidence', async () => {
    // A MAC's OUI only tells you who registered the NIC (e.g. "Motorola
    // (Wuhan) Mobility Technologies" — Lenovo's OEM/manufacturing arm), not
    // the product. Google Cast TXT records self-report both a friendly name
    // ("fn") and a model string ("md") that identify the device far better.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    const noHostnameResult: ExploreResult = {
      ...mockResult,
      ip: '192.168.1.66',
      hostname: undefined,
      mac_vendor: 'Motorola (Wuhan) Mobility Technologies Communication Co., Ltd.',
      open_ports: [],
      mdns: [],
      evidence: ['ping TTL 64'],
    };
    act(() => {
      handlers['network:explore_host_found']({
        payload: { scan_id: 'scan-id-123', result: noHostnameResult },
      });
    });
    expect(await screen.findByText('192.168.1.66')).toBeInTheDocument();

    act(() => {
      handlers['network:explore_mdns_update']({
        payload: {
          scan_id: 'scan-id-123',
          records: {
            '192.168.1.66': [{
              service_type: '_googlecast._tcp.local.',
              instance_name: 'LenovoCD-24502F-453e6721',
              hostname: '453e6721-d97d-f36c-d859.local',
              txt: { fn: 'Hall clock', md: 'LenovoCD-24502F' },
            }],
          },
        },
      });
    });

    // Friendly name wins over the opaque UUID-based mDNS hostname, and
    // shows in both the Hostname and (default-to-hostname) Name columns.
    await waitFor(() => expect(screen.getAllByText('Hall clock')).toHaveLength(2));
    expect(screen.queryByText('453e6721-d97d-f36c-d859.local')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('192.168.1.66'));
    expect(await screen.findByText(/model "LenovoCD-24502F"/)).toBeInTheDocument();
  });

  it('sorts by every column, always pushing missing values to the end', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    const hosts: ExploreResult[] = [
      { ...mockResult, ip: '192.168.1.10', mac_vendor: 'Zyxel Communications', open_ports: [], mdns: [], evidence: [] },
      { ...mockResult, ip: '192.168.1.20', mac_vendor: undefined, open_ports: [], mdns: [], evidence: [] },
      { ...mockResult, ip: '192.168.1.30', mac_vendor: 'Apple, Inc.', open_ports: [], mdns: [], evidence: [] },
    ];
    act(() => {
      for (const result of hosts) {
        handlers['network:explore_host_found']({ payload: { scan_id: 'scan-id-123', result } });
      }
    });
    expect(await screen.findByText('192.168.1.10')).toBeInTheDocument();

    const ipCellsInOrder = () =>
      Array.from(document.querySelectorAll('tbody tr td:first-child')).map((td) => td.textContent?.trim());

    fireEvent.click(screen.getByText('Vendor'));
    expect(ipCellsInOrder()).toEqual(['192.168.1.30', '192.168.1.10', '192.168.1.20']); // Apple, Zyxel, then missing last

    fireEvent.click(screen.getByText('Vendor')); // toggle to desc
    expect(ipCellsInOrder()).toEqual(['192.168.1.10', '192.168.1.30', '192.168.1.20']); // Zyxel, Apple, missing still last
  });

  it('shows a host immediately with bare ports, then fills in hostname/MAC/vendor from a later enrichment event', async () => {
    // The backend now emits explore_host_found the moment the fast port
    // scan/ping finishes (no banner/hostname/MAC/vendor yet), then streams
    // the slower enrichment in via explore_host_enriched — so scan results
    // don't wait on the tail latency of ARP/hostname resolution.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    const bareResult: ExploreResult = {
      ip: '192.168.1.70',
      hostname: undefined,
      mac_address: undefined,
      mac_vendor: undefined,
      os_guess: 'Linux/macOS/BSD-like (TTL 64)',
      response_time_ms: 12,
      suggested_session_type: 'ssh',
      ttl: 64,
      open_ports: [{ port: 22, service_name: 'ssh', protocol: 'tcp' }],
      mdns: [],
      evidence: ['ping TTL 64'],
    };
    act(() => {
      handlers['network:explore_host_found']({ payload: { scan_id: 'scan-id-123', result: bareResult } });
    });
    expect(await screen.findByText('192.168.1.70')).toBeInTheDocument();
    // No hostname yet — the IP itself is what's showing in the hostname cell.
    expect(screen.queryByText('nas.local')).not.toBeInTheDocument();

    // mDNS arrives first (its 4s window typically closes before enrichment
    // for a slow host finishes) — must survive being overwritten below.
    act(() => {
      handlers['network:explore_mdns_update']({
        payload: {
          scan_id: 'scan-id-123',
          records: { '192.168.1.70': [{ service_type: '_ssh._tcp.local.', instance_name: 'nas', txt: {} }] },
        },
      });
    });

    const enrichedResult: ExploreResult = {
      ...bareResult,
      hostname: 'nas.local',
      mac_address: 'BC:24:11:11:22:33',
      mac_vendor: 'Proxmox Server Solutions GmbH',
      open_ports: [{ port: 22, service_name: 'ssh', protocol: 'tcp', banner: 'SSH-2.0-OpenSSH_9.6', version: 'OpenSSH 9.6' }],
      evidence: ['port 22: "OpenSSH 9.6"'],
    };
    act(() => {
      handlers['network:explore_host_enriched']({ payload: { scan_id: 'scan-id-123', result: enrichedResult } });
    });

    await waitFor(() => expect(screen.getAllByText('nas.local')).toHaveLength(2)); // Hostname + Name columns
    expect(screen.getByText('BC:24:11:11:22:33')).toBeInTheDocument();
    expect(screen.getByText('Proxmox Server Solutions GmbH')).toBeInTheDocument();

    // Expand the row: both the enrichment's own evidence and the earlier
    // mDNS-derived record should be present — enrichment must not have
    // clobbered the mDNS data merged in beforehand.
    fireEvent.click(screen.getByText('192.168.1.70'));
    expect(await screen.findByText(/banner: SSH-2\.0-OpenSSH_9\.6/)).toBeInTheDocument();
    expect(screen.getByText(/_ssh\._tcp\.local\./)).toBeInTheDocument();
  });

  it('migrates a friendly name from the IP key to the MAC key once enrichment resolves a MAC (regression: label was silently lost on reconnect)', async () => {
    // Reproduces a real regression: a device is renamed while its row still
    // only has an IP (before the async MAC/hostname resolver finishes), so
    // the label saves keyed by IP. Once enrichment resolves a MAC, the
    // device's lookup key switches to that MAC — without a migration, the
    // label becomes permanently unreachable under the old IP key, which
    // looked like "the custom name doesn't persist across sessions".
    let savedSettings: Record<string, unknown> = {};
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      if (cmd === 'settings_get') return Promise.resolve(savedSettings);
      if (cmd === 'settings_update') {
        savedSettings = (args as { settings?: Record<string, unknown> } | undefined)?.settings ?? savedSettings;
        return Promise.resolve(undefined);
      }
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    const bareResult: ExploreResult = {
      ip: '192.168.1.80',
      hostname: undefined,
      mac_address: undefined,
      mac_vendor: undefined,
      os_guess: 'Linux/macOS/BSD-like (TTL 64)',
      response_time_ms: 12,
      suggested_session_type: 'ssh',
      ttl: 64,
      open_ports: [{ port: 22, service_name: 'ssh', protocol: 'tcp' }],
      mdns: [],
      evidence: [],
    };
    act(() => {
      handlers['network:explore_host_found']({ payload: { scan_id: 'scan-id-123', result: bareResult } });
    });
    await screen.findByText('192.168.1.80');

    // Rename the device before its MAC has resolved.
    fireEvent.click(screen.getByTitle('Click to set a friendly name'));
    const input = document.querySelector<HTMLInputElement>('input.border-accent-primary')!;
    fireEvent.change(input, { target: { value: 'Garage Pi' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('settings_update', {
        settings: expect.objectContaining({ network_device_labels: { '192.168.1.80': 'Garage Pi' } }),
      }),
    );
    expect(await screen.findByText('Garage Pi')).toBeInTheDocument();

    // Enrichment now resolves a MAC for the same host.
    act(() => {
      handlers['network:explore_host_enriched']({
        payload: {
          scan_id: 'scan-id-123',
          result: { ...bareResult, mac_address: 'AA:BB:CC:DD:EE:FF', mac_vendor: 'Raspberry Pi Foundation' },
        },
      });
    });

    // The label must survive the key switch — and settings must be rewritten
    // keyed by MAC, with the stale IP entry removed, not just left in memory.
    expect(await screen.findByText('Garage Pi')).toBeInTheDocument();
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('settings_update', {
        settings: expect.objectContaining({ network_device_labels: { 'AA:BB:CC:DD:EE:FF': 'Garage Pi' } }),
      }),
    );
  });

  it('loads a saved friendly name from settings and lets it be edited inline', async () => {
    const existingSettings = {
      theme: 'dark',
      network_device_labels: { 'BC:24:11:A0:F2:1F': 'Homelab NAS' },
    };
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      if (cmd === 'settings_get') return Promise.resolve(existingSettings);
      if (cmd === 'settings_update') return Promise.resolve(undefined);
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    act(() => {
      handlers['network:explore_host_found']({
        payload: { scan_id: 'scan-id-123', result: { ...mockResult, open_ports: [], mdns: [], evidence: [] } },
      });
    });

    // Saved label shows in the editable Name column; the read-only
    // Hostname column still shows the actual resolved hostname alongside it.
    expect(await screen.findByText('Homelab NAS')).toBeInTheDocument();
    expect(screen.getByText('homelab.local')).toBeInTheDocument();

    // Click into edit mode (the Name column's button, not the plain-text
    // Hostname cell), change it, commit with Enter.
    fireEvent.click(screen.getByRole('button', { name: 'Homelab NAS' }));
    const input = await screen.findByDisplayValue('Homelab NAS');
    fireEvent.change(input, { target: { value: 'Living Room NAS' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('settings_update', {
        settings: expect.objectContaining({
          network_device_labels: { 'BC:24:11:A0:F2:1F': 'Living Room NAS' },
        }),
      });
    });
    expect(await screen.findByText('Living Room NAS')).toBeInTheDocument();
  });

  it('cancels an in-progress label edit on Escape without saving', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      if (cmd === 'settings_get') return Promise.resolve({});
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    act(() => {
      handlers['network:explore_host_found']({
        payload: { scan_id: 'scan-id-123', result: { ...mockResult, open_ports: [], mdns: [], evidence: [] } },
      });
    });
    // Shows in both the read-only Hostname column and the (default-to-
    // hostname) Name column.
    await waitFor(() => expect(screen.getAllByText('homelab.local')).toHaveLength(2));

    // The Name column's button is the editable one.
    fireEvent.click(screen.getByRole('button', { name: 'homelab.local' }));
    const input = await screen.findByDisplayValue('homelab.local');
    fireEvent.change(input, { target: { value: 'discarded edit' } });
    fireEvent.keyDown(input, { key: 'Escape' });

    await waitFor(() => expect(screen.getAllByText('homelab.local')).toHaveLength(2));
    expect(mockInvoke).not.toHaveBeenCalledWith('settings_update', expect.anything());
  });

  it('classifies device type from open ports, mDNS, hostname, and vendor signals', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    const hosts: ExploreResult[] = [
      // Camera: RTSP port open.
      { ...mockResult, ip: '192.168.1.20', open_ports: [{ port: 554, service_name: 'rtsp', protocol: 'tcp' }], mdns: [], evidence: [] },
      // Router/gateway: conventional .1 with an admin HTTP port and a known router-vendor OUI.
      { ...mockResult, ip: '192.168.1.1', mac_vendor: 'Sagemcom Broadband SAS', open_ports: [{ port: 80, service_name: 'http', protocol: 'tcp' }], mdns: [], evidence: [] },
      // VM: Proxmox virtual-NIC vendor.
      { ...mockResult, ip: '192.168.1.30', mac_vendor: 'Proxmox Server Solutions GmbH', open_ports: [], mdns: [], evidence: [] },
      // NAS/server: Jellyfin identified via enrich_port's version string.
      { ...mockResult, ip: '192.168.1.40', mac_vendor: undefined, open_ports: [{ port: 8096, service_name: 'jellyfin', protocol: 'tcp', version: 'Jellyfin: nl.jellyfin' }], mdns: [], evidence: [] },
    ];
    act(() => {
      for (const result of hosts) {
        handlers['network:explore_host_found']({ payload: { scan_id: 'scan-id-123', result } });
      }
    });

    expect(await screen.findByText('Camera')).toBeInTheDocument();
    expect(screen.getByText('Router/Gateway')).toBeInTheDocument();
    expect(screen.getByText('Virtual Machine')).toBeInTheDocument();
    expect(screen.getByText('NAS/Server')).toBeInTheDocument();
  });

  it('fetches and merges Tailscale peers, tagging an already-scanned host rather than duplicating it', async () => {
    const peers: TailscalePeer[] = [
      { ip: '100.100.111.101', hostname: 'newserver.tailc76fbd.ts.net', os: 'linux', online: true, is_self: false },
      { ip: '100.79.163.121', hostname: 'redmi-pad-pro-5g.tailc76fbd.ts.net', os: 'android', online: false, is_self: false },
    ];
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      if (cmd === 'network_tailscale_peers') return Promise.resolve(peers);
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    // A LAN host that happens to already have a resolved hostname — its
    // Tailscale IP differs from its LAN IP, so this merge is by whatever
    // real identity information is available, not IP matching in this case.
    act(() => {
      handlers['network:explore_host_found']({
        payload: { scan_id: 'scan-id-123', result: { ...mockResult, ip: '192.168.1.11', open_ports: [], mdns: [], evidence: [] } },
      });
    });
    expect(await screen.findByText('192.168.1.11')).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('tailscale-include-btn'));

    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_tailscale_peers'));
    expect(await screen.findByText('100.100.111.101')).toBeInTheDocument();
    expect(screen.getByText('100.79.163.121')).toBeInTheDocument();
    // Both the original LAN row and the two new Tailscale-only rows are present.
    expect(screen.getByText('192.168.1.11')).toBeInTheDocument();
  });

  // Regression coverage: several suggested_session_type values the backend
  // can now return (smb, ftp, nfs, rlogin, proxmox, snmp, ipmi, redfish,
  // webdav) had no entry in the frontend's SESSION_TYPE_MAP at all — the
  // scan would detect and label the service, but clicking Connect just
  // said "not directly connectable" instead of opening a session.
  it.each([
    ['smb', SessionType.Smb, 445],
    ['ftp', SessionType.Ftp, 21],
    ['nfs', SessionType.NfsExplorer, 2049],
    ['rlogin', SessionType.Rlogin, 513],
    ['proxmox', SessionType.ProxmoxConsole, 8006],
    ['snmp', SessionType.Snmp, 161],
    ['ipmi', SessionType.IpmiSol, 623],
  ])('Connect opens a %s session on the real detected port', async (svcType, expectedType, port) => {
    useSessionStore.setState({ sessions: [], openTabs: [], activeTabId: null });
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'network_local_subnets') return Promise.resolve([]);
      if (cmd === 'network_explore_start') return Promise.resolve('scan-id-123');
      return Promise.resolve(undefined);
    });
    renderWithToast(<NetworkExplorer />);
    fireEvent.change(screen.getByPlaceholderText(/192\.168/), { target: { value: '10.0.0.0/28' } });
    fireEvent.click(screen.getByTestId('scan-start-btn'));
    await waitFor(() => expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', expect.anything()));

    const result: ExploreResult = {
      ip: '192.168.1.50',
      hostname: undefined,
      mac_address: undefined,
      mac_vendor: undefined,
      os_guess: 'Linux/macOS/BSD-like (TTL 64)',
      response_time_ms: 5,
      suggested_session_type: svcType,
      ttl: 64,
      open_ports: [{ port, service_name: svcType, protocol: svcType === 'snmp' || svcType === 'ipmi' ? 'udp' : 'tcp' }],
      mdns: [],
      evidence: [],
    };
    act(() => {
      handlers['network:explore_host_found']({ payload: { scan_id: 'scan-id-123', result } });
    });
    await screen.findByText('192.168.1.50');

    fireEvent.click(screen.getByText('Connect'));

    await waitFor(() => {
      const sessions = useSessionStore.getState().sessions;
      expect(sessions).toHaveLength(1);
      expect(sessions[0].type).toBe(expectedType);
      expect(sessions[0].connection.port).toBe(port);
    });
  });
});
