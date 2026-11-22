// Fluxo de abertura: validação e análise pelo Rust, depois carregamento das listas.
import { api, onAnalysisProgress } from '../api/hellius';
import { useAppStore } from '../store/appStore';
import { clearCodeCaches } from '../views/Workspace/codeData';
import { ptBR } from '../i18n/pt-BR';

const COLLAPSE_MS = 720;

export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return ptBR.errors.generic;
}

/** Abre um arquivo. Ignora pedidos durante outra análise. Erros viram toast. */
export async function openFile(path: string): Promise<void> {
  const store = useAppStore.getState();
  if (store.screen === 'analyzing') return;
  store.setScreen('analyzing');
  store.setProgress({ phase: ptBR.analysis.phaseLabel, step: 0, total: 4 });
  let unlisten: (() => void) | undefined;
  try {
    unlisten = await onAnalysisProgress((p) => useAppStore.getState().setProgress(p));
    const file = await api.openFile(path);
    const [engine, functions, strings, imports] = await Promise.all([
      api.engineInfo(),
      api.listFunctions(),
      api.listStrings(),
      api.listImports(),
    ]);
    // Totais e fatias do arquivo anterior não valem para este.
    clearCodeCaches();
    useAppStore.getState().setLoaded({ file, engine, functions, strings, imports });
    useAppStore.getState().addRecent({ path: file.path, name: file.name });
    useAppStore.getState().setProgress({ phase: 'Concluído', step: 4, total: 4 });
    await new Promise((r) => setTimeout(r, COLLAPSE_MS));
    useAppStore.getState().setScreen('workspace');
  } catch (e) {
    useAppStore.getState().setProgress(null);
    useAppStore.getState().setScreen('portal');
    useAppStore.getState().pushToast({ tone: 'error', message: `${ptBR.analysis.error}: ${errorMessage(e)}` });
  } finally {
    unlisten?.();
  }
}
