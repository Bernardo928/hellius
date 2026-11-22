// "Copiar" e Ctrl+C: sempre copiam o texto COMPLETO do escopo, não só o visível.
import { api } from '../api/hellius';
import { useAppStore } from '../store/appStore';
import { copyText } from './clipboard';
import { errorMessage } from './analysis';
import { ptBR } from '../i18n/pt-BR';

export async function copyCurrentScope(): Promise<void> {
  const s = useAppStore.getState();
  if (!s.selectedFunction && s.scope === 'function') return;
  try {
    const text = await api.getFullText(
      s.scope,
      s.codeKind,
      s.scope === 'function' ? s.selectedFunction ?? undefined : undefined,
    );
    const ok = await copyText(text);
    useAppStore.getState().pushToast({
      tone: ok ? 'success' : 'error',
      message: ok ? `Copiado: ${text.split('\n').length} linhas` : 'Não foi possível copiar.',
    });
  } catch (e) {
    useAppStore.getState().pushToast({ tone: 'error', message: `${ptBR.errors.generic} ${errorMessage(e)}` });
  }
}
