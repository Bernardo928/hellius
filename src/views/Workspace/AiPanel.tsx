import { useEffect, useState, type FormEvent } from 'react';
import { GlassButton, GlassInput } from '../../ui/glass';
import { IconCopy, IconSpark } from '../../ui/icons';
import { useAppStore, displayName } from '../../store/appStore';
import { api } from '../../api/hellius';
import { cancelExplain, explainFunction, refreshAiStatus } from '../../lib/ai';
import { copyText } from '../../lib/clipboard';
import { errorMessage } from '../../lib/analysis';
import { ptBR } from '../../i18n/pt-BR';

const t = ptBR.ai;

/** Painel de IA no inspetor: chave, consentimento e a explicação em streaming. */
export function AiPanel() {
  const ai = useAppStore((s) => s.ai);
  const selected = useAppStore((s) => s.selectedFunction);
  const functions = useAppStore((s) => s.functions);
  const renames = useAppStore((s) => s.renames);
  const patchAi = useAppStore((s) => s.patchAi);
  const pushToast = useAppStore((s) => s.pushToast);
  const [key, setKey] = useState('');
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    void refreshAiStatus();
  }, []);

  const status = ai.status;
  const fn = functions.find((f) => f.address === selected);
  const explainedName = (() => {
    const f = functions.find((x) => x.address === ai.functionAddress);
    return f ? displayName(f, renames) : ai.functionAddress;
  })();

  const saveKey = async (e: FormEvent) => {
    e.preventDefault();
    if (!key.trim()) return;
    setSaving(true);
    try {
      patchAi({ status: await api.aiSaveSettings({ apiKey: key.trim() }) });
      setKey('');
    } catch (err) {
      pushToast({ tone: 'error', message: errorMessage(err) });
    } finally {
      setSaving(false);
    }
  };

  const consentAndRun = async () => {
    try {
      patchAi({ status: await api.aiSaveSettings({ consent: true }) });
      if (selected) void explainFunction(selected);
    } catch (err) {
      pushToast({ tone: 'error', message: errorMessage(err) });
    }
  };

  const forgetKey = async () => {
    try {
      patchAi({ status: await api.aiSaveSettings({ clearKey: true }) });
    } catch (err) {
      pushToast({ tone: 'error', message: errorMessage(err) });
    }
  };

  // 1. Sem chave: pede a chave.
  if (!status?.configured) {
    return (
      <div className="ai">
        <h2 className="insp__heading">{t.title}</h2>
        <p className="insp__muted">{t.needKey}</p>
        <form className="ai__key" onSubmit={saveKey}>
          <label className="insp__label" htmlFor="ai-key">
            {t.keyLabel}
          </label>
          <GlassInput
            id="ai-key"
            type="password"
            autoComplete="off"
            spellCheck={false}
            placeholder="sk-ant-…"
            value={key}
            onChange={(e) => setKey(e.target.value)}
          />
          <GlassButton type="submit" variant="primary" disabled={saving || !key.trim()}>
            {t.saveKey}
          </GlassButton>
        </form>
        <p className="ai__fine">{t.keyStorage}</p>
      </div>
    );
  }

  // 2. Com chave, sem consentimento: explica o que será enviado.
  if (!status.consent) {
    return (
      <div className="ai">
        <h2 className="insp__heading">{t.title}</h2>
        <p>{t.consentBody}</p>
        <ul className="ai__list">
          {t.consentItems.map((i) => (
            <li key={i}>{i}</li>
          ))}
        </ul>
        <GlassButton variant="primary" icon={<IconSpark />} onClick={consentAndRun} disabled={!selected}>
          {t.consentButton}
        </GlassButton>
        <p className="ai__fine">{t.model(status.model)}</p>
      </div>
    );
  }

  // 3. Pronto para explicar / explicando / explicado.
  return (
    <div className="ai">
      <div className="ai__head">
        <h2 className="insp__heading">{t.title}</h2>
        {ai.functionAddress && <span className="insp__muted mono">{explainedName}</span>}
      </div>

      {!ai.running && !ai.text && !ai.error && (
        <>
          <p className="insp__muted">{fn ? t.ready(displayName(fn, renames)) : t.noFunction}</p>
          <GlassButton variant="primary" icon={<IconSpark />} disabled={!selected} onClick={() => selected && void explainFunction(selected)}>
            {t.explain}
          </GlassButton>
        </>
      )}

      {(ai.running || ai.text) && (
        <article className="ai__text" aria-live="polite" aria-busy={ai.running}>
          {ai.text || t.thinking}
        </article>
      )}

      {ai.error && (
        <p className="ai__error" role="alert">
          {ai.error}
        </p>
      )}

      <div className="ai__actions">
        {ai.running ? (
          <GlassButton onClick={() => void cancelExplain()}>{t.stop}</GlassButton>
        ) : (
          (ai.text || ai.error) && (
            <GlassButton icon={<IconSpark />} disabled={!selected} onClick={() => selected && void explainFunction(selected)}>
              {fn && ai.functionAddress !== selected ? t.explainThis(displayName(fn, renames)) : t.again}
            </GlassButton>
          )
        )}
        {ai.text && !ai.running && (
          <GlassButton
            iconOnly
            aria-label={t.copy}
            icon={<IconCopy size={14} />}
            onClick={async () => {
              const ok = await copyText(ai.text);
              pushToast({ tone: ok ? 'success' : 'error', message: ok ? t.copied : 'Não foi possível copiar.' });
            }}
          />
        )}
      </div>

      <p className="ai__fine">
        {t.disclaimer} {t.model(status.model)}
        {status.source === 'app' && (
          <>
            {' '}
            <button type="button" className="ai__link" onClick={forgetKey}>
              {t.forgetKey}
            </button>
          </>
        )}
      </p>
    </div>
  );
}
