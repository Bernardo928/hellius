import { useEffect, useRef, type ReactNode } from 'react';
import { GlassButton, GlassPanel } from '../ui/glass';
import { IconClose } from '../ui/icons';
import { ptBR } from '../i18n/pt-BR';

interface ModalProps {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
}

/** Diálogo de vidro. Foca o primeiro campo (não o botão de fechar), fecha com Esc
 *  e devolve o foco ao elemento anterior. */
export function Modal({ open, title, onClose, children }: ModalProps) {
  const ref = useRef<HTMLDivElement>(null);
  // onClose num ref: o efeito roda só ao abrir, não a cada render do pai.
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useEffect(() => {
    if (!open) return;
    const prev = document.activeElement as HTMLElement | null;
    const root = ref.current;
    const target =
      root?.querySelector<HTMLElement>('[autofocus], input, textarea, select') ??
      root?.querySelector<HTMLElement>('.modal__body button') ??
      root?.querySelector<HTMLElement>('button');
    target?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        closeRef.current();
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
      prev?.focus?.();
    };
  }, [open]);

  if (!open) return null;
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className="modal-wrap">
        <GlassPanel role="dialog" aria-modal="true" aria-label={title} className="modal">
          <header className="modal__head">
            <h2>{title}</h2>
            <GlassButton size="sm" iconOnly aria-label={ptBR.shortcuts.close} onClick={onClose} icon={<IconClose size={14} />} />
          </header>
          <div className="modal__body">{children}</div>
        </GlassPanel>
      </div>
    </div>
  );
}
