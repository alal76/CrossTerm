import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import '@/i18n';
import NetworkExplorer from '@/components/NetworkTools/NetworkExplorer';
import { invoke } from '@tauri-apps/api/core';

const mockInvoke = vi.mocked(invoke);

describe('NetworkExplorer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the explore heading and CIDR input', () => {
    render(<NetworkExplorer />);
    expect(screen.getByRole('heading', { name: 'Network Explore' })).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(/Enter CIDR range/)
    ).toBeInTheDocument();
  });

  it('disables scan button when CIDR is empty', () => {
    render(<NetworkExplorer />);
    const buttons = screen.getAllByRole('button');
    const scanButton = buttons.find((b) => b.textContent?.includes('Network Explore'));
    expect(scanButton).toBeDisabled();
  });

  it('enables scan button when CIDR is entered', () => {
    render(<NetworkExplorer />);
    const input = screen.getByPlaceholderText(/Enter CIDR range/);
    fireEvent.change(input, { target: { value: '192.168.1.0/24' } });
    const buttons = screen.getAllByRole('button');
    const scanButton = buttons.find((b) => b.textContent?.includes('Network Explore'));
    expect(scanButton).not.toBeDisabled();
  });

  it('invokes network_explore_start on scan', async () => {
    mockInvoke.mockResolvedValueOnce('scan-id-123');
    render(<NetworkExplorer />);
    const input = screen.getByPlaceholderText(/Enter CIDR range/);
    fireEvent.change(input, { target: { value: '10.0.0.0/28' } });
    const buttons = screen.getAllByRole('button');
    const scanButton = buttons.find((b) => b.textContent?.includes('Network Explore'));
    fireEvent.click(scanButton!);

    expect(mockInvoke).toHaveBeenCalledWith('network_explore_start', {
      target: {
        cidr: '10.0.0.0/28',
        services: expect.arrayContaining(['ssh', 'rdp', 'vnc']),
        extra_ports: [],
      },
    });
  });

  it('shows empty state when no results', () => {
    render(<NetworkExplorer />);
    expect(
      screen.getByText(/Enter a CIDR range to discover/)
    ).toBeInTheDocument();
  });

  it('toggles service filters panel', () => {
    render(<NetworkExplorer />);
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
    render(<NetworkExplorer />);
    fireEvent.click(screen.getByText('Service Filters'));
    const extraInput = screen.getByPlaceholderText(/Extra ports/);
    fireEvent.change(extraInput, { target: { value: '2222, 8080' } });
    expect(extraInput).toHaveValue('2222, 8080');
  });
});
