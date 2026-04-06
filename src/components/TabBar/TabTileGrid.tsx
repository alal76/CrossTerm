import { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { Grid2X2, GripVertical } from 'lucide-react';
import clsx from 'clsx';

interface TilePosition {
  row: number;
  col: number;
}

interface TiledTab {
  tabId: string;
  position: TilePosition;
}

interface TabTileGridProps {
  tabs?: TiledTab[];
  onTabDrop?: (tabId: string, position: TilePosition) => void;
  className?: string;
}

export default function TabTileGrid({
  tabs = [],
  onTabDrop,
  className,
}: Readonly<TabTileGridProps>) {
  const { t } = useTranslation();
  const [dragOverCell, setDragOverCell] = useState<TilePosition | null>(null);

  const handleDragOver = useCallback(
    (e: React.DragEvent, position: TilePosition) => {
      e.preventDefault();
      setDragOverCell(position);
    },
    []
  );

  const handleDrop = useCallback(
    (e: React.DragEvent, position: TilePosition) => {
      e.preventDefault();
      const tabId = e.dataTransfer.getData('text/tab-id');
      if (tabId) {
        onTabDrop?.(tabId, position);
      }
      setDragOverCell(null);
    },
    [onTabDrop]
  );

  const handleDragLeave = useCallback(() => {
    setDragOverCell(null);
  }, []);

  const getTabAtPosition = (row: number, col: number) => {
    return tabs.find((tab) => tab.position.row === row && tab.position.col === col);
  };

  const grid = [
    [0, 0],
    [0, 1],
    [1, 0],
    [1, 1],
  ] as const;

  return (
    <div className={clsx('flex flex-col gap-2', className)}>
      <div className="flex items-center gap-2 px-2 py-1">
        <Grid2X2 size={14} className="text-text-secondary" />
        <span className="text-xs text-text-secondary font-medium">
          {t('tabTile.2x2')}
        </span>
      </div>
      <div className="grid grid-cols-2 grid-rows-2 gap-1 flex-1 min-h-[200px]">
        {grid.map(([row, col]) => {
          const tiled = getTabAtPosition(row, col);
          const isOver =
            dragOverCell?.row === row && dragOverCell?.col === col;

          return (
            <div
              key={`${row}-${col}`}
              role="gridcell"
              tabIndex={0}
              onDragOver={(e) => handleDragOver(e, { row, col })}
              onDrop={(e) => handleDrop(e, { row, col })}
              onDragLeave={handleDragLeave}
              className={clsx(
                'rounded border-2 flex items-center justify-center transition-colors',
                isOver && 'border-accent-primary bg-accent-primary/10',
                !isOver && tiled && 'border-border-default bg-surface-secondary',
                !isOver && !tiled && 'border-dashed border-border-subtle bg-surface-sunken'
              )}
            >
              {tiled ? (
                <div className="flex items-center gap-1 text-sm text-text-primary">
                  <GripVertical size={14} className="text-text-secondary cursor-grab" />
                  <span className="truncate">{tiled.tabId}</span>
                </div>
              ) : (
                <span className="text-xs text-text-disabled">
                  {t('tabTile.grid')}
                </span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
