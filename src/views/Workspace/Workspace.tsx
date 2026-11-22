import { useCallback, useEffect, useState } from 'react';
import { useAppStore } from '../../store/appStore';
import { goToInput } from '../../lib/navigation';
import { GlassButton, GlassPanel, GlassSearchField } from '../../ui/glass';
import { Modal } from '../../components/Modal';
import { ptBR } from '../../i18n/pt-BR';
import { Toolbar } from './Toolbar';
import { SidePane } from './SidePane';
import { CodeViewer } from './CodeViewer';
import { Inspector } from './Inspector';
import { StatusBar } from './StatusBar';
import { Resizer } from './Resizer';
import './Workspace.css';

interface Layout {
  left: number;
  right: number;
  leftOpen: boolean;
  rightOpen: boolean;
}

const LAYOUT_KEY = 'hellius.layout.v2';
const DEFAULT_LAYOUT: Layout = { left: 264, right: 300, leftOpen: true, rightOpen: true };
const LIMITS = { left: [200, 420], right: [240, 440] } as const;

function clamp(v: number, [lo, hi]: readonly [number, number]): number {
  return Math.min(hi, Math.max(lo, v));
}

function loadLayout(): Layout {
  try {
    const raw = window.localStorage.getItem(LAYOUT_KEY);
    if (!raw) return DEFAULT_LAYOUT;
    const p = JSON.parse(raw) as Partial<Layout>;
    return {
      left: clamp(Number(p.left) || DEFAULT_LAYOUT.left, LIMITS.left),
      right: clamp(Number(p.right) || DEFAULT_LAYOUT.right, LIMITS.right),
      leftOpen: p.leftOpen !== false,
      rightOpen: p.rightOpen !== false,
    };
  } catch {
    return DEFAULT_LAYOUT;
  }
}

export function Workspace() {
  const setSelectAll = useAppStore((s) => s.setSelectAll);
  const setSearch = useAppStore((s) => s.setSearch);
  const goToOpen = useAppStore((s) => s.goToOpen);
  const setGoTo = useAppStore((s) => s.setGoTo);
  const setHelp = useAppStore((s) => s.setHelp);
  const helpOpen = useAppStore((s) => s.helpOpen);
  const goBack = useAppStore((s) => s.goBack);
  const goForward = useAppStore((s) => s.goForward);
  const selectFunction = useAppStore((s) => s.selectFunction);

  const [layout, setLayout] = useState<Layout>(loadLayout);
  const saveLayout = useCallback((patch: Partial<Layout>) => {
    setLayout((prev) => {
      const next = { ...prev, ...patch };
      try {
        window.localStorage.setItem(LAYOUT_KEY, JSON.stringify(next));
      } catch {
        /* sem armazenamento: vale só nesta sessão */
      }
      return next;
    });
  }, []);

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

  const cols = [
    layout.leftOpen ? `${layout.left}px 8px` : '',
    'minmax(0, 1fr)',
    layout.rightOpen ? `8px ${layout.right}px` : '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className="ws">
      <Toolbar
        sidebarOpen={layout.leftOpen}
        inspectorOpen={layout.rightOpen}
        onToggleSidebar={() => saveLayout({ leftOpen: !layout.leftOpen })}
        onToggleInspector={() => saveLayout({ rightOpen: !layout.rightOpen })}
      />
      <div className="ws__main" style={{ gridTemplateColumns: cols }}>
        {layout.leftOpen && (
          <>
            <GlassPanel as="nav" className="ws__side" aria-label="Listas">
              <SidePane />
            </GlassPanel>
            <Resizer
              label="barra lateral"
              onDelta={(dx) => saveLayout({ left: clamp(layout.left + dx, LIMITS.left) })}
              onToggleCollapse={() => saveLayout({ leftOpen: false })}
            />
          </>
        )}
        <main className="ws__content">
          <CodeViewer />
          <StatusBar />
        </main>
        {layout.rightOpen && (
          <>
            <Resizer
              label="inspetor"
              onDelta={(dx) => saveLayout({ right: clamp(layout.right - dx, LIMITS.right) })}
              onToggleCollapse={() => saveLayout({ rightOpen: false })}
            />
            <GlassPanel as="aside" className="ws__inspector" aria-label={ptBR.inspector.title}>
              <Inspector />
            </GlassPanel>
          </>
        )}
      </div>

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
  return (
    <Modal open={open} title="Ir para endereço" onClose={onClose}>
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
        <GlassSearchField
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
