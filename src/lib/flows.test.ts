import { beforeEach, describe, expect, it, vi } from 'vitest';

const openFileMock = vi.fn();
vi.mock('../api/hellius', () => ({
  api: { openFile: (...a: unknown[]) => openFileMock(...a) },
  onAnalysisProgress: vi.fn(async () => () => {}),
}));

import { useAppStore } from '../store/appStore';
import { openFile } from './analysis';
import { goToInput } from './navigation';

const fn = (address: string, name: string, sizeBytes: number) => ({
  address,
  name,
  sizeBytes,
  blockCount: 1,
  xrefsIn: [],
  xrefsOut: [],
});

describe('abertura de arquivo', () => {
  beforeEach(() => openFileMock.mockReset());

  it('ignora um segundo arquivo enquanto a análise roda', async () => {
    useAppStore.setState({ screen: 'analyzing' });
    await openFile('/tmp/b.dll');
    expect(openFileMock).not.toHaveBeenCalled();
  });

  it('erro na análise volta ao portal com um aviso', async () => {
    useAppStore.setState({ screen: 'portal', toasts: [] });
    openFileMock.mockRejectedValueOnce('O arquivo está vazio.');
    await openFile('/tmp/vazio.bin');
    expect(useAppStore.getState().screen).toBe('portal');
    const toasts = useAppStore.getState().toasts;
    expect(toasts[toasts.length - 1]?.message).toContain('O arquivo está vazio.');
  });
});

describe('navegação por endereço', () => {
  beforeEach(() => {
    useAppStore.setState({
      functions: [fn('0x1000', 'a', 0x40), fn('0x2000', 'b', 0x100)],
      selectedFunction: '0x1000',
      history: ['0x1000'],
      historyIndex: 0,
      jumpRequest: null,
      renames: {},
    });
  });

  it('endereço no meio de outra função seleciona essa função e pede o salto', () => {
    expect(goToInput('0x2010')).toBe(true);
    expect(useAppStore.getState().selectedFunction).toBe('0x2000');
    expect(useAppStore.getState().jumpRequest?.address).toBe('0x2010');
  });

  it('nome parcial encontra a função', () => {
    expect(goToInput('b')).toBe(true);
    expect(useAppStore.getState().selectedFunction).toBe('0x2000');
  });
});
