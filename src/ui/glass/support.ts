/**
 * Detecta se a refração SVG (backdrop-filter: url(#...)) deve ser usada.
 *
 * CSS.supports só confirma que a sintaxe é aceita. Para a refração valer de fato,
 * exigimos também um WebView baseado em Chromium: WebView2 (Windows) identifica-se
 * com "Edg/" no User-Agent. WebKitGTK (Linux) não entra aqui e recebe o fallback
 * sem refração, que mantém a aparência de vidro.
 */
export function refractionSupported(): boolean {
  try {
    if (typeof CSS === 'undefined' || !CSS.supports('backdrop-filter', 'blur(2px)')) return false;
    return /Edg\//.test(navigator.userAgent);
  } catch {
    return false;
  }
}

export function applyRefractionFlag(): boolean {
  const on = refractionSupported();
  document.documentElement.dataset.refract = on ? 'on' : 'off';
  return on;
}
