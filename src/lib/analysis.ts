// Fluxo de abertura: validação e análise pelo Rust, depois carregamento das listas.
import { api, onAnalysisProgress } from '../api/hellius';
import { useAppStore } from '../store/appStore';
import { ptBR } from '../i18n/pt-BR';

const COLLAPSE_MS = 720;

export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return ptBR.errors.generic;
}

/** Abre um arquivo. Erros viram toast e o portal volta ao normal. */
export async function openFile(path: string): Promise<void> {
  const store = useAppStore.getState();
  store.setScreen('analyzing');
  store.setProgress({ phase: ptBR.analysis.phaseLabel, step: 0, total: 4 });
  const unlisten = await onAnalysisProgress((p) => useAppStore.getState().setProgress(p));
  try {
    const file = await api.openFile(path);
    const [engine, functions, strings, imports] = await Promise.all([
      api.engineInfo(),
      api.listFunctions(),
      api.listStrings(),
      api.listImports(),
    ]);
    useAppStore.getState().setLoaded({ file, engine, functions, strings, imports });
    useAppStore.getState().addRecent({ path: file.path, name: file.name });
    // Transição: o portal colapsa por ~720 ms revelando o painel.
    useAppStore.getState().setProgress({ phase: 'Concluído', step: 4, total: 4 });
    await new Promise((r) => setTimeout(r, COLLAPSE_MS));
    useAppStore.getState().setScreen('workspace');
  } catch (e) {
    useAppStore.getState().setProgress(null);
    useAppStore.getState().setScreen('portal');
    useAppStore.getState().pushToast({ tone: 'error', message: `${ptBR.analysis.error}: ${errorMessage(e)}` });
  } finally {
    unlisten();
  }
}
