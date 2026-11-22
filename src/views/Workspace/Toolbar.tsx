import { useEffect, useRef, useState, type FormEvent } from 'react';
import { GlassButton, GlassPanel, GlassSearchField, GlassTabs, GlassToggle, GlassTooltip } from '../../ui/glass';
import {
  IconBack,
  IconCopy,
  IconForward,
  IconInspector,
  IconOpen,
  IconSelectAll,
  IconSettings,
  IconSidebar,
  IconSpark,
} from '../../ui/icons';
import { useAppStore } from '../../store/appStore';
import { THEME_IDS, type ThemeId } from '../../theme/themes';
import { copyCurrentScope } from '../../lib/copyScope';
import { goToInput } from '../../lib/navigation';
import { explainFunction } from '../../lib/ai';
import { ptBR } from '../../i18n/pt-BR';

interface Props {
  sidebarOpen: boolean;
  inspectorOpen: boolean;
  onToggleSidebar: () => void;
  onToggleInspector: () => void;
}

/** Barra de ferramentas unificada: o único vidro do topo. Botões dentro dela são "plain". */
export function Toolbar({ sidebarOpen, inspectorOpen, onToggleSidebar, onToggleInspector }: Props) {
  const file = useAppStore((s) => s.file);
  const resetToPortal = useAppStore((s) => s.resetToPortal);
  const setSelectAll = useAppStore((s) => s.setSelectAll);
  const selectAll = useAppStore((s) => s.selectAllActive);
  const canBack = useAppStore((s) => s.historyIndex > 0);
  const canForward = useAppStore((s) => s.historyIndex < s.history.length - 1);
  const goBack = useAppStore((s) => s.goBack);
  const goForward = useAppStore((s) => s.goForward);
  const selectFunction = useAppStore((s) => s.selectFunction);
  const settingsOpen = useAppStore((s) => s.settingsOpen);
  const setSettings = useAppStore((s) => s.setSettings);
  const selected = useAppStore((s) => s.selectedFunction);
  const hasCode = useAppStore((s) => s.engine?.hasCode ?? false);
  const aiReady = useAppStore((s) => !!s.ai.status?.configured && !!s.ai.status?.consent);
  const openAi = useAppStore((s) => s.openAi);
  const [q, setQ] = useState('');

  if (!file) return null;

  const onSearch = (e: FormEvent) => {
    e.preventDefault();
    if (goToInput(q)) setQ('');
  };

  const subtitle = [file.format, file.arch === 'unknown' ? ptBR.file.unknownArch : file.arch, file.bits ? `${file.bits} ${ptBR.file.bits}` : '']
    .filter(Boolean)
    .join(' · ');

  return (
    <GlassPanel as="header" className="toolbar">
      <div className="toolbar__group">
        <GlassTooltip label={ptBR.top.toggleSidebar}>
          <GlassButton iconOnly aria-label={ptBR.top.toggleSidebar} pressed={sidebarOpen} onClick={onToggleSidebar} icon={<IconSidebar />} />
        </GlassTooltip>
        <GlassTooltip label={`${ptBR.top.back} (Alt+←)`}>
          <GlassButton
            iconOnly
            aria-label={ptBR.top.back}
            disabled={!canBack}
            onClick={() => {
              const a = goBack();
              if (a) selectFunction(a, { fromHistory: true });
            }}
            icon={<IconBack />}
          />
        </GlassTooltip>
        <GlassTooltip label={`${ptBR.top.forward} (Alt+→)`}>
          <GlassButton
            iconOnly
            aria-label={ptBR.top.forward}
            disabled={!canForward}
            onClick={() => {
              const a = goForward();
              if (a) selectFunction(a, { fromHistory: true });
            }}
            icon={<IconForward />}
          />
        </GlassTooltip>
      </div>

      <div className="toolbar__title">
        <span className="toolbar__name" title={file.path}>
          {file.name}
        </span>
        <span className="toolbar__subtitle">{subtitle}</span>
      </div>

      <form className="toolbar__search" onSubmit={onSearch} role="search">
        <GlassSearchField aria-label={ptBR.top.search} placeholder={`${ptBR.top.search}  (Ctrl+G)`} value={q} onChange={(e) => setQ(e.target.value)} />
      </form>

      <div className="toolbar__group toolbar__actions">
        <GlassTooltip label={ptBR.top.newFile}>
          <GlassButton iconOnly aria-label={ptBR.top.newFile} onClick={resetToPortal} icon={<IconOpen />} />
        </GlassTooltip>
        <GlassButton pressed={selectAll} onClick={() => setSelectAll(true)} icon={<IconSelectAll />}>
          {ptBR.top.selectAll}
        </GlassButton>
        <GlassButton variant="primary" onClick={() => void copyCurrentScope()} icon={<IconCopy />}>
          {ptBR.top.copy}
        </GlassButton>
        <GlassTooltip label={selected && hasCode ? ptBR.top.explainTooltip : ptBR.ai.needFunction}>
          <GlassButton
            aria-disabled={!selected || !hasCode ? 'true' : undefined}
            icon={<IconSpark />}
            onClick={() => {
              if (!selected || !hasCode) return;
              openAi();
              // Sem chave ou sem consentimento, o painel pede antes de enviar.
              if (aiReady) void explainFunction(selected);
            }}
          >
            {ptBR.top.explain}
          </GlassButton>
        </GlassTooltip>
        <span className="toolbar__divider" aria-hidden="true" />
        <div className="toolbar__popover-anchor">
          <GlassTooltip label={ptBR.top.settings}>
            <GlassButton
              iconOnly
              aria-label={ptBR.top.settings}
              aria-haspopup="dialog"
              aria-expanded={settingsOpen}
              pressed={settingsOpen}
              onClick={() => setSettings(!settingsOpen)}
              icon={<IconSettings />}
            />
          </GlassTooltip>
          {settingsOpen && <SettingsPopover onClose={() => setSettings(false)} />}
        </div>
        <GlassTooltip label={ptBR.top.toggleInspector}>
          <GlassButton iconOnly aria-label={ptBR.top.toggleInspector} pressed={inspectorOpen} onClick={onToggleInspector} icon={<IconInspector />} />
        </GlassTooltip>
      </div>
    </GlassPanel>
  );
}

