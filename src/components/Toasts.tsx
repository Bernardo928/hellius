import { useEffect } from 'react';
import { GlassButton, GlassToast } from '../ui/glass';
import { useAppStore, type Toast } from '../store/appStore';
import { ptBR } from '../i18n/pt-BR';

const AUTO_MS = 4500;

function ToastItem({ t }: { t: Toast }) {
  const dismiss = useAppStore((s) => s.dismissToast);
  useEffect(() => {
    const id = window.setTimeout(() => dismiss(t.id), AUTO_MS);
    return () => window.clearTimeout(id);
  }, [t.id, dismiss]);
  return (
    <GlassToast
      tone={t.tone}
      action={
        <GlassButton size="sm" variant="ghost" iconOnly aria-label={ptBR.about.close} onClick={() => dismiss(t.id)} icon="×" />
      }
    >
      {t.message}
    </GlassToast>
  );
}

export function Toasts() {
  const toasts = useAppStore((s) => s.toasts);
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((t) => (
        <ToastItem key={t.id} t={t} />
      ))}
    </div>
  );
}
