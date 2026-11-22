import { useRef, type KeyboardEvent, type PointerEvent } from 'react';
import { ptBR } from '../../i18n/pt-BR';

interface Props {
  /** Chamado com o deslocamento em px desde o último movimento. */
  onDelta: (dx: number) => void;
  onToggleCollapse: () => void;
  label: string;
}

/** Alça vertical arrastável. Duplo clique recolhe/expande; setas ajustam pelo teclado. */
export function Resizer({ onDelta, onToggleCollapse, label }: Props) {
  const last = useRef<number | null>(null);
  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    last.current = e.clientX;
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (last.current === null) return;
    onDelta(e.clientX - last.current);
    last.current = e.clientX;
  };
  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    last.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) e.currentTarget.releasePointerCapture(e.pointerId);
  };
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'ArrowLeft') onDelta(-16);
    if (e.key === 'ArrowRight') onDelta(16);
    if (e.key === 'Enter') onToggleCollapse();
  };
  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={`${ptBR.a11y.resize}: ${label}`}
      tabIndex={0}
      className="resizer"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      onDoubleClick={onToggleCollapse}
      onKeyDown={onKey}
    />
  );
}
