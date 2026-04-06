import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Globe, Upload, Eye, Check, X } from 'lucide-react';
import clsx from 'clsx';
import type { LocaleInfo } from '@/types';

interface LocaleInstallerProps {
  open: boolean;
  onClose: () => void;
  installedLocales?: LocaleInfo[];
}

export default function LocaleInstaller({
  open,
  onClose,
  installedLocales = [],
}: Readonly<LocaleInstallerProps>) {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<Record<string, string> | null>(null);
  const [previewName, setPreviewName] = useState('');

  const handleFileSelect = useCallback(async () => {
    try {
      const input = document.createElement('input');
      input.type = 'file';
      input.accept = '.json';
      input.onchange = async (e) => {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) return;
        const text = await file.text();
        const data = JSON.parse(text) as Record<string, string>;
        setPreview(data);
        setPreviewName(file.name.replace('.json', ''));
      };
      input.click();
    } catch {
      // Handle parse errors silently
    }
  }, []);

  const handleInstall = useCallback(() => {
    if (!preview) return;
    // Stub: In production, save to i18n directory and register with i18next
    setPreview(null);
    setPreviewName('');
  }, [preview]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
      <div className="w-[480px] max-h-[600px] bg-surface-primary rounded-xl shadow-xl border border-border-default flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-default">
          <div className="flex items-center gap-2">
            <Globe size={16} className="text-accent-primary" />
            <h2 className="text-sm font-semibold text-text-primary">
              {t('locale.install')}
            </h2>
          </div>
          <button onClick={onClose} className="p-1 rounded hover:bg-interactive-hover">
            <X size={16} />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4 space-y-4">
          {/* Import Button */}
          <button
            onClick={handleFileSelect}
            className={clsx(
              'flex items-center gap-2 w-full px-4 py-3 rounded-lg',
              'border-2 border-dashed border-border-default',
              'text-text-secondary hover:border-accent-primary hover:text-accent-primary',
              'transition-colors'
            )}
          >
            <Upload size={16} />
            <span className="text-sm">{t('locale.import')}</span>
          </button>

          {/* Preview */}
          {preview && (
            <div className="rounded-lg border border-border-default p-3 space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-sm font-medium text-text-primary flex items-center gap-2">
                  <Eye size={14} />
                  {t('locale.preview')}: {previewName}
                </span>
                <button
                  onClick={handleInstall}
                  className="flex items-center gap-1 px-3 py-1 rounded bg-accent-primary text-text-inverse text-xs"
                >
                  <Check size={12} />
                  {t('plugin.install')}
                </button>
              </div>
              <div className="max-h-48 overflow-y-auto text-xs font-mono text-text-secondary bg-surface-sunken rounded p-2">
                {Object.entries(preview).slice(0, 20).map(([key, value]) => (
                  <div key={key}>
                    <span className="text-accent-primary">{key}</span>: {String(value)}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Installed Locales */}
          <div>
            <h3 className="text-xs font-medium text-text-secondary uppercase mb-2">
              {t('locale.installed')}
            </h3>
            <ul className="space-y-1">
              {installedLocales.map((locale) => (
                <li
                  key={locale.code}
                  className="flex items-center justify-between px-3 py-2 rounded bg-surface-secondary"
                >
                  <div>
                    <span className="text-sm text-text-primary">{locale.native_name}</span>
                    <span className="text-xs text-text-secondary ml-2">({locale.code})</span>
                  </div>
                  <span className="text-xs text-text-secondary">{locale.completeness}%</span>
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </div>
  );
}
