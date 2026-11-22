import { useState, type FormEvent } from 'react';
import { GlassButton, GlassChip, GlassInput, GlassTooltip } from '../../ui/glass';
import { useAppStore } from '../../store/appStore';
import { THEME_IDS, type ThemeId } from '../../theme/themes';
import { copyText } from '../../lib/clipboard';
import { copyCurrentScope } from '../../lib/copyScope';
import { goToInput } from '../../lib/navigation';
import { ptBR } from '../../i18n/pt-BR';

export function TopBar() {
  const file = useAppStore((s) => s.file);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const resetToPortal = useAppStore((s) => s.resetToPortal);
  const setSelectAll = useAppStore((s) => s.setSelectAll);
  const selectAll = useAppStore((s) => s.selectAllActive);
  const pushToast = useAppStore((s) => s.pushToast);
  const [q, setQ] = useState('');

  if (!file) return null;

  const onSearch = (e: FormEvent) => {
    e.preventDefault();
    if (goToInput(q)) setQ('');
  };

  const cycleTheme = () => {
    const i = THEME_IDS.indexOf(theme);
    const next: ThemeId = THEME_IDS[(i + 1) % THEME_IDS.length] ?? 'black';
    setTheme(next);
  };

  const copyHash = async () => {
    const ok = await copyText(file.sha256);
    pushToast({ tone: ok ? 'success' : 'error', message: ok ? ptBR.top.hashCopied : 'Não foi possível copiar.' });
  };

  return (
    <header className="topbar glass glass-floating">
      <div className="topbar__inner">
        <GlassButton size="sm" variant="ghost" onClick={resetToPortal}>
          {ptBR.top.newFile}
        </GlassButton>
        <span className="topbar__name" title={file.path}>
          {file.name}
        </span>
        <span className="glass-chip topbar__chip">{file.format}</span>
        <span className="glass-chip topbar__chip mono">
          {file.arch}
          {file.bits ? ` · ${file.bits} ${ptBR.file.bits}` : ''}
        </span>
        <GlassChip className="topbar__chip mono" title={file.sha256} onClick={copyHash} aria-label={`${ptBR.top.copyHash}: ${file.sha256}`}>
          sha256 {file.sha256.slice(0, 12)}…
        </GlassChip>

        <form className="topbar__search" onSubmit={onSearch} role="search">
          <GlassInput
            aria-label={ptBR.top.search}
            placeholder={ptBR.top.search}
            value={q}
            onChange={(e) => setQ(e.target.value)}
          />
        </form>

        <div className="topbar__actions">
          <GlassButton size="sm" variant="secondary" pressed={selectAll} onClick={() => setSelectAll(true)}>
            {ptBR.top.selectAll}
          </GlassButton>
          <GlassButton size="sm" variant="secondary" onClick={() => void copyCurrentScope()}>
            {ptBR.top.copy}
          </GlassButton>
          <GlassTooltip label={ptBR.top.explainTooltip}>
            <GlassButton size="sm" variant="secondary" disabled>
              {ptBR.top.explain}
            </GlassButton>
          </GlassTooltip>
          <GlassButton size="sm" variant="ghost" onClick={cycleTheme} aria-label={`${ptBR.theme.label}: ${ptBR.theme[theme]}`}>
            {ptBR.theme[theme]}
          </GlassButton>
        </div>
      </div>
    </header>
  );
}
