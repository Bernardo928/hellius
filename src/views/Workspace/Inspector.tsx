import { useEffect, useState, type FormEvent } from 'react';
import { GlassButton, GlassInput } from '../../ui/glass';
import { IconCopy } from '../../ui/icons';
import { useAppStore, displayName } from '../../store/appStore';
import { copyText } from '../../lib/clipboard';
import { ptBR } from '../../i18n/pt-BR';

/** Inspetor: metadados reais do arquivo e dados da função (demonstração). */
export function Inspector() {
  const file = useAppStore((s) => s.file);
  const functions = useAppStore((s) => s.functions);
  const selected = useAppStore((s) => s.selectedFunction);
  const renames = useAppStore((s) => s.renames);
  const renameFunction = useAppStore((s) => s.renameFunction);
  const selectFunction = useAppStore((s) => s.selectFunction);
  const pushToast = useAppStore((s) => s.pushToast);
  const fn = functions.find((f) => f.address === selected) ?? null;
  const [draft, setDraft] = useState('');

  useEffect(() => {
    setDraft(fn ? displayName(fn, renames) : '');
  }, [fn, renames]);

  const copyHash = async () => {
    if (!file) return;
    const ok = await copyText(file.sha256);
    pushToast({ tone: ok ? 'success' : 'error', message: ok ? ptBR.top.hashCopied : 'Não foi possível copiar.' });
  };

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (fn) renameFunction(fn.address, draft);
  };

  const xrefs = (label: string, addrs: string[]) => (
    <section className="insp__section">
      <h3 className="insp__label">{label}</h3>
      {addrs.length === 0 ? (
        <p className="insp__muted">{ptBR.inspector.none}</p>
      ) : (
        <ul className="insp__xrefs">
          {addrs.map((a) => {
            const t = functions.find((f) => f.address === a);
            return (
              <li key={a}>
                <button type="button" className="insp__xref" onClick={() => selectFunction(a)}>
                  <span className="mono">{a}</span>
                  <span className="insp__muted">{t ? displayName(t, renames) : ''}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );

  return (
    <div className="insp">
      {file && (
        <section className="insp__section">
          <h2 className="insp__heading">Arquivo</h2>
          <dl className="insp__grid">
            <dt>{ptBR.file.format}</dt>
            <dd>{file.format === 'Raw' ? ptBR.file.rawFormat : file.format}</dd>
            <dt>{ptBR.file.arch}</dt>
            <dd className="mono">{file.arch === 'unknown' ? ptBR.file.unknownArch : file.arch}</dd>
            <dt>{ptBR.inspector.size}</dt>
            <dd>{file.sizeBytes.toLocaleString('pt-BR')} B</dd>
            <dt>Entrada</dt>
            <dd className="mono">{file.entryPoint}</dd>
          </dl>
          <div className="insp__hash">
            <span className="insp__label">SHA-256</span>
            <span className="insp__hash-value mono" title={file.sha256}>
              {file.sha256}
            </span>
            <GlassButton size="sm" iconOnly aria-label={ptBR.top.copyHash} onClick={copyHash} icon={<IconCopy size={14} />} />
          </div>
        </section>
      )}

      {fn ? (
        <>
          <section className="insp__section">
            <h2 className="insp__heading mono">{displayName(fn, renames)}</h2>
            <dl className="insp__grid">
              <dt>{ptBR.inspector.address}</dt>
              <dd className="mono">{fn.address}</dd>
              <dt>{ptBR.inspector.size}</dt>
              <dd>{fn.sizeBytes} B</dd>
              <dt>{ptBR.inspector.blocks}</dt>
              <dd>{fn.blockCount}</dd>
            </dl>
          </section>
          {xrefs(ptBR.inspector.xrefsIn, fn.xrefsIn)}
          {xrefs(ptBR.inspector.xrefsOut, fn.xrefsOut)}
          <form className="insp__section insp__rename" onSubmit={onSubmit}>
            <label className="insp__label" htmlFor="rename-input">
              {ptBR.inspector.rename}
            </label>
            <div className="insp__rename-row">
              <GlassInput id="rename-input" value={draft} onChange={(e) => setDraft(e.target.value)} />
              <GlassButton type="submit" size="md">
                {ptBR.inspector.renameApply}
              </GlassButton>
            </div>
          </form>
        </>
      ) : (
        <p className="insp__muted">{ptBR.inspector.noSelection}</p>
      )}
    </div>
  );
}
