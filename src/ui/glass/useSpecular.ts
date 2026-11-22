import { useEffect, type RefObject } from 'react';

/**
 * Atualiza --mx/--my no elemento conforme o ponteiro, com requestAnimationFrame.
 * Usado pelo brilho especular do vidro. Sem efeito com prefers-reduced-transparency
 * (o CSS esconde o brilho).
 */
export function useSpecular(ref: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let frame = 0;
    let x = 0;
    let y = 0;
    const onMove = (e: PointerEvent) => {
      const r = el.getBoundingClientRect();
      x = e.clientX - r.left;
      y = e.clientY - r.top;
      if (!frame) {
        frame = requestAnimationFrame(() => {
          frame = 0;
          el.style.setProperty('--mx', `${x}px`);
          el.style.setProperty('--my', `${y}px`);
        });
      }
    };
    el.addEventListener('pointermove', onMove);
    return () => {
      el.removeEventListener('pointermove', onMove);
      if (frame) cancelAnimationFrame(frame);
    };
  }, [ref]);
}
