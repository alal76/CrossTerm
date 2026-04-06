import { useTranslation } from 'react-i18next';
import { Eye, FolderSync } from 'lucide-react';

interface SftpToolbarProps {
  onPreview?: () => void;
  onSync?: () => void;
  previewDisabled?: boolean;
  syncDisabled?: boolean;
}

export default function SftpToolbar({
  onPreview,
  onSync,
  previewDisabled = false,
  syncDisabled = false,
}: Readonly<SftpToolbarProps>) {
  const { t } = useTranslation();

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border-default bg-surface-secondary">
      <span className="text-xs font-medium text-text-secondary mr-2">
        {t('sftp.toolbar')}
      </span>
      <button
        onClick={onPreview}
        disabled={previewDisabled}
        className="flex items-center gap-1 px-2 py-1 rounded text-xs text-text-primary hover:bg-interactive-hover disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <Eye size={14} />
        {t('sftp.preview')}
      </button>
      <button
        onClick={onSync}
        disabled={syncDisabled}
        className="flex items-center gap-1 px-2 py-1 rounded text-xs text-text-primary hover:bg-interactive-hover disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <FolderSync size={14} />
        {t('sftp.syncWizard')}
      </button>
    </div>
  );
}
