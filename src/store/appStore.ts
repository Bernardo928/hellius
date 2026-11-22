import { create } from 'zustand';
import type { EngineInfo, FileSummary, FunctionInfo, ImportInfo, StringInfo, CodeKind, Scope } from '../api/types';
import { applyTheme, loadTheme, type ThemeId } from '../theme/themes';

export type Screen = 'portal' | 'analyzing' | 'workspace';
export type SideTab = 'functions' | 'strings' | 'imports' | 'sections';

export interface RecentFile {
  path: string;
  name: string;
}

export interface Toast {
  id: number;
  tone: 'info' | 'error' | 'success';
  message: string;
}

const RECENT_KEY = 'hellius.recent';
const RECENT_MAX = 5;

function loadRecent(): RecentFile[] {
  try {
    const raw = window.localStorage.getItem(RECENT_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((x): x is RecentFile => typeof x?.path === 'string' && typeof x?.name === 'string')
      .slice(0, RECENT_MAX);
  } catch {
    return [];
  }
}

function saveRecent(list: RecentFile[]): void {
  try {
    window.localStorage.setItem(RECENT_KEY, JSON.stringify(list));
  } catch {
    /* sem armazenamento: a lista some ao fechar, sem quebrar nada */
  }
}

export interface AppState {
  screen: Screen;
  theme: ThemeId;
  file: FileSummary | null;
  engine: EngineInfo | null;
  functions: FunctionInfo[];
  strings: StringInfo[];
  imports: ImportInfo[];
  progress: { phase: string; step: number; total: number } | null;
  recent: RecentFile[];
  toasts: Toast[];
  aboutOpen: boolean;
  helpOpen: boolean;
  dragging: boolean;

  // Visualizador
  sideTab: SideTab;
  selectedFunction: string | null;
  codeKind: CodeKind;
  scope: Scope;
  /** "Selecionar tudo" ativo: todas as linhas do escopo entram na seleção. */
  selectAllActive: boolean;
  /** Linha com foco (índice dentro do escopo atual), para a barra de status. */
  cursorLine: number | null;
  cursorAddress: string | null;
  /** Pedido de rolagem até um endereço, atendido pelo visualizador. */
  jumpRequest: { address: string; nonce: number } | null;
  scopeTotalLines: number | null;
  history: string[];
  historyIndex: number;
  renames: Record<string, string>;
  searchOpen: boolean;
  searchQuery: string;
  searchIndex: number;
  goToOpen: boolean;

  // Ações
  setTheme: (t: ThemeId) => void;
  setDragging: (v: boolean) => void;
  setScreen: (s: Screen) => void;
  setProgress: (p: AppState['progress']) => void;
  setLoaded: (payload: {
    file: FileSummary;
    engine: EngineInfo;
    functions: FunctionInfo[];
    strings: StringInfo[];
    imports: ImportInfo[];
  }) => void;
  resetToPortal: () => void;
  pushToast: (t: Omit<Toast, 'id'>) => number;
  dismissToast: (id: number) => void;
  addRecent: (f: RecentFile) => void;
  setAbout: (v: boolean) => void;
  setHelp: (v: boolean) => void;
  setSideTab: (t: SideTab) => void;
  selectFunction: (addr: string, opts?: { fromHistory?: boolean }) => void;
  goBack: () => string | null;
  goForward: () => string | null;
  setCodeKind: (k: CodeKind) => void;
  setScope: (s: Scope) => void;
  setSelectAll: (v: boolean) => void;
  setCursorLine: (n: number | null, address?: string | null) => void;
  requestJump: (address: string) => void;
  consumeJump: () => void;
  setScopeTotalLines: (n: number | null) => void;
  renameFunction: (addr: string, name: string) => void;
  setSearch: (patch: Partial<Pick<AppState, 'searchOpen' | 'searchQuery' | 'searchIndex'>>) => void;
  setGoTo: (v: boolean) => void;
}

let toastSeq = 1;

export const useAppStore = create<AppState>()((set, get) => ({
  screen: 'portal',
  theme: loadTheme(),
  file: null,
  engine: null,
  functions: [],
  strings: [],
  imports: [],
  progress: null,
  recent: loadRecent(),
  toasts: [],
  aboutOpen: false,
  helpOpen: false,
  dragging: false,

  sideTab: 'functions',
  selectedFunction: null,
  codeKind: 'disasm',
  scope: 'function',
  selectAllActive: false,
  cursorLine: null,
  cursorAddress: null,
  jumpRequest: null,
  scopeTotalLines: null,
  history: [],
  historyIndex: -1,
  renames: {},
  searchOpen: false,
  searchQuery: '',
  searchIndex: 0,
  goToOpen: false,

  setTheme: (t) => {
    applyTheme(t);
    set({ theme: t });
  },
  setDragging: (v) => set({ dragging: v }),
  setScreen: (s) => set({ screen: s }),
  setProgress: (p) => set({ progress: p }),

  setLoaded: ({ file, engine, functions, strings, imports }) => {
    const first = functions[0]?.address ?? null;
    set({
      file,
      engine,
      functions,
      strings,
      imports,
      progress: null,
      selectedFunction: first,
      history: first ? [first] : [],
      historyIndex: first ? 0 : -1,
      selectAllActive: false,
      cursorLine: null,
      scopeTotalLines: null,
      renames: {},
      searchOpen: false,
      searchQuery: '',
      scope: 'function',
    });
  },

  resetToPortal: () =>
    set({
      screen: 'portal',
      file: null,
      engine: null,
      functions: [],
      strings: [],
      imports: [],
      progress: null,
      selectedFunction: null,
      history: [],
      historyIndex: -1,
      selectAllActive: false,
      renames: {},
    }),

  pushToast: (t) => {
    const id = toastSeq++;
    set((s) => ({ toasts: [...s.toasts, { ...t, id }].slice(-3) }));
    return id;
  },
  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),

  addRecent: (f) => {
    const next = [f, ...get().recent.filter((r) => r.path !== f.path)].slice(0, RECENT_MAX);
    saveRecent(next);
    set({ recent: next });
  },

  setAbout: (v) => set({ aboutOpen: v }),
  setHelp: (v) => set({ helpOpen: v }),
  setSideTab: (t) => set({ sideTab: t }),

  selectFunction: (addr, opts) => {
    const s = get();
    if (opts?.fromHistory) {
      set({ selectedFunction: addr, selectAllActive: false, cursorLine: null, scopeTotalLines: null });
      return;
    }
    if (s.selectedFunction === addr) return;
    const trimmed = s.history.slice(0, s.historyIndex + 1);
    const history = [...trimmed, addr];
    set({
      selectedFunction: addr,
      history,
      historyIndex: history.length - 1,
      selectAllActive: false,
      cursorLine: null,
      scopeTotalLines: null,
    });
  },

  goBack: () => {
    const s = get();
    if (s.historyIndex <= 0) return null;
    const idx = s.historyIndex - 1;
    const addr = s.history[idx] ?? null;
    set({ historyIndex: idx, selectedFunction: addr, selectAllActive: false, cursorLine: null, scopeTotalLines: null });
    return addr;
  },

  goForward: () => {
    const s = get();
    if (s.historyIndex >= s.history.length - 1) return null;
    const idx = s.historyIndex + 1;
    const addr = s.history[idx] ?? null;
    set({ historyIndex: idx, selectedFunction: addr, selectAllActive: false, cursorLine: null, scopeTotalLines: null });
    return addr;
  },

  setCodeKind: (k) => set({ codeKind: k, selectAllActive: false, cursorLine: null }),
  setScope: (s) => set({ scope: s, selectAllActive: false, cursorLine: null, scopeTotalLines: null }),
  setSelectAll: (v) => set({ selectAllActive: v }),
  setCursorLine: (n, address) => set({ cursorLine: n, cursorAddress: address ?? null }),
  requestJump: (address) => set({ jumpRequest: { address, nonce: Date.now() } }),
  consumeJump: () => set({ jumpRequest: null }),
  setScopeTotalLines: (n) => set({ scopeTotalLines: n }),

  renameFunction: (addr, name) => {
    const clean = name.trim();
    set((s) => ({
      renames: clean
        ? { ...s.renames, [addr]: clean }
        : Object.fromEntries(Object.entries(s.renames).filter(([k]) => k !== addr)),
    }));
  },

  setSearch: (patch) => set(patch),
  setGoTo: (v) => set({ goToOpen: v }),
}));

/** Nome exibido de uma função, considerando renomeações desta sessão. */
export function displayName(f: FunctionInfo, renames: Record<string, string>): string {
  return renames[f.address] ?? f.name;
}
