import { useEffect, useRef, type ReactNode } from 'react';
import { GlassButton, GlassPanel } from '../ui/glass';
import { ptBR } from '../i18n/pt-BR';

interface ModalProps {
  open: boolean;
  title: string;
  onClose: () => void;
  children: ReactNode;
}

/** Diálogo de vidro. Fecha com Esc e devolve o foco ao elemento anterior. */
export function Modal({ open, title, onClose, children }: ModalProps) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const prev = document.activeElement as HTMLElement | null;
    ref.current?.querySelector<HTMLElement>('input, button')?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
      prev?.focus?.();
    };
  }, [open, onClose]);

  if (!open) return null;
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className="modal-wrap">
      <GlassPanel role="dialog" aria-modal="true" aria-label={title} className="modal">
        <header className="modal__head">
          <h2>{title}</h2>
          <GlassButton size="sm" variant="ghost" iconOnly aria-label={ptBR.shortcuts.close} onClick={onClose} icon="×" />
        </header>
        {children}
      </GlassPanel>
      </div>
    </div>
  );
}
