import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import '@/i18n';
import NetworkExplorer from '@/components/NetworkTools/NetworkExplorer';
import { ToastProvider } from '@/components/Shared/Toast';
import { invoke } from '@tauri-apps/api/core';

const mockInvoke = vi.mocked(invoke);

function renderWithToast(ui: React.ReactElement) {
  return render(<ToastProvider>{ui}</ToastProvider>);
}

describe('NetworkExplorer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
});
