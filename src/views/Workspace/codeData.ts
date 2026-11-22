// Dados do visualizador: layout das linhas e cache de fatias (lazy loading).
import { useCallback, useEffect, useReducer, useRef, useState } from 'react';
import { api } from '../../api/hellius';
import type { CodeKind, CodeLine, FunctionInfo, Scope } from '../../api/types';

/** Linhas buscadas por vez. Nunca enviamos o binário inteiro ao front. */
export const CHUNK = 400;

export type RowRef =
  | { t: 'header'; fn: string }
  | { t: 'blank'; fn: string }
  | { t: 'line'; fn: string; idx: number };

// Totais por (tipo, função). Módulo-level: sobrevive à troca de aba.
// Limpo a cada arquivo aberto (clearCodeCaches), pois endereços se repetem entre arquivos.
const totalsCache = new Map<string, number>();
let generation = 0;

/** Chamado ao abrir outro arquivo: invalida totais e fatias. */
export function clearCodeCaches(): void {
  totalsCache.clear();
  generation++;
}

export function cacheGeneration(): number {
  return generation;
}

async function totalOf(fn: string, kind: CodeKind): Promise<number> {
  const key = `${kind}|${fn}`;
  const hit = totalsCache.get(key);
  if (hit !== undefined) return hit;
  const slice = await api.getCode(fn, kind, 0, 1);
  totalsCache.set(key, slice.totalLines);
  return slice.totalLines;
}

/** Monta a lista de linhas do escopo. Função: só as linhas dela. Programa: todas, com cabeçalhos. */
export function useLayout(
  scope: Scope,
  kind: CodeKind,
  selected: string | null,
  functions: FunctionInfo[],
): { rows: RowRef[] | null; error: string | null } {
  const [state, setState] = useState<{ key: string; rows: RowRef[] | null; error: string | null }>({
    key: '',
    rows: null,
    error: null,
  });
  const fnList = scope === 'program' ? functions.map((f) => f.address) : selected ? [selected] : [];
  const key = `${scope}|${kind}|${fnList.join(',')}`;

  useEffect(() => {
    if (fnList.length === 0) {
      setState({ key, rows: [], error: null });
      return;
    }
    let alive = true;
    (async () => {
      try {
        const totals = await Promise.all(fnList.map((fn) => totalOf(fn, kind)));
        if (!alive) return;
        const rows: RowRef[] = [];
        fnList.forEach((fn, i) => {
          if (scope === 'program') {
            rows.push({ t: 'header', fn });
          }
          const n = totals[i] ?? 0;
          for (let idx = 0; idx < n; idx++) rows.push({ t: 'line', fn, idx });
          if (scope === 'program') rows.push({ t: 'blank', fn });
        });
        setState({ key, rows, error: null });
      } catch (e) {
        if (alive) setState({ key, rows: null, error: String(e) });
      }
    })();
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  if (state.key !== key) return { rows: null, error: null };
  return { rows: state.rows, error: state.error };
}

/** Cache de fatias de linhas, com busca assíncrona por fatia. */
export function useLineCache(kind: CodeKind) {
  const cache = useRef(new Map<string, CodeLine[]>());
  const pending = useRef(new Map<string, Promise<void>>());
  const gen = useRef(generation);
  if (gen.current !== generation) {
    gen.current = generation;
    cache.current.clear();
    pending.current.clear();
  }
  const [, bump] = useReducer((x: number) => x + 1, 0);
  const [version, setVersion] = useState(0);

  const ensure = useCallback(
    (fn: string, chunk: number): Promise<void> => {
      const key = `${kind}|${fn}|${chunk}`;
      if (cache.current.has(key)) return Promise.resolve();
      const inflight = pending.current.get(key);
      if (inflight) return inflight;
      const p = api
        .getCode(fn, kind, chunk * CHUNK, CHUNK)
        .then((slice) => {
          cache.current.set(key, slice.lines);
          pending.current.delete(key);
          bump();
          setVersion((v) => v + 1);
        })
        .catch((e: unknown) => {
          pending.current.delete(key);
          throw e;
        });
      pending.current.set(key, p);
      return p;
    },
    [kind],
  );

  const get = useCallback(
    (fn: string, idx: number): CodeLine | undefined => {
      const chunk = Math.floor(idx / CHUNK);
      return cache.current.get(`${kind}|${fn}|${chunk}`)?.[idx % CHUNK];
    },
    [kind],
  );

  return { ensure, get, version };
}

/** Carrega todas as fatias de uma função (usado por busca e salto). */
export async function ensureFunctionLoaded(
  ensure: (fn: string, chunk: number) => Promise<void>,
  fn: string,
  total: number,
): Promise<void> {
  const chunks = Math.ceil(total / CHUNK);
  for (let c = 0; c < chunks; c++) await ensure(fn, c);
}
