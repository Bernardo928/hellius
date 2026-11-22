import { useCallback, useEffect } from 'react';
import { GlassDefs, applyRefractionFlag } from './ui/glass';
import { useAppStore } from './store/appStore';
import { applyTheme, THEME_IDS, type ThemeId } from './theme/themes';
import { startFpsGuard } from './lib/fps';
import { Portal } from './views/Portal/Portal';
import { Workspace } from './views/Workspace/Workspace';
import { Toasts } from './components/Toasts';
import { AboutModal } from './components/AboutModal';
import { GlassDemo } from './dev/GlassDemo';
import { ptBR } from './i18n/pt-BR';

export function App() {
  const screen = useAppStore((s) => s.screen);
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);
  const aboutOpen = useAppStore((s) => s.aboutOpen);
  const setAbout = useAppStore((s) => s.setAbout);

  useEffect(() => {
    applyRefractionFlag();
    const stop = startFpsGuard();
    return stop;
  }, []);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const cycleTheme = useCallback(() => {
    const i = THEME_IDS.indexOf(theme);
    setTheme((THEME_IDS[(i + 1) % THEME_IDS.length] ?? 'black') as ThemeId);
  }, [theme, setTheme]);

  // Página de desenvolvimento dos componentes de vidro.
  if (window.location.hash === '#/dev/glass') {
    return (
      <div className="app-root">
        <GlassDefs />
        <GlassDemo />
      </div>
    );
  }

  return (
    <div className="app-root">
      <GlassDefs />
      <div className="ambient" aria-hidden="true" />
      {screen === 'workspace' ? (
        <Workspace />
      ) : (
        <Portal onOpenInfo={() => setAbout(true)} onOpenTheme={cycleTheme} />
      )}
      <Toasts />
      <AboutModal open={aboutOpen} onClose={() => setAbout(false)} />
      <span className="sr-only">{ptBR.app.name}</span>
    </div>
  );
}
