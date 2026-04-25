import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import { Radio, Send } from 'lucide-react';

const MAC_REGEX = /^([0-9A-Fa-f]{2}[:\-.]){5}[0-9A-Fa-f]{2}$/;

export default function WakeOnLan() {
  const { t } = useTranslation();
  const [macAddress, setMacAddress] = useState('');
  const [broadcastIp, setBroadcastIp] = useState('');
  const [sending, setSending] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSend = useCallback(async () => {
    const mac = macAddress.trim();
    if (!mac) return;

    if (!MAC_REGEX.test(mac)) {
      setError('Invalid MAC address. Use format AA:BB:CC:DD:EE:FF');
      return;
    }

    setSending(true);
    setSent(false);
    setError(null);
    try {
      await invoke('network_wol_send', {
        target: {
          mac_address: mac,
          broadcast_ip: broadcastIp.trim() || null,
        },
      });
      setSent(true);
      setTimeout(() => setSent(false), 3000);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setSending(false);
    }
  }, [macAddress, broadcastIp]);

  return (
    <div className="flex flex-col gap-4 p-4">
      <div className="flex items-center gap-2">
        <Radio size={20} className="text-text-secondary" />
        <h2 className="text-base font-semibold text-text-primary">
          {t('network.wol')}
        </h2>
      </div>

      <div className="flex flex-col gap-3">
        <div>
          <label className="mb-1 block text-xs font-medium text-text-secondary">
            {t('network.macAddress')}
          </label>
          <input
            type="text"
            value={macAddress}
            onChange={(e) => { setMacAddress(e.target.value); setError(null); }}
            placeholder="AA:BB:CC:DD:EE:FF"
            className="w-full rounded-md border border-border-default bg-surface-primary px-3 py-2 text-sm text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
            onKeyDown={(e) => e.key === 'Enter' && handleSend()}
          />
        </div>

        <div>
          <label className="mb-1 block text-xs font-medium text-text-secondary">
            Broadcast IP ({t('credentialForm.optional')})
          </label>
          <input
            type="text"
            value={broadcastIp}
            onChange={(e) => setBroadcastIp(e.target.value)}
            placeholder="255.255.255.255"
            className="w-full rounded-md border border-border-default bg-surface-primary px-3 py-2 text-sm text-text-primary placeholder:text-text-disabled focus:border-border-focus focus:outline-none"
          />
        </div>

        <button
          onClick={handleSend}
          disabled={sending || !macAddress.trim()}
          className="flex items-center gap-2 self-start rounded-md bg-interactive-default px-4 py-2 text-sm font-medium text-text-inverse hover:bg-interactive-hover disabled:cursor-not-allowed disabled:bg-interactive-disabled disabled:text-text-disabled"
        >
          <Send size={14} />
          {sending ? t('network.wolSending') : t('network.wolSend')}
        </button>

        {sent && (
          <div className="rounded-md border border-status-connected bg-surface-elevated px-3 py-2 text-sm text-status-connected">
            Magic packet sent to {macAddress}
          </div>
        )}

        {error && (
          <div className="rounded-md border border-status-error bg-red-500/10 px-3 py-2 text-sm text-status-error">
            {error}
          </div>
        )}
      </div>
    </div>
  );
}
