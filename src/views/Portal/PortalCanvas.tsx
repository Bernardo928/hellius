import { useEffect, useRef } from 'react';

interface Props {
  /** Multiplicador de velocidade: 1 normal, maior quando há arquivo sobre a janela. */
  speed: number;
  size: number;
}

const PARTICLES = 300;

interface P {
  angle: number;
  radius: number; // 0..1 (1 = borda)
  speed: number;
  hue: number;
}

function spawn(): P {
  return {
    angle: Math.random() * Math.PI * 2,
    radius: 0.8 + Math.random() * 0.2,
    speed: 0.004 + Math.random() * 0.006,
    hue: 200 + Math.random() * 90,
  };
}

/**
 * Vórtice em espiral desenhado em <canvas> 2D. Pausa quando a janela perde o foco
 * ou fica oculta; com prefers-reduced-motion desenha um único quadro estático.
 */
export function PortalCanvas({ speed, size }: Props) {
  const ref = useRef<HTMLCanvasElement>(null);
  const speedRef = useRef(speed);
  speedRef.current = speed;

  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    canvas.width = size * dpr;
    canvas.height = size * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    const particles: P[] = Array.from({ length: PARTICLES }, spawn);
    const c = size / 2;
    const R = size / 2 - 4;
    let raf = 0;
    let running = false;

    const draw = () => {
      ctx.clearRect(0, 0, size, size);
      for (const p of particles) {
        if (!reduced) {
          p.angle += p.speed * speedRef.current * (1 + (1 - p.radius) * 4);
          p.radius -= 0.0016 * speedRef.current;
          if (p.radius < 0.06) Object.assign(p, spawn(), { radius: 0.9 + Math.random() * 0.1 });
        }
        const r = p.radius * R;
        const x = c + Math.cos(p.angle) * r;
        const y = c + Math.sin(p.angle) * r;
        const alpha = Math.max(0.05, 1 - p.radius) * 0.9;
        ctx.fillStyle = `hsla(${p.hue}, 90%, 72%, ${alpha})`;
        ctx.beginPath();
        ctx.arc(x, y, 1.1 + (1 - p.radius) * 0.9, 0, Math.PI * 2);
        ctx.fill();
      }
      // Miolo luminoso
      const g = ctx.createRadialGradient(c, c, 0, c, c, R * 0.35);
      g.addColorStop(0, 'rgba(120,160,255,0.25)');
      g.addColorStop(1, 'rgba(120,160,255,0)');
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(c, c, R * 0.35, 0, Math.PI * 2);
      ctx.fill();
    };

    const loop = () => {
      if (!running) return;
      draw();
      raf = requestAnimationFrame(loop);
    };

    const start = () => {
      if (running || reduced) return;
      running = true;
      raf = requestAnimationFrame(loop);
    };
    const stop = () => {
      running = false;
      cancelAnimationFrame(raf);
    };
    const onVis = () => (document.hidden || !document.hasFocus() ? stop() : start());

    draw();
    if (!reduced && document.hasFocus() && !document.hidden) start();
    window.addEventListener('focus', start);
    window.addEventListener('blur', stop);
    document.addEventListener('visibilitychange', onVis);
    return () => {
      stop();
      window.removeEventListener('focus', start);
      window.removeEventListener('blur', stop);
      document.removeEventListener('visibilitychange', onVis);
    };
  }, [size]);

  return <canvas ref={ref} className="portal__vortex" style={{ width: size, height: size }} aria-hidden="true" />;
}
