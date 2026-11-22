import { act, fireEvent, render, waitFor } from '@testing-library/react';
import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAppStore } from '../../store/appStore';
import { CodeViewer } from './CodeViewer';

// Função sintética de 20.000 linhas, servida em fatias pelo "Rust" simulado.
const N = 20000;
const LINES = Array.from({ length: N }, (_, i) => ({
  address: '0x' + (0x140010000 + i).toString(16),
  bytes: '90',
  text: `nop ; ${i}`,
}));

vi.mock('../../api/hellius', () => {
  const lines = Array.from({ length: 20000 }, (_, i) => ({
    address: '0x' + (0x140010000 + i).toString(16),
    bytes: '90',
    text: `nop ; ${i}`,
  }));
  return {
    api: {
      getCode: vi.fn(async (fn: string, kind: string, start: number, count: number) => ({
        functionAddress: fn,
        kind,
        totalLines: lines.length,
        startLine: start,
        lines: lines.slice(start, start + count),
      })),
      getFullText: vi.fn(async () =>
        lines.map((l) => `${l.address}  ${l.bytes}  ${l.text}`).join('\n'),
      ),
    },
    onAnalysisProgress: vi.fn(),
  };
});

beforeAll(() => {
  // jsdom não faz layout: damos um tamanho fixo para o virtualizador.
  Element.prototype.getBoundingClientRect = function () {
    return { x: 0, y: 0, top: 0, left: 0, bottom: 600, right: 800, width: 800, height: 600, toJSON() { return {}; } } as DOMRect;
  };
  // O virtualizador mede clientHeight/clientWidth.
  Object.defineProperty(HTMLElement.prototype, 'clientHeight', { configurable: true, get: () => 600 });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get: () => 800 });
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn(async () => {}) },
  });
});

afterAll(() => {
  vi.restoreAllMocks();
});

describe('CodeViewer: selecionar tudo e copiar', () => {
  beforeEach(() => {
    useAppStore.setState({
      screen: 'workspace',
      scope: 'function',
      codeKind: 'disasm',
      selectedFunction: '0x140010000',
      selectAllActive: false,
      functions: [
        { address: '0x140010000', name: 'grande', sizeBytes: 20000, blockCount: 1, xrefsIn: [], xrefsOut: [] },
      ],
      engine: { name: 'Demonstração (MockEngine)', isDemo: true },
      toasts: [],
    });
    void LINES;
  });

  it('virtualiza: renderiza só uma janela das 20.000 linhas', async () => {
    const { container } = render(<CodeViewer />);
    await waitFor(() => {
      expect(container.querySelectorAll('.code__row').length).toBeGreaterThan(0);
    });
    expect(container.querySelectorAll('.code__row').length).toBeLessThan(200);
  });

  it('Ctrl+A marca as linhas renderizadas como selecionadas e a barra conta o escopo', async () => {
    const { container } = render(<CodeViewer />);
    await waitFor(() => expect(container.querySelectorAll('.code__row').length).toBeGreaterThan(0));
    const region = container.querySelector('.code__scroll') as HTMLElement;
    act(() => {
      fireEvent.keyDown(region, { key: 'a', ctrlKey: true });
    });
    expect(useAppStore.getState().selectAllActive).toBe(true);
    expect(container.querySelectorAll('.code__row.is-selected').length).toBeGreaterThan(0);
  });

  it('copiar devolve o escopo inteiro, não só as linhas visíveis', async () => {
    const { container } = render(<CodeViewer />);
    await waitFor(() => expect(container.querySelectorAll('.code__row').length).toBeGreaterThan(0));
    const region = container.querySelector('.code__scroll') as HTMLElement;
    act(() => {
      fireEvent.keyDown(region, { key: 'a', ctrlKey: true });
    });
    fireEvent.copy(region);
    await waitFor(() => expect(navigator.clipboard.writeText).toHaveBeenCalled());
    const calls = (navigator.clipboard.writeText as unknown as ReturnType<typeof vi.fn>).mock.calls;
    const written = String(calls[calls.length - 1]?.[0]);
    expect(written.split('\n')).toHaveLength(N);
  });

  it('Esc desfaz a seleção', async () => {
    const { container } = render(<CodeViewer />);
    await waitFor(() => expect(container.querySelectorAll('.code__row').length).toBeGreaterThan(0));
    const region = container.querySelector('.code__scroll') as HTMLElement;
    act(() => {
      fireEvent.keyDown(region, { key: 'a', ctrlKey: true });
    });
    act(() => {
      fireEvent.keyDown(region, { key: 'Escape' });
    });
    expect(useAppStore.getState().selectAllActive).toBe(false);
    expect(container.querySelectorAll('.code__row.is-selected').length).toBe(0);
  });
});
