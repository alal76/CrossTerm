import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranslation } from 'react-i18next';
import {
  Play,
  Square,
  Pause,
  RotateCcw,
  Plus,
  Trash2,
  GripVertical,
  Terminal,
  Clock,
  Eye,
  Variable,
  GitBranch,
  Repeat,
  FileCode,
} from 'lucide-react';
import clsx from 'clsx';
import type { MacroInfo, MacroStep, MacroStepType, MacroExecution } from '@/types';

const STEP_TYPE_OPTIONS: { value: MacroStepType; label: string; icon: React.ReactNode }[] = [
  { value: 'send', label: 'Send', icon: <Terminal size={14} /> },
  { value: 'expect', label: 'Expect', icon: <Eye size={14} /> },
  { value: 'wait', label: 'Wait', icon: <Clock size={14} /> },
  { value: 'set_variable', label: 'Set Variable', icon: <Variable size={14} /> },
  { value: 'conditional', label: 'Conditional', icon: <GitBranch size={14} /> },
  { value: 'loop', label: 'Loop', icon: <Repeat size={14} /> },
];

export default function MacroEditor() {
  const { t } = useTranslation();
  const [macros, setMacros] = useState<MacroInfo[]>([]);
  const [selectedMacro, setSelectedMacro] = useState<MacroInfo | null>(null);
  const [execution, setExecution] = useState<MacroExecution | null>(null);
  const [macroName, setMacroName] = useState('');
  const [steps, setSteps] = useState<MacroStep[]>([]);
  const loadMacros = useCallback(async () => {
    try {
      const list = await invoke<MacroInfo[]>('macro_list');
      setMacros(list);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    loadMacros();
  }, [loadMacros]);

  const selectMacro = (m: MacroInfo) => {
    setSelectedMacro(m);
    setMacroName(m.name);
    setSteps([...m.steps]);
  };

  const handleCreate = async () => {
    try {
      const created = await invoke<MacroInfo>('macro_create', {
        name: macroName || 'New Macro',
        steps,
      });
      await loadMacros();
      setSelectedMacro(created);
    } catch {
      // ignore
    }
  };

  const handleSave = async () => {
    if (!selectedMacro) return;
    try {
      const updated = await invoke<MacroInfo>('macro_update', {
        id: selectedMacro.id,
        name: macroName,
        steps,
      });
      await loadMacros();
      setSelectedMacro(updated);
    } catch {
      // ignore
    }
  };

  const handleDelete = async () => {
    if (!selectedMacro) return;
    try {
      await invoke('macro_delete', { id: selectedMacro.id });
      setSelectedMacro(null);
      setSteps([]);
      setMacroName('');
      await loadMacros();
    } catch {
      // ignore
    }
  };

  const handleRun = async () => {
    if (!selectedMacro) return;
    try {
      const exec = await invoke<MacroExecution>('macro_execute', {
        macroId: selectedMacro.id,
        sessionId: 'current',
      });
      setExecution(exec);
    } catch {
      // ignore
    }
  };

  const handleStop = async () => {
    if (!execution) return;
    try {
      await invoke('macro_cancel', { executionId: execution.id });
      setExecution(null);
    } catch {
      // ignore
    }
  };

  const handlePause = async () => {
    if (!execution) return;
    try {
      await invoke('macro_pause', { executionId: execution.id });
      setExecution({ ...execution, status: 'paused' });
    } catch {
      // ignore
    }
  };

  const handleResume = async () => {
    if (!execution) return;
    try {
      await invoke('macro_resume', { executionId: execution.id });
      setExecution({ ...execution, status: 'running' });
    } catch {
      // ignore
    }
  };

  const addStep = (stepType: MacroStepType) => {
    const newStep: MacroStep = { type: stepType };
    switch (stepType) {
      case 'send':
        newStep.data = '';
        break;
      case 'expect':
        newStep.pattern = '';
        newStep.timeout_ms = 5000;
        break;
      case 'wait':
        newStep.duration_ms = 1000;
        break;
      case 'set_variable':
        newStep.name = '';
        newStep.value = '';
        break;
      case 'conditional':
        newStep.condition = '';
        newStep.then_steps = [];
        newStep.else_steps = [];
        break;
      case 'loop':
        newStep.count = 1;
        newStep.steps = [];
        break;
    }
    setSteps([...steps, newStep]);
  };

  const removeStep = (index: number) => {
    setSteps(steps.filter((_, i) => i !== index));
  };

  const updateStep = (index: number, updates: Partial<MacroStep>) => {
    setSteps(steps.map((s, i) => (i === index ? { ...s, ...updates } : s)));
  };

  const stepLabel = (step: MacroStep): string => {
    switch (step.type) {
      case 'send':
        return `Send: ${step.data || '(empty)'}`;
      case 'expect':
        return `Expect: ${step.pattern || '(empty)'}`;
      case 'wait':
        return `Wait: ${step.duration_ms || 0}ms`;
      case 'set_variable':
        return `Set: ${step.name || '?'} = ${step.value || step.from_capture || '?'}`;
      case 'conditional':
        return `If: ${step.condition || '?'}`;
      case 'loop':
        return `Loop: ${step.count || 0}x`;
      default:
        return step.type;
    }
  };

  return (
    <div className="flex h-full bg-surface-primary">
      {/* Macro List */}
      <div className="w-56 border-r border-border-default flex flex-col">
        <div className="flex items-center justify-between p-3 border-b border-border-default">
          <h3 className="text-sm font-medium text-text-primary flex items-center gap-1">
            <FileCode size={14} />
            {t('macro.editor')}
          </h3>
          <button
            onClick={() => {
              setSelectedMacro(null);
              setMacroName('');
              setSteps([]);
            }}
            className="p-1 rounded hover:bg-surface-elevated text-text-secondary hover:text-text-primary transition-colors"
            title={t('macro.create')}
          >
            <Plus size={14} />
          </button>
        </div>
        <div className="flex-1 overflow-auto">
          {macros.length === 0 ? (
            <p className="p-3 text-xs text-text-secondary">{t('macro.noMacros')}</p>
          ) : (
            macros.map((m) => (
              <button
                key={m.id}
                onClick={() => selectMacro(m)}
                className={clsx(
                  'w-full text-left px-3 py-2 text-sm border-b border-border-subtle transition-colors',
                  selectedMacro?.id === m.id
                    ? 'bg-interactive-default/10 text-text-primary'
                    : 'text-text-secondary hover:bg-surface-elevated'
                )}
              >
                {m.name}
                <span className="block text-xs text-text-disabled">
                  {m.steps.length} {t('macro.steps').toLowerCase()}
                </span>
              </button>
            ))
          )}
        </div>
      </div>

      {/* Editor */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Toolbar */}
        <div className="flex items-center gap-2 p-3 border-b border-border-default">
          <input
            type="text"
            value={macroName}
            onChange={(e) => setMacroName(e.target.value)}
            placeholder={t('macro.create')}
            className="flex-1 px-2 py-1 text-sm rounded bg-surface-sunken text-text-primary border border-border-default focus:border-border-focus outline-none"
          />
          <button
            onClick={selectedMacro ? handleSave : handleCreate}
            className="px-3 py-1 text-sm rounded bg-interactive-default text-text-inverse hover:bg-interactive-hover transition-colors"
          >
            {selectedMacro ? t('actions.save') : t('actions.create')}
          </button>
          {selectedMacro && (
            <>
              <button
                onClick={handleRun}
                className="p-1.5 rounded bg-green-600 text-white hover:bg-green-500 transition-colors"
                title={t('macro.run')}
              >
                <Play size={14} />
              </button>
              {execution?.status === 'running' && (
                <>
                  <button
                    onClick={handlePause}
                    className="p-1.5 rounded bg-yellow-600 text-white hover:bg-yellow-500 transition-colors"
                    title={t('macro.pause')}
                  >
                    <Pause size={14} />
                  </button>
                  <button
                    onClick={handleStop}
                    className="p-1.5 rounded bg-red-600 text-white hover:bg-red-500 transition-colors"
                    title={t('macro.stop')}
                  >
                    <Square size={14} />
                  </button>
                </>
              )}
              {execution?.status === 'paused' && (
                <button
                  onClick={handleResume}
                  className="p-1.5 rounded bg-green-600 text-white hover:bg-green-500 transition-colors"
                  title={t('macro.resume')}
                >
                  <RotateCcw size={14} />
                </button>
              )}
              <button
                onClick={handleDelete}
                className="p-1.5 rounded text-red-400 hover:bg-red-500/10 transition-colors"
                title={t('actions.delete')}
              >
                <Trash2 size={14} />
              </button>
            </>
          )}
        </div>

        {/* Steps */}
        <div className="flex-1 overflow-auto p-3">
          <div className="flex items-center justify-between mb-2">
            <h4 className="text-sm font-medium text-text-primary">{t('macro.steps')}</h4>
          </div>

          {steps.length === 0 ? (
            <p className="text-sm text-text-secondary py-4 text-center">
              {t('macro.addStep')}
            </p>
          ) : (
            <div className="space-y-2">
              {steps.map((step, i) => (
                <div
                  key={`step-${step.type}-${i}`}
                  className="flex items-center gap-2 p-2 rounded bg-surface-secondary border border-border-subtle"
                >
                  <GripVertical size={14} className="text-text-disabled cursor-grab" />
                  <span className="flex-1 text-sm text-text-primary font-mono">
                    {stepLabel(step)}
                  </span>
                  {step.type === 'send' && (
                    <input
                      type="text"
                      value={step.data || ''}
                      onChange={(e) => updateStep(i, { data: e.target.value })}
                      className="w-48 px-2 py-0.5 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
                      placeholder="Data to send"
                    />
                  )}
                  {step.type === 'expect' && (
                    <input
                      type="text"
                      value={step.pattern || ''}
                      onChange={(e) => updateStep(i, { pattern: e.target.value })}
                      className="w-48 px-2 py-0.5 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
                      placeholder="Pattern"
                    />
                  )}
                  {step.type === 'wait' && (
                    <input
                      type="number"
                      value={step.duration_ms || 0}
                      onChange={(e) => updateStep(i, { duration_ms: Number.parseInt(e.target.value) || 0 })}
                      className="w-24 px-2 py-0.5 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
                      placeholder="ms"
                    />
                  )}
                  <button
                    onClick={() => removeStep(i)}
                    className="p-1 text-red-400 hover:text-red-300 transition-colors"
                  >
                    <Trash2 size={12} />
                  </button>
                </div>
              ))}
            </div>
          )}

          {/* Add Step Buttons */}
          <div className="mt-3 flex flex-wrap gap-1">
            {STEP_TYPE_OPTIONS.map((opt) => (
              <button
                key={opt.value}
                onClick={() => addStep(opt.value)}
                className="flex items-center gap-1 px-2 py-1 text-xs rounded bg-surface-elevated text-text-secondary hover:text-text-primary hover:bg-surface-secondary border border-border-subtle transition-colors"
              >
                {opt.icon}
                {opt.label}
              </button>
            ))}
          </div>
        </div>

        {/* Variables Panel */}
        {execution && (
          <div className="border-t border-border-default p-3">
            <h4 className="text-sm font-medium text-text-primary mb-2">
              {t('macro.variables')}
            </h4>
            <div className="text-xs text-text-secondary">
              {Object.keys(execution.variables).length === 0 ? (
                <span>No variables set</span>
              ) : (
                <div className="space-y-1">
                  {Object.entries(execution.variables).map(([key, val]) => (
                    <div key={key} className="flex gap-2">
                      <span className="font-mono text-accent-primary">${key}</span>
                      <span>=</span>
                      <span className="font-mono">{val}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
