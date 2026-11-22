import { useEffect } from 'react';
import { GlassButton, GlassToast } from '../ui/glass';
import { useAppStore, type Toast } from '../store/appStore';
import { IconClose } from '../ui/icons';
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
        <GlassButton size="sm" iconOnly aria-label={ptBR.about.close} onClick={() => dismiss(t.id)} icon={<IconClose size={14} />} />
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
