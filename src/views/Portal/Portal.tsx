import { useEffect, useRef } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';
import { GlassButton, GlassTooltip } from '../../ui/glass';
import { IconInfo, IconOpen, IconSettings } from '../../ui/icons';
import { SettingsPopover } from '../Workspace/Toolbar';
import { useAppStore } from '../../store/appStore';
import { openFile, errorMessage } from '../../lib/analysis';
import { ptBR } from '../../i18n/pt-BR';
import { PortalCanvas } from './PortalCanvas';
import './Portal.css';

const DIALOG_FILTERS = [
  { name: 'Binários (exe, dll, sys, so, elf, bin)', extensions: ['exe', 'dll', 'sys', 'so', 'elf', 'bin', 'o', 'dylib'] },
  { name: 'Todos os arquivos', extensions: ['*'] },
];

interface PortalProps {
  onOpenInfo: () => void;
}

export function Portal({ onOpenInfo }: PortalProps) {
  const dragging = useAppStore((s) => s.dragging);
  const analyzing = useAppStore((s) => s.screen === 'analyzing');
  const progress = useAppStore((s) => s.progress);
  const recent = useAppStore((s) => s.recent);
  const settingsOpen = useAppStore((s) => s.settingsOpen);
  const setSettings = useAppStore((s) => s.setSettings);
  const fileName = useAppStore((s) => s.file?.name ?? null);
  const setDragging = useAppStore((s) => s.setDragging);
  const pushToast = useAppStore((s) => s.pushToast);
  const portalRef = useRef<HTMLDivElement>(null);

  const done = analyzing && progress?.phase === 'Concluído';

  // Arrastar e soltar: evento nativo do Tauri, pois o drop do HTML5 não traz o caminho.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    // Fora do Tauri (ex.: navegador de desenvolvimento) não há webview nativo.
    if (!('__TAURI_INTERNALS__' in window)) return;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === 'enter' || p.type === 'over') {
          setDragging(true);
        } else if (p.type === 'leave') {
          setDragging(false);
        } else if (p.type === 'drop') {
          setDragging(false);
          const paths = p.paths;
          // Arquivo solto durante uma análise é ignorado (evita duas análises disputando a sessão).
          if (paths.length === 0 || useAppStore.getState().screen === 'analyzing') return;
          if (paths.length > 1) {
            pushToast({ tone: 'info', message: ptBR.portal.ignoredFiles(paths.length - 1) });
          }
          void openFile(paths[0] as string);
        }
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch((e: unknown) => pushToast({ tone: 'error', message: errorMessage(e) }));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [setDragging, pushToast]);

  const chooseFile = async () => {
    try {
      const picked = await open({ multiple: false, directory: false, filters: DIALOG_FILTERS });
      if (typeof picked === 'string') void openFile(picked);
    } catch (e) {
      pushToast({ tone: 'error', message: errorMessage(e) });
    }
  };

  const speed = dragging ? 3 : analyzing && !done ? 4 : 1;
  const className = [
    'portal',
    dragging && 'is-dragging',
    analyzing && 'is-analyzing',
    done && 'is-collapsing',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <section className="portal-screen" aria-label="Portal">
      <header className="portal-screen__top">
        <GlassTooltip label={ptBR.about.title}>
          <GlassButton iconOnly aria-label={ptBR.about.title} onClick={onOpenInfo} icon={<IconInfo />} />
        </GlassTooltip>
        <div className="toolbar__popover-anchor">
          <GlassTooltip label={ptBR.settings.title}>
            <GlassButton
              iconOnly
              aria-label={ptBR.settings.title}
              aria-haspopup="dialog"
              aria-expanded={settingsOpen}
              pressed={settingsOpen}
              onClick={() => setSettings(!settingsOpen)}
              icon={<IconSettings />}
            />
          </GlassTooltip>
          {settingsOpen && <SettingsPopover onClose={() => setSettings(false)} />}
        </div>
      </header>

      <div className="portal-screen__center">
        <div
          ref={portalRef}
          className={className}
          role="button"
          tabIndex={0}
          aria-label={ptBR.portal.open}
          onClick={() => !analyzing && void chooseFile()}
          onKeyDown={(e) => {
            if (!analyzing && (e.key === 'Enter' || e.key === ' ')) {
              e.preventDefault();
              void chooseFile();
            }
          }}
        >
          <div className="portal__ring" aria-hidden="true" />
          <PortalCanvas speed={speed} size={340} />
          <div className="portal__core">
            <span className="portal__name">Hellius</span>
          </div>
          {analyzing && fileName && <span className="portal__flying">{fileName}</span>}
        </div>

        <p className="portal-screen__hint" aria-live="polite">
          {analyzing
            ? `${progress?.phase ?? ptBR.analysis.phaseLabel}${progress && progress.step ? ` (${progress.step}/${progress.total})` : ''}…`
            : dragging
              ? ptBR.portal.hintDrop
              : ptBR.portal.hint}
        </p>

        {!analyzing && (
          <GlassButton variant="primary" size="lg" icon={<IconOpen />} onClick={() => void chooseFile()}>
            {ptBR.portal.open}
          </GlassButton>
        )}

        {!analyzing && recent.length > 0 && (
          <section className="recent" aria-labelledby="recent-title">
            <h2 id="recent-title" className="recent__title">
              {ptBR.portal.recent}
            </h2>
            <ul className="recent__list">
              {recent.map((r) => (
                <li key={r.path}>
                  <button type="button" className="recent__row" onClick={() => void openFile(r.path)}>
                    <span className="recent__name">{r.name}</span>
                    <span className="recent__path mono">{r.path}</span>
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </section>
  );
}
