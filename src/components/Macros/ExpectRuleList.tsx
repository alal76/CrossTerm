import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2, ToggleLeft, ToggleRight, Eye, AlertCircle } from 'lucide-react';
import type { ExpectRule, ExpectActionType } from '@/types';

const ACTION_TYPES: { value: ExpectActionType; label: string }[] = [
  { value: 'send_text', label: 'Send Text' },
  { value: 'run_macro', label: 'Run Macro' },
  { value: 'notify', label: 'Notify' },
  { value: 'callback', label: 'Callback' },
];

export default function ExpectRuleList() {
  const { t } = useTranslation();
  const [rules, setRules] = useState<ExpectRule[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [newName, setNewName] = useState('');
  const [newPattern, setNewPattern] = useState('');
  const [newActionType, setNewActionType] = useState<ExpectActionType>('send_text');
  const [newActionValue, setNewActionValue] = useState('');
  const [patternError, setPatternError] = useState<string | null>(null);

  const loadRules = useCallback(async () => {
    try {
      const list = await invoke<ExpectRule[]>('expect_rule_list');
      setRules(list);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadRules();
  }, [loadRules]);

  const handleAdd = async () => {
    if (!newName || !newPattern) return;

    const action: ExpectRule['action'] = { type: newActionType };
    switch (newActionType) {
      case 'send_text':
        action.text = newActionValue;
        break;
      case 'run_macro':
        action.macro_id = newActionValue;
        break;
      case 'notify':
        action.message = newActionValue;
        break;
      case 'callback':
        action.event_name = newActionValue;
        break;
    }

    try {
      await invoke('expect_rule_create', {
        name: newName,
        pattern: newPattern,
        action,
      });
      setNewName('');
      setNewPattern('');
      setNewActionValue('');
      setShowForm(false);
      await loadRules();
    } catch (err) {
      setPatternError(String(err));
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await invoke('expect_rule_delete', { id });
      await loadRules();
    } catch {
      // ignore
    }
  };

  const handleToggle = async (id: string, enabled: boolean) => {
    try {
      await invoke('expect_rule_toggle', { id, enabled: !enabled });
      await loadRules();
    } catch {
      // ignore
    }
  };

  const testPattern = () => {
    try {
      new RegExp(newPattern);
      setPatternError(null);
    } catch (e) {
      setPatternError(String(e));
    }
  };

  return (
    <div className="flex flex-col h-full bg-surface-primary p-4 overflow-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-sm font-medium text-text-primary flex items-center gap-1">
          <Eye size={16} />
          {t('macro.expect')}
        </h3>
        <button
          onClick={() => setShowForm(!showForm)}
          className="flex items-center gap-1 px-2 py-1 text-xs rounded bg-interactive-default text-text-inverse hover:bg-interactive-hover transition-colors"
        >
          <Plus size={12} />
          {t('macro.addRule')}
        </button>
      </div>

      {/* Add Form */}
      {showForm && (
        <div className="mb-4 p-3 rounded bg-surface-secondary border border-border-default space-y-2">
          <input
            type="text"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            placeholder="Rule name"
            className="w-full px-2 py-1 text-sm rounded bg-surface-sunken border border-border-default text-text-primary"
          />
          <div className="flex gap-2">
            <input
              type="text"
              value={newPattern}
              onChange={(e) => {
                setNewPattern(e.target.value);
                setPatternError(null);
              }}
              placeholder="Regex pattern"
              className="flex-1 px-2 py-1 text-sm rounded bg-surface-sunken border border-border-default text-text-primary font-mono"
            />
            <button
              onClick={testPattern}
              className="px-2 py-1 text-xs rounded bg-surface-elevated text-text-secondary hover:text-text-primary border border-border-subtle transition-colors"
            >
              Test
            </button>
          </div>
          {patternError && (
            <p className="text-xs text-red-400 flex items-center gap-1">
              <AlertCircle size={11} />
              {patternError}
            </p>
          )}
          <div className="flex gap-2">
            <select
              value={newActionType}
              onChange={(e) => setNewActionType(e.target.value as ExpectActionType)}
              className="px-2 py-1 text-sm rounded bg-surface-sunken border border-border-default text-text-primary"
            >
              {ACTION_TYPES.map((at) => (
                <option key={at.value} value={at.value}>
                  {at.label}
                </option>
              ))}
            </select>
            <input
              type="text"
              value={newActionValue}
              onChange={(e) => setNewActionValue(e.target.value)}
              placeholder="Action value"
              className="flex-1 px-2 py-1 text-sm rounded bg-surface-sunken border border-border-default text-text-primary"
            />
          </div>
          <div className="flex justify-end gap-2">
            <button
              onClick={() => setShowForm(false)}
              className="px-2 py-1 text-xs rounded text-text-secondary hover:text-text-primary transition-colors"
            >
              {t('actions.cancel')}
            </button>
            <button
              onClick={handleAdd}
              className="px-3 py-1 text-xs rounded bg-interactive-default text-text-inverse hover:bg-interactive-hover transition-colors"
            >
              {t('actions.create')}
            </button>
          </div>
        </div>
      )}

      {/* Rules Table */}
      {rules.length === 0 ? (
        <p className="text-sm text-text-secondary text-center py-4">
          No expect rules defined. Add a rule to auto-respond to patterns.
        </p>
      ) : (
        <div className="border border-border-default rounded overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-surface-secondary text-text-secondary text-xs">
                <th className="text-left px-3 py-2">Name</th>
                <th className="text-left px-3 py-2">Pattern</th>
                <th className="text-left px-3 py-2">Action</th>
                <th className="text-center px-3 py-2">Enabled</th>
                <th className="w-10"></th>
              </tr>
            </thead>
            <tbody>
              {rules.map((rule) => (
                <tr
                  key={rule.id}
                  className="border-t border-border-subtle hover:bg-surface-elevated transition-colors"
                >
                  <td className="px-3 py-2 text-text-primary">{rule.name}</td>
                  <td className="px-3 py-2 font-mono text-xs text-text-secondary">
                    {rule.pattern}
                  </td>
                  <td className="px-3 py-2 text-xs text-text-secondary">
                    {rule.action.type}
                  </td>
                  <td className="px-3 py-2 text-center">
                    <button
                      onClick={() => handleToggle(rule.id, rule.enabled)}
                      className="text-text-secondary hover:text-text-primary transition-colors"
                    >
                      {rule.enabled ? (
                        <ToggleRight size={20} className="text-accent-primary" />
                      ) : (
                        <ToggleLeft size={20} />
                      )}
                    </button>
                  </td>
                  <td className="px-3 py-2">
                    <button
                      onClick={() => handleDelete(rule.id)}
                      className="p-1 text-red-400 hover:text-red-300 transition-colors"
                    >
                      <Trash2 size={12} />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
