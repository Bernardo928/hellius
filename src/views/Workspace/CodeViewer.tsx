import {
  memo,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
} from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { GlassButton, GlassInput, GlassTabs } from '../../ui/glass';
import { useAppStore, displayName } from '../../store/appStore';
import type { CodeKind, CodeLine, Scope } from '../../api/types';
import { tokenizeAsm, tokenizeC, tokenizePlain, type Token } from '../../lib/tokenizer/asm';
import { copyCurrentScope } from '../../lib/copyScope';
import { parseHex } from '../../lib/navigation';
import { ptBR } from '../../i18n/pt-BR';
import { CHUNK, ensureFunctionLoaded, useLayout, useLineCache, type RowRef } from './codeData';

const ROW_H = 20;

export function CodeViewer() {
  const scope = useAppStore((s) => s.scope);
  const kind = useAppStore((s) => s.codeKind);
  const selected = useAppStore((s) => s.selectedFunction);
  const functions = useAppStore((s) => s.functions);
  const renames = useAppStore((s) => s.renames);
  const selectAll = useAppStore((s) => s.selectAllActive);
  const demo = useAppStore((s) => s.engine?.isDemo ?? false);
  const jump = useAppStore((s) => s.jumpRequest);
  const searchOpen = useAppStore((s) => s.searchOpen);
  const setSelectAll = useAppStore((s) => s.setSelectAll);
  const setCodeKind = useAppStore((s) => s.setCodeKind);
  const setScope = useAppStore((s) => s.setScope);
  const setCursorLine = useAppStore((s) => s.setCursorLine);
  const selectFunction = useAppStore((s) => s.selectFunction);
  const requestJump = useAppStore((s) => s.requestJump);
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

  // Quando o conjunto de linhas muda, remede para o virtualizador re-renderizar com a janela certa.
  useEffect(() => {
    virtualizer.measure();
  }, [rows, virtualizer]);

  useEffect(() => {
    setScopeTotalLines(rows ? rows.filter((r) => r.t === 'line').length : null);
  }, [rows, setScopeTotalLines]);

  // Reset ao trocar função, escopo ou tipo.
  useEffect(() => {
    setCursor(null);
    setWord(null);
    setMatches([]);
  }, [selected, scope, kind]);

  // Carrega as fatias visíveis.
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rows, firstVisible, lastVisible, ensure]);

  // Carregar tudo do escopo (busca e salto). Mostra estado de carregando se demorar.
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

  // Pedido de salto (endereço dentro da função selecionada).
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
      if (i >= 0) {
        setCursor(i);
        virtualizer.scrollToIndex(i, { align: 'center' });
      } else {
        pushToast({ tone: 'info', message: 'Endereço não encontrado nesta função.' });
      }
    })().catch((e: unknown) => pushToast({ tone: 'error', message: String(e) }));
    return () => {
      alive = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [jump, rows, version]);

  // Busca: calcula ocorrências sobre o conjunto completo.
  const runSearch = useCallback(
    async (q: string) => {
      setQuery(q);
      if (!q) {
        setMatches([]);
        return;
      }
      const all = await loadAll();
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

  // Esc desfaz seleção/realce/busca quando o foco está no visualizador.
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'a') {
      e.preventDefault();
      setSelectAll(true);
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      setSearch({ searchOpen: true });
      return;
    }
    if (e.key === 'Escape') {
      if (searchOpen) {
        setSearch({ searchOpen: false });
        setQuery('');
        setMatches([]);
      }
      setSelectAll(false);
      setWord(null);
    }
  };

  // Ctrl+C e "Copiar" sempre copiam o escopo completo.
  const onCopy = (e: ClipboardEvent<HTMLDivElement>) => {
    e.preventDefault();
    void copyCurrentScope();
  };

  // Clique fora da área desfaz "Selecionar tudo".
  useEffect(() => {
    if (!selectAll) return;
    const onDown = (ev: globalThis.MouseEvent) => {
      const el = scrollRef.current;
      if (el && !el.contains(ev.target as Node)) setSelectAll(false);
    };
    document.addEventListener('mousedown', onDown);
    return () => document.removeEventListener('mousedown', onDown);
  }, [selectAll, setSelectAll]);

  const onRowClick = (i: number, r: RowRef) => {
    setSelectAll(false);
    setCursor(i);
    if (r.t === 'line') {
      const l = get(r.fn, r.idx);
      setCursorLine(i + 1, l?.address ?? null);
    } else {
      setCursorLine(i + 1, null);
    }
  };

  const onTokenClick = (tok: Token) => {
    if (tok.kind !== 'target') return;
    const n = parseHex(tok.text);
    if (n === null) return;
    const hit = functions.find((f) => parseHex(f.address) === n);
    if (hit) selectFunction(hit.address);
    else requestJump(tok.text);
  };

  const onTokenDouble = (tok: Token) => {
    if (tok.kind === 'reg' || tok.kind === 'sym' || tok.kind === 'mnemonic') {
      setWord((w) => (w === tok.text ? null : tok.text));
    }
  };

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

  const headerName = selected ? fnName(selected) : '';
  const isLoading = !rows && !error;

  return (
    <div className="code" aria-label="Visualizador de código">
      <div className="code__toolbar">
        <GlassTabs tabs={kindOptions} value={kind} onChange={(k: CodeKind) => setCodeKind(k)} label="Tipo de código" />
        <GlassTabs
          tabs={scopeOptions}
          value={scope}
          onChange={(s: Scope) => setScope(s)}
          label="Escopo"
        />
        <span className="code__title mono">
          {scope === 'function' ? headerName : ptBR.code.scopeProgram}
        </span>
        {demo && <span className="demo-badge">{ptBR.code.demoBadge}</span>}
      </div>

      {searchOpen && (
        <div className="code__search">
          <GlassInput
            autoFocus
            aria-label={ptBR.code.search}
            placeholder={ptBR.code.search}
            value={query}
            onChange={(e) => void runSearch(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                stepMatch(e.shiftKey ? -1 : 1);
              }
            }}
          />
          <span className="code__count mono" aria-live="polite">
            {query
              ? matches.length
                ? ptBR.code.matches(matchPos + 1, matches.length)
                : ptBR.code.noMatches
              : ''}
          </span>
          <GlassButton size="sm" variant="ghost" iconOnly aria-label="Anterior" onClick={() => stepMatch(-1)} icon="↑" />
          <GlassButton size="sm" variant="ghost" iconOnly aria-label="Próxima" onClick={() => stepMatch(1)} icon="↓" />
        </div>
      )}

      {/* Área de rolagem: sem backdrop-filter aqui (regra de desempenho). */}
      <div
        ref={scrollRef}
        className="code__scroll mono"
        tabIndex={0}
        role="region"
        aria-label={ptBR.code.disasm}
        onKeyDown={onKeyDown}
        onCopy={onCopy}
      >
        {isLoading && <div className="code__state">{scope === 'program' ? ptBR.code.loadingProgram : ptBR.code.loading}</div>}
        {error && <div className="code__state code__state--error">{error}</div>}
        {rows && rows.length === 0 && <div className="code__state">{ptBR.code.empty}</div>}
        {busy && <div className="code__busy" role="status">{ptBR.code.loading}</div>}
        {rows && rows.length > 0 && (
          <div className="code__inner" style={{ height: virtualizer.getTotalSize() }}>
            {items.map((item) => {
              const r = rows[item.index];
              if (!r) return null;
              return (
                <div
                  key={item.key}
                  className="code__row-wrap"
                  style={{ transform: `translateY(${item.start}px)`, height: ROW_H }}
                >
                  <CodeRow
                    index={item.index}
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
                    onClick={onRowClick}
                    onTokenClick={onTokenClick}
                    onTokenDouble={onTokenDouble}
                    version={version}
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

interface RowProps {
  index: number;
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
  onClick: (i: number, r: RowRef) => void;
  onTokenClick: (t: Token) => void;
  onTokenDouble: (t: Token) => void;
  version: number;
}

/** Uma linha. Memoizada: só re-renderiza quando seus próprios dados mudam. */
const CodeRow = memo(function CodeRow(p: RowProps) {
  const cls = [
    'code__row',
    p.selectAll && p.row.t !== 'blank' && 'is-selected',
    p.isCursor && 'is-cursor',
    p.isMatch && 'is-match',
    p.isCurrentMatch && 'is-current-match',
  ]
    .filter(Boolean)
    .join(' ');

  const onClick = () => p.onClick(p.index, p.row);

  if (p.row.t === 'header') {
    return (
      <div className="code__row code__row--header" onClick={onClick}>
        {`; ---- ${p.headerName} @ ${p.headerAddr} ----`}
      </div>
    );
  }
  if (p.row.t === 'blank') return <div className="code__row" />;

  if (!p.line) {
    return (
      <div className={cls} onClick={onClick}>
        <span className="code__gutter">{p.index + 1}</span>
        <span className="code__placeholder">…</span>
      </div>
    );
  }

  const l = p.line;
  const tokens: Token[] =
    p.kind === 'disasm' ? tokenizeAsm(l.text) : p.kind === 'pseudo' ? tokenizeC(l.text) : tokenizePlain(l.text);

  return (
    <div className={cls} onClick={onClick}>
      <span className="code__gutter" aria-hidden="true">{p.index + 1}</span>
      <span className="code__addr">{l.address}</span>
      {p.kind !== 'pseudo' && <span className="code__bytes">{l.bytes ?? ''}</span>}
      <span className="code__text">
        {tokens.map((t, i) => (
          <span
            key={i}
            className={[
              `tok tok--${t.kind}`,
              p.word && t.text === p.word && 'is-occurrence',
              t.kind === 'target' && 'tok--link',
            ]
              .filter(Boolean)
              .join(' ')}
            onClick={(e) => {
              if (t.kind === 'target') {
                e.stopPropagation();
                p.onTokenClick(t);
              }
            }}
            onDoubleClick={(e) => {
              e.stopPropagation();
              p.onTokenDouble(t);
            }}
          >
            {t.text}
          </span>
        ))}
        {l.comment && <span className="tok tok--comment">{`  ; ${l.comment}`}</span>}
      </span>
    </div>
  );
});
