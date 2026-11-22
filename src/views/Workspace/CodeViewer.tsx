import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
  type MouseEvent,
} from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { GlassButton, GlassSearchField, GlassTabs } from '../../ui/glass';
import { IconClose, IconDown, IconUp } from '../../ui/icons';
import { useAppStore, displayName } from '../../store/appStore';
import type { CodeKind, CodeLine, Scope } from '../../api/types';
import { tokenizeAsm, tokenizeC, tokenizePlain, type Token } from '../../lib/tokenizer/asm';
import { copyCurrentScope } from '../../lib/copyScope';
import { goToInput, parseHex } from '../../lib/navigation';
import { ptBR } from '../../i18n/pt-BR';
import { CHUNK, ensureFunctionLoaded, useLayout, useLineCache, type RowRef } from './codeData';

const ROW_H = 22;

/**
 * Visualizador de código (camada de conteúdo: sem vidro, sem backdrop-filter).
 * Linhas não são botões: o clique é delegado ao contêiner. O único elemento
 * clicável dentro de uma linha é o destino de call/jmp.
 */
export function CodeViewer() {
  const scope = useAppStore((s) => s.scope);
  const kind = useAppStore((s) => s.codeKind);
  const selected = useAppStore((s) => s.selectedFunction);
  const functions = useAppStore((s) => s.functions);
  const renames = useAppStore((s) => s.renames);
  const selectAll = useAppStore((s) => s.selectAllActive);
  const demo = useAppStore((s) => s.engine?.isDemo ?? false);
  const hasCode = useAppStore((s) => s.engine?.hasCode ?? true);
  const partial = useAppStore((s) => s.engine?.partial ?? false);
  const jump = useAppStore((s) => s.jumpRequest);
  const searchOpen = useAppStore((s) => s.searchOpen);
  const setSelectAll = useAppStore((s) => s.setSelectAll);
  const setCodeKind = useAppStore((s) => s.setCodeKind);
  const setScope = useAppStore((s) => s.setScope);
  const setCursorLine = useAppStore((s) => s.setCursorLine);
  const consumeJump = useAppStore((s) => s.consumeJump);
  const setSearch = useAppStore((s) => s.setSearch);
  const setScopeTotalLines = useAppStore((s) => s.setScopeTotalLines);
  const pushToast = useAppStore((s) => s.pushToast);

  const { rows, error } = useLayout(scope, kind, selected, functions);
  const { ensure, get, version } = useLineCache(kind);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [cursor, setCursor] = useState<number | null>(null);
  const [word, setWord] = useState<string | null>(null);
  const [matches, setMatches] = useState<number[]>([]);
  const [matchPos, setMatchPos] = useState(0);
  const [query, setQuery] = useState('');
  const [busy, setBusy] = useState(false);
  // Busca: só o resultado do pedido mais recente vale.
  const searchSeq = useRef(0);

  const fnName = (addr: string) => {
    const f = functions.find((x) => x.address === addr);
    return f ? displayName(f, renames) : addr;
  };

  const virtualizer = useVirtualizer({
    count: rows?.length ?? 0,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_H,
    overscan: 24,
  });
  const items = virtualizer.getVirtualItems();

  useEffect(() => {
    virtualizer.measure();
  }, [rows, virtualizer]);

  useEffect(() => {
    setScopeTotalLines(rows ? rows.filter((r) => r.t === 'line').length : null);
  }, [rows, setScopeTotalLines]);

  useEffect(() => {
    setCursor(null);
    setWord(null);
    setMatches([]);
    searchSeq.current++;
  }, [selected, scope, kind]);

  // Fatias visíveis (com folga) sob demanda.
  const firstVisible = items[0]?.index ?? -1;
  const lastVisible = items[items.length - 1]?.index ?? -1;
  useEffect(() => {
    if (!rows) return;
    const wanted = new Map<string, { fn: string; chunk: number }>();
    for (let i = Math.max(0, firstVisible - 40); i <= Math.min(rows.length - 1, lastVisible + 40); i++) {
      const r = rows[i];
      if (r?.t === 'line') {
        const chunk = Math.floor(r.idx / CHUNK);
        wanted.set(`${r.fn}|${chunk}`, { fn: r.fn, chunk });
      }
    }
    wanted.forEach(({ fn, chunk }) => {
      ensure(fn, chunk).catch((e: unknown) =>
        pushToast({ tone: 'error', message: `Falha ao carregar código: ${String(e)}` }),
      );
    });
  }, [rows, firstVisible, lastVisible, ensure, pushToast]);

  const loadAll = useCallback(async (): Promise<RowRef[]> => {
    if (!rows) return [];
    setBusy(true);
    try {
      const fns = scope === 'program' ? functions.map((f) => f.address) : selected ? [selected] : [];
      for (const fn of fns) {
        const total = rows.reduce((n, r) => (r.t === 'line' && r.fn === fn ? n + 1 : n), 0);
        await ensureFunctionLoaded(ensure, fn, total);
      }
      return rows;
    } finally {
      setBusy(false);
    }
  }, [rows, scope, functions, selected, ensure]);

  const moveCursor = useCallback(
    (i: number) => {
      if (!rows || rows.length === 0) return;
      const idx = Math.max(0, Math.min(rows.length - 1, i));
      setCursor(idx);
      const r = rows[idx];
      const l = r?.t === 'line' ? get(r.fn, r.idx) : undefined;
      setCursorLine(idx + 1, l?.address ?? null);
      virtualizer.scrollToIndex(idx, { align: 'auto' });
    },
    [rows, get, setCursorLine, virtualizer],
  );

  // Pedido de salto para um endereço dentro da função selecionada.
  useEffect(() => {
    if (!jump || !rows) return;
    let alive = true;
    (async () => {
      const target = parseHex(jump.address);
      const all = await loadAll();
      if (!alive) return;
      const i = all.findIndex((r) => {
        if (r.t !== 'line') return false;
        const l = get(r.fn, r.idx);
        return l !== undefined && parseHex(l.address) === target;
      });
      consumeJump();
      if (i >= 0) moveCursor(i);
      else pushToast({ tone: 'info', message: 'Endereço não encontrado nesta função.' });
    })().catch((e: unknown) => pushToast({ tone: 'error', message: String(e) }));
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jump, rows, version]);

  const runSearch = useCallback(
    async (q: string) => {
      const seq = ++searchSeq.current;
      setQuery(q);
      if (!q) {
        setMatches([]);
        return;
      }
      const all = await loadAll();
      if (seq !== searchSeq.current) return; // chegou uma busca mais nova
      const lq = q.toLowerCase();
      const found: number[] = [];
      all.forEach((r, i) => {
        if (r.t !== 'line') return;
        const l = get(r.fn, r.idx);
        if (!l) return;
        const hay = `${l.address} ${l.bytes ?? ''} ${l.text} ${l.comment ?? ''}`.toLowerCase();
        if (hay.includes(lq)) found.push(i);
      });
      setMatches(found);
      setMatchPos(0);
      if (found[0] !== undefined) virtualizer.scrollToIndex(found[0], { align: 'center' });
    },
    [loadAll, get, virtualizer],
  );

  const stepMatch = (dir: 1 | -1) => {
    if (matches.length === 0) return;
    const next = (matchPos + dir + matches.length) % matches.length;
    setMatchPos(next);
    virtualizer.scrollToIndex(matches[next] ?? 0, { align: 'center' });
  };

  const closeSearch = () => {
    searchSeq.current++;
    setSearch({ searchOpen: false });
    setQuery('');
    setMatches([]);
    scrollRef.current?.focus();
  };

  const followTarget = (text: string) => {
    // Endereço no início de outra função, ou dentro dela: goToInput seleciona a função certa.
    goToInput(text);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      setSelectAll(true);
      return;
    }
    if (mod && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      setSearch({ searchOpen: true });
      return;
    }
    const page = Math.max(1, Math.floor((scrollRef.current?.clientHeight ?? 400) / ROW_H) - 1);
    const at = cursor ?? -1;
    switch (e.key) {
      case 'ArrowDown':
        e.preventDefault();
        moveCursor(at + 1);
        return;
      case 'ArrowUp':
        e.preventDefault();
        moveCursor(at - 1);
        return;
      case 'PageDown':
        e.preventDefault();
        moveCursor(at + page);
        return;
      case 'PageUp':
        e.preventDefault();
        moveCursor(at - page);
        return;
      case 'Home':
        e.preventDefault();
        moveCursor(0);
        return;
      case 'End':
        e.preventDefault();
        moveCursor((rows?.length ?? 1) - 1);
        return;
      case 'Enter': {
        const r = cursor !== null ? rows?.[cursor] : undefined;
        const target = r?.t === 'line' ? get(r.fn, r.idx)?.target : undefined;
        if (target) {
          e.preventDefault();
          followTarget(target);
        }
        return;
      }
      case 'Escape':
        if (searchOpen) closeSearch();
        setSelectAll(false);
        setWord(null);
        return;
    }
  };

  // Ctrl+C e "Copiar" sempre copiam o escopo completo.
  const onCopy = (e: ClipboardEvent<HTMLDivElement>) => {
    e.preventDefault();
    void copyCurrentScope();
  };

  // Clique delegado: a linha não é um botão.
  const onMouseDown = (e: MouseEvent<HTMLDivElement>) => {
    const el = (e.target as HTMLElement).closest<HTMLElement>('[data-row]');
    if (!el || (e.target as HTMLElement).closest('.tok-link')) return;
    setSelectAll(false);
    moveCursor(Number(el.dataset.row));
  };

  const onDoubleClick = (e: MouseEvent<HTMLDivElement>) => {
    const tok = (e.target as HTMLElement).closest<HTMLElement>('[data-tok]');
    if (!tok) return;
    const kindOf = tok.dataset.tok;
    if (kindOf === 'reg' || kindOf === 'sym' || kindOf === 'mnemonic') {
      const text = tok.textContent ?? '';
      setWord((w) => (w === text ? null : text));
    }
  };

  useEffect(() => {
    if (!selectAll) return;
    const onDown = (ev: globalThis.MouseEvent) => {
      const el = scrollRef.current;
      if (el && !el.contains(ev.target as Node)) setSelectAll(false);
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [selectAll, setSelectAll]);

  const matchSet = useMemo(() => new Set(matches), [matches]);
  const currentMatch = matches[matchPos];

  const kindOptions = [
    { id: 'disasm' as const, label: ptBR.code.disasm },
    { id: 'pseudo' as const, label: ptBR.code.pseudo },
    { id: 'hex' as const, label: ptBR.code.hex },
  ];
  const scopeOptions = [
    { id: 'function' as const, label: ptBR.code.scopeFunction },
    { id: 'program' as const, label: ptBR.code.scopeProgram },
  ];

  const isLoading = !rows && !error;

  return (
    <section className="code" aria-label="Código">
      <header className="code__head">
        <div className="code__title">
          <span className="code__eyebrow">{scope === 'function' ? ptBR.code.scopeFunction : ptBR.code.scopeProgram}</span>
          <h1 className="code__name mono">{scope === 'function' ? (selected ? fnName(selected) : '—') : ptBR.code.allFunctions}</h1>
        </div>
        <div className="code__controls">
          <GlassTabs tabs={kindOptions} value={kind} onChange={(k: CodeKind) => setCodeKind(k)} label="Tipo de código" />
          <GlassTabs tabs={scopeOptions} value={scope} onChange={(s: Scope) => setScope(s)} label="Escopo" />
        </div>
        {demo && (
          <p className="code__demo" role="note">
            <span className="code__demo-dot" aria-hidden="true" />
            {ptBR.code.demoBadge}
          </p>
        )}
        {partial && (
          <p className="code__demo" role="note">
            <span className="code__demo-dot" aria-hidden="true" />
            {ptBR.code.partial}
          </p>
        )}
        {kind === 'pseudo' && <p className="code__note">{ptBR.code.pseudoNote}</p>}
      </header>

      {searchOpen && (
        <div className="code__search">
          <GlassSearchField
            autoFocus
            aria-label={ptBR.code.search}
            placeholder={ptBR.code.search}
            value={query}
            onChange={(e) => void runSearch(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                stepMatch(e.shiftKey ? -1 : 1);
              } else if (e.key === 'Escape') {
                e.preventDefault();
                closeSearch();
              }
            }}
          />
          <span className="code__count mono" aria-live="polite">
            {query ? (matches.length ? ptBR.code.matches(matchPos + 1, matches.length) : ptBR.code.noMatches) : ''}
          </span>
          <GlassButton size="sm" iconOnly aria-label="Ocorrência anterior" onClick={() => stepMatch(-1)} icon={<IconUp size={14} />} />
          <GlassButton size="sm" iconOnly aria-label="Próxima ocorrência" onClick={() => stepMatch(1)} icon={<IconDown size={14} />} />
          <GlassButton size="sm" iconOnly aria-label="Fechar busca" onClick={closeSearch} icon={<IconClose size={14} />} />
        </div>
      )}

      <div
        ref={scrollRef}
        className="code__scroll mono"
        tabIndex={0}
        role="region"
        aria-label={`${kindOptions.find((k) => k.id === kind)?.label ?? ''}: setas movem, Enter segue chamadas`}
        onKeyDown={onKeyDown}
        onCopy={onCopy}
        onMouseDown={onMouseDown}
        onDoubleClick={onDoubleClick}
        data-select-all={selectAll || undefined}
      >
        {isLoading && <div className="code__state">{scope === 'program' ? ptBR.code.loadingProgram : ptBR.code.loading}</div>}
        {error && <div className="code__state code__state--error">{error}</div>}
        {rows && rows.length === 0 && <div className="code__state">{hasCode ? ptBR.code.empty : ptBR.code.noCode}</div>}
        {busy && <div className="code__busy" role="status">{ptBR.code.loading}</div>}
        {rows && rows.length > 0 && (
          <div className="code__inner" style={{ height: virtualizer.getTotalSize() }}>
            {items.map((item) => {
              const r = rows[item.index];
              if (!r) return null;
              return (
                <CodeRow
                  key={item.key}
                  index={item.index}
                  top={item.start}
                  row={r}
                  kind={kind}
                  line={r.t === 'line' ? get(r.fn, r.idx) : undefined}
                  headerName={r.t === 'header' ? fnName(r.fn) : ''}
                  headerAddr={r.t === 'header' ? r.fn : ''}
                  selectAll={selectAll}
                  isCursor={cursor === item.index}
                  isMatch={matchSet.has(item.index)}
                  isCurrentMatch={currentMatch === item.index}
                  word={word}
                  onFollow={followTarget}
                  version={version}
                />
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}

interface RowProps {
  index: number;
  top: number;
  row: RowRef;
  kind: CodeKind;
  line: CodeLine | undefined;
  headerName: string;
  headerAddr: string;
  selectAll: boolean;
  isCursor: boolean;
  isMatch: boolean;
  isCurrentMatch: boolean;
  word: string | null;
  onFollow: (address: string) => void;
  version: number;
}

const CodeRow = memo(function CodeRow(p: RowProps) {
  const cls = [
    'code__row',
    p.row.t === 'header' && 'code__row--header',
    p.selectAll && p.row.t !== 'blank' && 'is-selected',
    p.isCursor && 'is-cursor',
    p.isMatch && 'is-match',
    p.isCurrentMatch && 'is-current-match',
  ]
    .filter(Boolean)
    .join(' ');
  const style = { transform: `translateY(${p.top}px)`, height: ROW_H };

  if (p.row.t === 'header') {
    return (
      <div className={cls} style={style} data-row={p.index}>
        <span className="code__gutter" aria-hidden="true">{p.index + 1}</span>
        {`; ${p.headerName} @ ${p.headerAddr}`}
      </div>
    );
  }
  if (p.row.t === 'blank') return <div className="code__row" style={style} data-row={p.index} />;

  if (!p.line) {
    return (
      <div className={cls} style={style} data-row={p.index}>
        <span className="code__gutter" aria-hidden="true">{p.index + 1}</span>
        <span className="code__placeholder">…</span>
      </div>
    );
  }

  const l = p.line;
  const tokens: Token[] =
    p.kind === 'disasm' ? tokenizeAsm(l.text) : p.kind === 'pseudo' ? tokenizeC(l.text) : tokenizePlain(l.text);

  return (
    <div className={cls} style={style} data-row={p.index}>
      <span className="code__gutter" aria-hidden="true">{p.index + 1}</span>
      <span className="code__addr">{l.address}</span>
      {p.kind !== 'pseudo' && <span className="code__bytes">{l.bytes ?? ''}</span>}
      <span className="code__text">
        {tokens.map((t, i) => {
          const tokCls = ['tok', `tok--${t.kind}`, p.word && t.text === p.word && 'is-occurrence'].filter(Boolean).join(' ');
          if (t.kind === 'target') {
            // Único elemento clicável da linha. Teclado: Enter na linha do cursor.
            return (
              <button
                key={i}
                type="button"
                tabIndex={-1}
                className={`${tokCls} tok-link`}
                title={`Ir para ${t.text}`}
                onClick={() => p.onFollow(t.text)}
              >
                {t.text}
              </button>
            );
          }
          return (
            <span key={i} className={tokCls} data-tok={t.kind}>
              {t.text}
            </span>
          );
        })}
        {l.comment && <span className="tok tok--comment">{`  ; ${l.comment}`}</span>}
      </span>
    </div>
  );
});
