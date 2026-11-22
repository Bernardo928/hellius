/**
 * Mede o FPS nos primeiros segundos. Abaixo de 50 FPS desliga o blur dos
 * contêineres não essenciais (data-perf="low"), conforme a especificação.
 */
export function startFpsGuard(seconds = 2, threshold = 50): () => void {
  let frames = 0;
  let start = 0;
  let raf = 0;
  let stopped = false;
  const tick = (now: number) => {
    if (stopped) return;
    if (!start) start = now;
    frames++;
    const elapsed = now - start;
    if (elapsed >= seconds * 1000) {
      const fps = (frames * 1000) / elapsed;
      if (fps < threshold) document.documentElement.dataset.perf = 'low';
      stopped = true;
      return;
    }
    raf = requestAnimationFrame(tick);
  };
  raf = requestAnimationFrame(tick);
  return () => {
    stopped = true;
    cancelAnimationFrame(raf);
  };
}
