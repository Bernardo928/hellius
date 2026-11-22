import { useEffect, useRef } from 'react';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { open } from '@tauri-apps/plugin-dialog';
import { GlassButton, GlassChip, GlassPanel, GlassTooltip } from '../../ui/glass';
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
  onOpenTheme: () => void;
}

export function Portal({ onOpenInfo, onOpenTheme }: PortalProps) {
  const dragging = useAppStore((s) => s.dragging);
  const analyzing = useAppStore((s) => s.screen === 'analyzing');
  const progress = useAppStore((s) => s.progress);
  const recent = useAppStore((s) => s.recent);
  const theme = useAppStore((s) => s.theme);
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
          if (paths.length === 0) return;
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
          <GlassButton variant="ghost" iconOnly size="md" aria-label={ptBR.about.title} onClick={onOpenInfo} icon={<InfoIcon />} />
        </GlassTooltip>
        <GlassTooltip label={`${ptBR.theme.label}: ${ptBR.theme[theme]}`}>
          <GlassButton variant="ghost" iconOnly size="md" aria-label={ptBR.theme.label} onClick={onOpenTheme} icon={<ThemeIcon />} />
        </GlassTooltip>
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
          <GlassPanel className="portal-screen__actions" floating>
            <GlassButton variant="primary" size="lg" onClick={() => void chooseFile()}>
              {ptBR.portal.open}
            </GlassButton>
          </GlassPanel>
        )}

        {!analyzing && recent.length > 0 && (
          <div className="portal-screen__recent" aria-label={ptBR.portal.recent}>
            <span className="portal-screen__recent-label">{ptBR.portal.recent}</span>
            <div className="portal-screen__chips">
              {recent.map((r) => (
                <GlassChip key={r.path} title={r.path} meta={r.path} onClick={() => void openFile(r.path)}>
                  {r.name}
                </GlassChip>
              ))}
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

function InfoIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <path d="M8 7.3v4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="8" cy="4.8" r="0.9" fill="currentColor" />
    </svg>
  );
}

function ThemeIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <path d="M8 1.8a6.2 6.2 0 0 1 0 12.4z" fill="currentColor" />
    </svg>
  );
}
