import { beforeEach, describe, expect, it } from 'vitest';
import { applyTheme, loadTheme } from './themes';

describe('temas', () => {
  beforeEach(() => {
    window.localStorage.clear();
    delete document.documentElement.dataset.theme;
  });

  it('padrão é preto quando não há escolha salva', () => {
    expect(loadTheme()).toBe('black');
  });

  it('aplica e persiste a escolha', () => {
    applyTheme('graphite');
    expect(document.documentElement.dataset.theme).toBe('graphite');
    expect(loadTheme()).toBe('graphite');
  });

  it('valor inválido salvo cai no padrão', () => {
    window.localStorage.setItem('hellius.theme', 'neon');
    expect(loadTheme()).toBe('black');
  });
});
