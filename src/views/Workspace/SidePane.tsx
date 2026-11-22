import { useMemo, useState } from 'react';
import { GlassSearchField, GlassTabs } from '../../ui/glass';
import { useAppStore, displayName, type SideTab } from '../../store/appStore';
import { copyText } from '../../lib/clipboard';
import { ptBR } from '../../i18n/pt-BR';
import { VirtualList } from './VirtualList';

export function SidePane() {
  const tab = useAppStore((s) => s.sideTab);
  const setTab = useAppStore((s) => s.setSideTab);
  const [filter, setFilter] = useState('');
  const tabs = [
    { id: 'functions' as const, label: ptBR.tabs.functions },
    { id: 'strings' as const, label: ptBR.tabs.strings },
    { id: 'imports' as const, label: ptBR.tabs.imports },
    { id: 'sections' as const, label: ptBR.tabs.sections },
  ];
  return (
    <div className="side">
      <div className="side__head">
        <GlassTabs tabs={tabs} value={tab} onChange={(t: SideTab) => setTab(t)} label="Listas" className="side__tabs" />
        <GlassSearchField
          aria-label={ptBR.lists.filter}
          placeholder={ptBR.lists.filter}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
      </div>
      <div className="side__body">
        {tab === 'functions' && <FunctionsList filter={filter} />}
        {tab === 'strings' && <StringsList filter={filter} />}
        {tab === 'imports' && <ImportsList filter={filter} />}
        {tab === 'sections' && <SectionsList filter={filter} />}
      </div>
    </div>
  );
}

const match = (hay: string, q: string) => !q || hay.toLowerCase().includes(q.trim().toLowerCase());

function FunctionsList({ filter }: { filter: string }) {
  const functions = useAppStore((s) => s.functions);
  const renames = useAppStore((s) => s.renames);
  const selected = useAppStore((s) => s.selectedFunction);
  const selectFunction = useAppStore((s) => s.selectFunction);
  const list = useMemo(
    () => functions.filter((f) => match(`${displayName(f, renames)} ${f.address}`, filter)),
    [functions, renames, filter],
  );
  return (
    <VirtualList
      items={list}
      rowHeight={44}
      label={ptBR.tabs.functions}
      empty={ptBR.lists.empty}
      activeIndex={list.findIndex((f) => f.address === selected)}
      getKey={(f) => f.address}
      render={(f) => (
        <button
          type="button"
          className={`list__row${selected === f.address ? ' is-active' : ''}`}
          aria-current={selected === f.address ? 'true' : undefined}
          onClick={() => selectFunction(f.address)}
        >
          <span className="list__title">{displayName(f, renames)}</span>
          <span className="list__meta mono">
            {f.address} · {f.sizeBytes} B
          </span>
        </button>
      )}
    />
  );
}

function StringsList({ filter }: { filter: string }) {
  const strings = useAppStore((s) => s.strings);
  const pushToast = useAppStore((s) => s.pushToast);
  const list = useMemo(() => strings.filter((x) => match(`${x.value} ${x.address}`, filter)), [strings, filter]);
  return (
    <VirtualList
      items={list}
      rowHeight={44}
      label={ptBR.tabs.strings}
      empty={ptBR.lists.empty}
      getKey={(x) => x.address}
      render={(x) => (
        <button
          type="button"
          className="list__row"
          title="Clique para copiar o endereço"
          onClick={async () => {
            const ok = await copyText(x.address);
            pushToast({ tone: ok ? 'success' : 'error', message: ok ? `Endereço ${x.address} copiado` : 'Não foi possível copiar.' });
          }}
        >
          <span className="list__title">{x.value}</span>
          <span className="list__meta mono">{x.address}</span>
        </button>
      )}
    />
  );
}

function ImportsList({ filter }: { filter: string }) {
  const imports = useAppStore((s) => s.imports);
  const list = useMemo(() => imports.filter((x) => match(`${x.library} ${x.name}`, filter)), [imports, filter]);
  return (
    <VirtualList
      items={list}
      rowHeight={44}
      label={ptBR.tabs.imports}
      empty={ptBR.lists.empty}
      getKey={(x) => `${x.library}:${x.name}`}
      render={(x) => (
        <div className="list__row list__row--static">
          <span className="list__title">{x.name}</span>
          <span className="list__meta mono">{x.library}</span>
        </div>
      )}
    />
  );
}

function SectionsList({ filter }: { filter: string }) {
  const file = useAppStore((s) => s.file);
  const sections = useMemo(
    () => (file?.sections ?? []).filter((x) => match(`${x.name} ${x.address}`, filter)),
    [file, filter],
  );
  return (
    <VirtualList
      items={sections}
      rowHeight={44}
      label={ptBR.tabs.sections}
      empty={ptBR.lists.sectionsEmpty}
      getKey={(x) => `${x.name}:${x.address}`}
      render={(x) => (
        <div className="list__row list__row--static">
          <span className="list__title">{x.name || '(sem nome)'}</span>
          <span className="list__meta mono">
            {x.address} · {x.size} B · {x.flags}
          </span>
        </div>
      )}
    />
  );
}
