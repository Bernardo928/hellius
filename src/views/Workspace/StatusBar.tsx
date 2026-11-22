import { useAppStore } from '../../store/appStore';
import { ptBR } from '../../i18n/pt-BR';

export function StatusBar() {
  const cursorLine = useAppStore((s) => s.cursorLine);
  const cursorAddress = useAppStore((s) => s.cursorAddress);
  const selectAll = useAppStore((s) => s.selectAllActive);
  const total = useAppStore((s) => s.scopeTotalLines);
  const engine = useAppStore((s) => s.engine);

  return (
    <footer className="statusbar glass glass-floating" role="status">
      <div className="statusbar__inner">
        <span className="mono">
          {ptBR.status.address}: {cursorAddress ?? '—'}
        </span>
        <span className="mono">
          {ptBR.status.line}: {cursorLine ?? '—'}
        </span>
        <span className="mono">
          {selectAll && total !== null ? ptBR.status.selected(total) : `${ptBR.status.selection}: —`}
        </span>
        <span className="statusbar__spacer" />
        {engine?.isDemo ? (
          <span className="demo-badge">{ptBR.status.demo}</span>
        ) : (
          <span>{engine?.name ?? ''}</span>
        )}
      </div>
    </footer>
  );
}
