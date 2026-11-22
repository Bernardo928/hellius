// "Explicar com IA": pede ao Rust a explicação da função atual e acumula o texto em streaming.
import { api, onAiDelta } from '../api/hellius';
import { useAppStore } from '../store/appStore';
import { errorMessage } from './analysis';

export async function refreshAiStatus(): Promise<void> {
  try {
    useAppStore.getState().patchAi({ status: await api.aiStatus() });
  } catch {
    /* sem backend (testes/navegador): painel mostra o formulário */
  }
}

/** Explica a função. Só roda com chave e consentimento; o painel cuida de pedir os dois. */
export async function explainFunction(functionAddress: string): Promise<void> {
  const s = useAppStore.getState();
  if (s.ai.running) await cancelExplain();
  const requestId = s.ai.requestId + 1;
  s.patchAi({ requestId, running: true, text: '', error: null, functionAddress });
  const unlisten = await onAiDelta((d) => {
    const cur = useAppStore.getState().ai;
    if (d.requestId === cur.requestId) useAppStore.getState().patchAi({ text: cur.text + d.text });
  });
  try {
    const full = await api.aiExplain(functionAddress, requestId);
    if (useAppStore.getState().ai.requestId === requestId) {
      useAppStore.getState().patchAi({ text: full, running: false });
    }
  } catch (e) {
    if (useAppStore.getState().ai.requestId === requestId) {
      useAppStore.getState().patchAi({ running: false, error: errorMessage(e) });
    }
  } finally {
    unlisten();
  }
}

export async function cancelExplain(): Promise<void> {
  try {
    await api.aiCancel();
  } finally {
    useAppStore.getState().patchAi({ running: false });
  }
}
