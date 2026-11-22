import { useEffect, useState, type FormEvent } from 'react';
import { GlassButton, GlassChip, GlassInput } from '../../ui/glass';
import { useAppStore, displayName } from '../../store/appStore';
import { ptBR } from '../../i18n/pt-BR';

export function Inspector() {
  const functions = useAppStore((s) => s.functions);
  const selected = useAppStore((s) => s.selectedFunction);
  const renames = useAppStore((s) => s.renames);
  const renameFunction = useAppStore((s) => s.renameFunction);
  const selectFunction = useAppStore((s) => s.selectFunction);
  const fn = functions.find((f) => f.address === selected) ?? null;
  const [draft, setDraft] = useState('');

  useEffect(() => {
    setDraft(fn ? displayName(fn, renames) : '');
  }, [fn, renames]);

  if (!fn) return <div className="inspector__empty">{ptBR.inspector.noSelection}</div>;

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    renameFunction(fn.address, draft);
  };

  const xrefRow = (label: string, addrs: string[]) => (
    <section className="inspector__section">
      <h3>{label}</h3>
      {addrs.length === 0 ? (
        <p className="inspector__muted">{ptBR.inspector.none}</p>
      ) : (
        <div className="inspector__chips">
          {addrs.map((a) => {
            const target = functions.find((f) => f.address === a);
            return (
              <GlassChip key={a} onClick={() => selectFunction(a)} meta={target ? displayName(target, renames) : undefined}>
                {a}
              </GlassChip>
            );
          })}
        </div>
      )}
    </section>
  );

  return (
    <div className="inspector">
      <h2 className="inspector__title mono">{displayName(fn, renames)}</h2>
      <dl className="inspector__grid">
        <dt>{ptBR.inspector.address}</dt>
        <dd className="mono">{fn.address}</dd>
        <dt>{ptBR.inspector.size}</dt>
        <dd>{fn.sizeBytes} B</dd>
        <dt>{ptBR.inspector.blocks}</dt>
        <dd>{fn.blockCount}</dd>
      </dl>
      {xrefRow(ptBR.inspector.xrefsIn, fn.xrefsIn)}
      {xrefRow(ptBR.inspector.xrefsOut, fn.xrefsOut)}
      <form className="inspector__rename" onSubmit={onSubmit}>
        <label htmlFor="rename-input">{ptBR.inspector.rename}</label>
        <div className="inspector__rename-row">
          <GlassInput id="rename-input" value={draft} onChange={(e) => setDraft(e.target.value)} />
          <GlassButton type="submit" size="md" variant="secondary">
            {ptBR.inspector.renameApply}
          </GlassButton>
        </div>
      </form>
    </div>
  );
}
