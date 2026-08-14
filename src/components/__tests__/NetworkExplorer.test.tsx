import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import '@/i18n';
import NetworkExplorer from '@/components/NetworkTools/NetworkExplorer';
import { ToastProvider } from '@/components/Shared/Toast';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ExploreResult } from '@/types';

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
        },
      });
    });
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

  it('accepts extra ports input', () => {
    renderWithToast(<NetworkExplorer />);
    fireEvent.click(screen.getByText('Service Filters'));
    const extraInput = screen.getByPlaceholderText(/Extra ports/);
    fireEvent.change(extraInput, { target: { value: '2222, 8080' } });
    expect(extraInput).toHaveValue('2222, 8080');
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

    expect(await screen.findByText('living-room-tv.local')).toBeInTheDocument();
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

    // Friendly name wins over the opaque UUID-based mDNS hostname.
    expect(await screen.findByText('Hall clock')).toBeInTheDocument();
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

    expect(await screen.findByText('nas.local')).toBeInTheDocument();
    expect(screen.getByText('BC:24:11:11:22:33')).toBeInTheDocument();
    expect(screen.getByText('Proxmox Server Solutions GmbH')).toBeInTheDocument();

    // Expand the row: both the enrichment's own evidence and the earlier
    // mDNS-derived record should be present — enrichment must not have
    // clobbered the mDNS data merged in beforehand.
    fireEvent.click(screen.getByText('192.168.1.70'));
    expect(await screen.findByText(/banner: SSH-2\.0-OpenSSH_9\.6/)).toBeInTheDocument();
    expect(screen.getByText(/_ssh\._tcp\.local\./)).toBeInTheDocument();
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

    // Saved label overrides the resolved hostname in the table.
    expect(await screen.findByText('Homelab NAS')).toBeInTheDocument();
    expect(screen.queryByText('homelab.local')).not.toBeInTheDocument();

    // Click into edit mode, change it, commit with Enter.
    fireEvent.click(screen.getByText('Homelab NAS'));
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
    expect(await screen.findByText('homelab.local')).toBeInTheDocument();

    fireEvent.click(screen.getByText('homelab.local'));
    const input = await screen.findByDisplayValue('homelab.local');
    fireEvent.change(input, { target: { value: 'discarded edit' } });
    fireEvent.keyDown(input, { key: 'Escape' });

    expect(await screen.findByText('homelab.local')).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith('settings_update', expect.anything());
  });
});
