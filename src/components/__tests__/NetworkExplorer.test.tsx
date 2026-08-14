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
});
