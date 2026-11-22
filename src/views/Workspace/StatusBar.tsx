import { useAppStore } from '../../store/appStore';
import { ptBR } from '../../i18n/pt-BR';

/** Linha de status no rodapé do conteúdo (texto, sem vidro). */
export function StatusBar() {
  const cursorLine = useAppStore((s) => s.cursorLine);
  const cursorAddress = useAppStore((s) => s.cursorAddress);
  const selectAll = useAppStore((s) => s.selectAllActive);
  const total = useAppStore((s) => s.scopeTotalLines);
  const engine = useAppStore((s) => s.engine);

  return (
    <footer className="status" role="status">
      <span className="mono">
        {ptBR.status.address} {cursorAddress ?? '—'}
      </span>
      <span className="mono">
        {ptBR.status.line} {cursorLine ?? '—'}
      </span>
      <span>{selectAll && total !== null ? ptBR.status.selected(total) : total !== null ? `${total} linhas` : ''}</span>
      <span className="status__spacer" />
      {engine?.isDemo ? <span className="status__demo">{ptBR.status.demo}</span> : <span>{engine?.name ?? ''}</span>}
    </footer>
  );
}