export function SettingsPopover({ onClose }: { onClose: () => void }) {
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const reduce = useAppStore((s) => s.reduceTransparency);
  const setReduce = useAppStore((s) => s.setReduceTransparency);
  const ref = useRef<HTMLDivElement>(null);
  const closeRef = useRef(onClose);
  closeRef.current = onClose;

  useEffect(() => {
    ref.current?.querySelector<HTMLElement>('[aria-selected="true"]')?.focus();
    const onDown = (e: MouseEvent) => {
      const anchor = ref.current?.parentElement;
      if (anchor && !anchor.contains(e.target as Node)) closeRef.current();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closeRef.current();
    };
    document.addEventListener('mousedown', onDown);
    window.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      window.removeEventListener('keydown', onKey);
    };
  }, []);

  const tabs = THEME_IDS.map((id) => ({ id, label: ptBR.theme[id] }));
  return (
    <div ref={ref} className="popover">
      <GlassPanel role="dialog" aria-label={ptBR.settings.title} className="popover__panel">
        <h2 className="popover__title">{ptBR.settings.appearance}</h2>
        <GlassTabs tabs={tabs} value={theme} onChange={(t: ThemeId) => setTheme(t)} label={ptBR.theme.label} className="popover__themes" />
        <div className="popover__row">
          <label htmlFor="reduce-transparency">{ptBR.settings.reduceTransparency}</label>
          <GlassToggle id="reduce-transparency" checked={reduce} onChange={setReduce} label={ptBR.settings.reduceTransparency} />
        </div>
        <p className="popover__hint">{ptBR.settings.reduceTransparencyHint}</p>
      </GlassPanel>
    </div>
  );
}
