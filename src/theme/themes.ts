export const THEME_IDS = ['black', 'graphite', 'light', 'contrast'] as const;
export type ThemeId = (typeof THEME_IDS)[number];

const KEY = 'hellius.theme';
const DEFAULT: ThemeId = 'black';

function isTheme(v: unknown): v is ThemeId {
  return typeof v === 'string' && (THEME_IDS as readonly string[]).includes(v);
}

/** Lê a escolha salva. Qualquer falha de armazenamento cai no padrão (preto). */
export function loadTheme(): ThemeId {
  try {
    const v = window.localStorage.getItem(KEY);
    return isTheme(v) ? v : DEFAULT;
  } catch {
    return DEFAULT;
  }
}

/** Aplica o tema na hora, sem recarregar. */
export function applyTheme(id: ThemeId): void {
  document.documentElement.dataset.theme = id;
  try {
    window.localStorage.setItem(KEY, id);
  } catch {
    /* armazenamento indisponível: a troca continua valendo nesta sessão */
  }
}
