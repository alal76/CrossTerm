import { useState, useCallback } from "react";
import { X, Plus, Trash2, Filter } from "lucide-react";
import { SessionType, ConnectionStatus } from "@/types";
import type { FilterExpr } from "@/types";

type ConditionType = FilterExpr extends { type: infer T } ? Exclude<T, "and" | "or" | "not"> : never;

interface ConditionRow {
  key: string;
  type: ConditionType;
  value: string;
}

const CONDITION_LABELS: Record<ConditionType, string> = {
  tag: "Tag is",
  protocol: "Type is",
  status: "Status is",
  last_connected_before: "Not connected in the last (days)",
  name_contains: "Name contains",
  host_contains: "Host contains",
};

let rowSeq = 0;
function newRow(): ConditionRow {
  rowSeq += 1;
  return { key: `row-${rowSeq}`, type: "name_contains", value: "" };
}

function rowToFilterExpr(row: ConditionRow): FilterExpr | null {
  const value = row.value.trim();
  if (!value) return null;
  switch (row.type) {
    case "tag":
      return { type: "tag", value };
    case "protocol":
      return { type: "protocol", value: value as SessionType };
    case "status":
      return { type: "status", value: value as ConnectionStatus };
    case "last_connected_before": {
      const days = Number.parseInt(value, 10);
      return Number.isNaN(days) ? null : { type: "last_connected_before", days };
    }
    case "name_contains":
      return { type: "name_contains", value };
    case "host_contains":
      return { type: "host_contains", value };
    default:
      return null;
  }
}

/** Builds a FilterExpr (all conditions combined with AND) from the given rows. */
export function buildFilterExpr(rows: ConditionRow[]): FilterExpr | null {
  const exprs = rows.map(rowToFilterExpr).filter((e): e is FilterExpr => e !== null);
  if (exprs.length === 0) return null;
  if (exprs.length === 1) return exprs[0];
  return { type: "and", children: exprs };
}

function ConditionValueInput({
  row,
  onChange,
}: {
  readonly row: ConditionRow;
  readonly onChange: (value: string) => void;
}) {
  if (row.type === "protocol") {
    return (
      <select
        value={row.value}
        onChange={(e) => onChange(e.target.value)}
        className="flex-1 px-2 py-1 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
      >
        <option value="">Select a type…</option>
        {Object.values(SessionType).map((t) => (
          <option key={t} value={t}>{t}</option>
        ))}
      </select>
    );
  }
  if (row.type === "status") {
    return (
      <select
        value={row.value}
        onChange={(e) => onChange(e.target.value)}
        className="flex-1 px-2 py-1 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
      >
        <option value="">Select a status…</option>
        {Object.values(ConnectionStatus).map((s) => (
          <option key={s} value={s}>{s}</option>
        ))}
      </select>
    );
  }
  if (row.type === "last_connected_before") {
    return (
      <input
        type="number"
        min={0}
        value={row.value}
        onChange={(e) => onChange(e.target.value)}
        placeholder="30"
        className="flex-1 px-2 py-1 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
      />
    );
  }
  return (
    <input
      type="text"
      value={row.value}
      onChange={(e) => onChange(e.target.value)}
      placeholder="value…"
      className="flex-1 px-2 py-1 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
    />
  );
}

export interface SmartGroupBuilderProps {
  readonly onClose: () => void;
  readonly onCreate: (name: string, filter: FilterExpr) => void;
}

export default function SmartGroupBuilder({ onClose, onCreate }: SmartGroupBuilderProps) {
  const [name, setName] = useState("");
  const [rows, setRows] = useState<ConditionRow[]>([newRow()]);
  const [error, setError] = useState<string | null>(null);

  const updateRow = useCallback((key: string, updates: Partial<ConditionRow>) => {
    setRows((prev) => prev.map((r) => (r.key === key ? { ...r, ...updates } : r)));
  }, []);

  const removeRow = useCallback((key: string) => {
    setRows((prev) => (prev.length > 1 ? prev.filter((r) => r.key !== key) : prev));
  }, []);

  const handleCreate = useCallback(() => {
    if (!name.trim()) {
      setError("Name is required.");
      return;
    }
    const filter = buildFilterExpr(rows);
    if (!filter) {
      setError("At least one condition needs a value.");
      return;
    }
    onCreate(name.trim(), filter);
  }, [name, rows, onCreate]);

  return (
    <div className="fixed inset-0 z-[8500] flex items-center justify-center bg-black/40">
      <div className="w-[420px] max-h-[80vh] overflow-y-auto bg-surface-primary rounded-xl shadow-xl border border-border-default flex flex-col">
        <div className="flex items-center justify-between px-4 py-3 border-b border-border-default">
          <span className="flex items-center gap-2 text-sm font-semibold text-text-primary">
            <Filter size={14} className="text-accent-primary" />
            New Smart Group
          </span>
          <button onClick={onClose} className="p-1 rounded hover:bg-interactive-hover">
            <X size={16} />
          </button>
        </div>

        <div className="flex-1 p-4 space-y-3">
          <div>
            <label htmlFor="smart-group-name" className="text-xs font-medium text-text-secondary">Name</label>
            <input
              id="smart-group-name"
              type="text"
              value={name}
              onChange={(e) => { setName(e.target.value); setError(null); }}
              placeholder="e.g. Production SSH"
              className="mt-1 w-full px-3 py-1.5 text-sm rounded bg-surface-sunken border border-border-default text-text-primary"
            />
          </div>

          <div className="space-y-2">
            <span className="text-xs font-medium text-text-secondary">Match sessions where all of:</span>
            {rows.map((row) => (
              <div key={row.key} className="flex items-center gap-1.5">
                <select
                  value={row.type}
                  onChange={(e) => updateRow(row.key, { type: e.target.value as ConditionType, value: "" })}
                  className="px-2 py-1 text-xs rounded bg-surface-sunken border border-border-default text-text-primary"
                >
                  {Object.entries(CONDITION_LABELS).map(([value, label]) => (
                    <option key={value} value={value}>{label}</option>
                  ))}
                </select>
                <ConditionValueInput row={row} onChange={(value) => updateRow(row.key, { value })} />
                <button
                  onClick={() => removeRow(row.key)}
                  disabled={rows.length === 1}
                  className="p-1 text-text-disabled hover:text-status-disconnected disabled:opacity-30 transition-colors"
                  title="Remove condition"
                >
                  <Trash2 size={12} />
                </button>
              </div>
            ))}
            <button
              onClick={() => setRows((prev) => [...prev, newRow()])}
              className="flex items-center gap-1 text-xs text-text-secondary hover:text-accent-primary transition-colors"
            >
              <Plus size={12} /> Add condition
            </button>
          </div>

          {error && <p className="text-xs text-status-disconnected">{error}</p>}
        </div>

        <div className="flex justify-end gap-2 px-4 py-3 border-t border-border-default">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-xs rounded text-text-secondary hover:text-text-primary transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={handleCreate}
            className="px-3 py-1.5 text-xs rounded bg-interactive-default hover:bg-interactive-hover text-text-primary transition-colors"
          >
            Create
          </button>
        </div>
      </div>
    </div>
  );
}
