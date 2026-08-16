import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FileCode, Eye } from 'lucide-react';
import clsx from 'clsx';
import MacroEditor from './MacroEditor';
import ExpectRuleList from './ExpectRuleList';

type MacrosTab = 'editor' | 'expect';

export default function MacrosPanel() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<MacrosTab>('editor');

  return (
    <div className="flex flex-col h-full bg-surface-primary">
      <div className="flex items-center gap-1 px-3 pt-2 border-b border-border-default">
        <button
          onClick={() => setTab('editor')}
          className={clsx(
            'flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-t border-b-2 transition-colors',
            tab === 'editor'
              ? 'border-accent-primary text-text-primary'
              : 'border-transparent text-text-secondary hover:text-text-primary'
          )}
        >
          <FileCode size={14} />
          {t('macro.editor')}
        </button>
        <button
          onClick={() => setTab('expect')}
          className={clsx(
            'flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-t border-b-2 transition-colors',
            tab === 'expect'
              ? 'border-accent-primary text-text-primary'
              : 'border-transparent text-text-secondary hover:text-text-primary'
          )}
        >
          <Eye size={14} />
          {t('macro.expect')}
        </button>
      </div>
      <div className="flex-1 overflow-hidden">
        {tab === 'editor' ? <MacroEditor /> : <ExpectRuleList />}
      </div>
    </div>
  );
}
