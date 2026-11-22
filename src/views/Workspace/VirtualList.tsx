import { useRef, type ReactNode } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';

interface Props<T> {
  items: T[];
  rowHeight: number;
  label: string;
  empty: string;
  getKey: (item: T) => string;
  render: (item: T) => ReactNode;
}

/** Lista virtualizada simples. Linhas sem backdrop-filter (regra de desempenho). */
export function VirtualList<T>({ items, rowHeight, label, empty, getKey, render }: Props<T>) {
  const ref = useRef<HTMLDivElement>(null);
  const v = useVirtualizer({
    count: items.length,
    getScrollElement: () => ref.current,
    estimateSize: () => rowHeight,
    overscan: 12,
  });
  if (items.length === 0) return <div className="list__empty">{empty}</div>;
  return (
    <div ref={ref} className="list" role="list" aria-label={label}>
      <div style={{ height: v.getTotalSize(), position: 'relative' }}>
        {v.getVirtualItems().map((vi) => {
          const it = items[vi.index];
          if (!it) return null;
          return (
            <div
              key={getKey(it)}
              role="listitem"
              className="list__item"
              style={{ position: 'absolute', top: 0, left: 0, right: 0, height: rowHeight, transform: `translateY(${vi.start}px)` }}
            >
              {render(it)}
            </div>
          );
        })}
      </div>
    </div>
  );
}
