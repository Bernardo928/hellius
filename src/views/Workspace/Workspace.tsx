import { useCallback, useEffect, useState } from 'react';
import { useAppStore } from '../../store/appStore';
import { goToInput } from '../../lib/navigation';
import { GlassInput, GlassButton } from '../../ui/glass';
import { Modal } from '../../components/Modal';
import { ptBR } from '../../i18n/pt-BR';
import { TopBar } from './TopBar';
import { SidePane } from './SidePane';
import { CodeViewer } from './CodeViewer';
import { Inspector } from './Inspector';
import { StatusBar } from './StatusBar';
import { Resizer } from './Resizer';
import './Workspace.css';

interface Layout {
  left: number;
  right: number;
  leftCollapsed: boolean;
  rightCollapsed: boolean;
}

const LAYOUT_KEY = 'hellius.layout';
const DEFAULT_LAYOUT: Layout = { left: 260, right: 280, leftCollapsed: false, rightCollapsed: false };
const LIMITS = { left: [200, 420], right: [220, 420] } as const;

function loadLayout(): Layout {
  try {
    const raw = window.localStorage.getItem(LAYOUT_KEY);
    if (!raw) return DEFAULT_LAYOUT;
    const p = JSON.parse(raw) as Partial<Layout>;
    return {
      left: clamp(p.left ?? DEFAULT_LAYOUT.left, LIMITS.left),
      right: clamp(p.right ?? DEFAULT_LAYOUT.right, LIMITS.right),
      leftCollapsed: !!p.leftCollapsed,
      rightCollapsed: !!p.rightCollapsed,
    };
  } catch {
    return DEFAULT_LAYOUT;
  }
}

function clamp(v: number, [lo, hi]: readonly [number, number]): number {
  return Math.min(hi, Math.max(lo, v));
}

export function Workspace() {
  const selectAll = useAppStore((s) => s.selectAllActive);
  const setSelectAll = useAppStore((s) => s.setSelectAll);
  const setSearch = useAppStore((s) => s.setSearch);
  const searchOpen = useAppStore((s) => s.searchOpen);
  const goToOpen = useAppStore((s) => s.goToOpen);
  const setGoTo = useAppStore((s) => s.setGoTo);
  const setHelp = useAppStore((s) => s.setHelp);
  const helpOpen = useAppStore((s) => s.helpOpen);
  const goBack = useAppStore((s) => s.goBack);
  const goForward = useAppStore((s) => s.goForward);
  const selectFunction = useAppStore((s) => s.selectFunction);

  const [layout, setLayout] = useState<Layout>(loadLayout);
  const saveLayout = useCallback((next: Layout) => {
    setLayout(next);
    try {
      window.localStorage.setItem(LAYOUT_KEY, JSON.stringify(next));
    } catch {
      /* sem armazenamento: o layout vale só nesta sessão */
    }
  }, []);

  // Atalhos globais da área de trabalho.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const typing = !!target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA');
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key.toLowerCase() === 'g') {
        e.preventDefault();
        setGoTo(true);
      } else if (mod && e.key.toLowerCase() === 'f' && !typing) {
        e.preventDefault();
        setSearch({ searchOpen: true });
      } else if (e.altKey && e.key === 'ArrowLeft') {
        e.preventDefault();
        const a = goBack();
        if (a) selectFunction(a, { fromHistory: true });
      } else if (e.altKey && e.key === 'ArrowRight') {
        e.preventDefault();
        const a = goForward();
        if (a) selectFunction(a, { fromHistory: true });
      } else if (e.key === '?' && !typing) {
        setHelp(true);
      } else if (e.key === 'Escape' && !typing) {
        setSelectAll(false);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [setGoTo, setSearch, setHelp, setSelectAll, goBack, goForward, selectFunction]);

  const leftW = layout.leftCollapsed ? 0 : layout.left;
  const rightW = layout.rightCollapsed ? 0 : layout.right;
  const gridCols = `${leftW}px 6px minmax(0, 1fr) 6px ${rightW}px`;

  return (
    <div className="ws" data-select-all={selectAll || undefined} data-search={searchOpen || undefined}>
      <TopBar />
      <div className="ws__main" style={{ gridTemplateColumns: gridCols }}>
        <aside className="ws__left glass glass-floating" aria-label={ptBR.tabs.functions} hidden={layout.leftCollapsed}>
          <SidePane />
        </aside>
        {layout.leftCollapsed ? (
          <span />
        ) : (
          <Resizer
            label="coluna esquerda"
            onDelta={(dx) => saveLayout({ ...layout, left: clamp(layout.left + dx, LIMITS.left) })}
            onToggleCollapse={() => saveLayout({ ...layout, leftCollapsed: !layout.leftCollapsed })}
          />
        )}
        <main className="ws__center">
          <CodeViewer />
        </main>
        {layout.rightCollapsed ? (
          <span />
        ) : (
          <Resizer
            label="coluna direita"
            onDelta={(dx) => saveLayout({ ...layout, right: clamp(layout.right - dx, LIMITS.right) })}
            onToggleCollapse={() => saveLayout({ ...layout, rightCollapsed: !layout.rightCollapsed })}
          />
        )}
        <aside className="ws__right glass glass-floating" aria-label={ptBR.inspector.title} hidden={layout.rightCollapsed}>
          <Inspector />
        </aside>
      </div>
      <StatusBar />

      <GoToDialog open={goToOpen} onClose={() => setGoTo(false)} />
      <Modal open={helpOpen} title={ptBR.shortcuts.title} onClose={() => setHelp(false)}>
        <table className="shortcuts">
          <tbody>
            {ptBR.shortcuts.rows.map(([k, v]) => (
              <tr key={k}>
                <th scope="row" className="mono">
                  {k}
                </th>
                <td>{v}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Modal>
    </div>
  );
}

function GoToDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [value, setValue] = useState('');
  if (!open) return null;
  return (
    <Modal open title="Ir para endereço" onClose={onClose}>
      <form
        className="goto"
        onSubmit={(e) => {
          e.preventDefault();
          if (goToInput(value)) {
            setValue('');
            onClose();
          }
        }}
      >
        <GlassInput
          autoFocus
          aria-label="Endereço ou nome"
          placeholder="0x140001040 ou nome da função"
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <GlassButton type="submit" variant="primary">
          Ir
        </GlassButton>
      </form>
    </Modal>
  );
}
