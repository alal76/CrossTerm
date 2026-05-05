import React from 'react';
import { render, screen } from '@testing-library/react';
import { SessionHealthCard } from './SessionHealthCard';

describe('SessionHealthCard', () => {
  it('shows green dot for ok status', () => {
    render(
      <SessionHealthCard
        sessionId="s1"
        sessionName="prod-server"
        status="ok"
      />
    );
    const dot = document.querySelector('.status-dot') as HTMLElement;
    expect(dot.style.background).toBe('rgb(34, 197, 94)');
  });

  it('renders latency correctly', () => {
    render(
      <SessionHealthCard
        sessionId="s1"
        sessionName="prod-server"
        status="ok"
        latencyMs={42}
      />
    );
    expect(screen.getByText('42ms')).toBeTruthy();
  });

  it('shows reconnect badge only when count > 0', () => {
    const { rerender } = render(
      <SessionHealthCard sessionId="s1" sessionName="srv" status="ok" reconnectCount={0} />
    );
    expect(document.querySelector('.reconnect-badge')).toBeNull();

    rerender(
      <SessionHealthCard sessionId="s1" sessionName="srv" status="ok" reconnectCount={3} />
    );
    expect(document.querySelector('.reconnect-badge')).toBeTruthy();
  });
});
