import { beforeEach, describe, expect, it } from 'vitest';
import { useAppStore, displayName } from './appStore';

const fn = (address: string, name: string) => ({
  address,
  name,
  sizeBytes: 10,
  blockCount: 1,
  xrefsIn: [],
  xrefsOut: [],
});

describe('appStore', () => {
  beforeEach(() => {
    useAppStore.setState({
      selectedFunction: null,
      history: [],
      historyIndex: -1,
      selectAllActive: false,
      renames: {},
      functions: [fn('0x1', 'a'), fn('0x2', 'b')],
    });
  });

  it('histórico volta e avança', () => {
    const s = useAppStore.getState();
    s.selectFunction('0x1');
    s.selectFunction('0x2');
    expect(useAppStore.getState().history).toEqual(['0x1', '0x2']);
    expect(useAppStore.getState().goBack()).toBe('0x1');
    expect(useAppStore.getState().selectedFunction).toBe('0x1');
    expect(useAppStore.getState().goForward()).toBe('0x2');
  });

  it('nova seleção depois de voltar apaga o trecho à frente', () => {
    useAppStore.getState().selectFunction('0x1');
    useAppStore.getState().selectFunction('0x2');
    useAppStore.getState().goBack();
    useAppStore.getState().selectFunction('0x2');
    expect(useAppStore.getState().history).toEqual(['0x1', '0x2']);
    expect(useAppStore.getState().historyIndex).toBe(1);
  });

  it('renomear é só em memória e limpa com nome vazio', () => {
    const s = useAppStore.getState();
    s.renameFunction('0x1', 'novo_nome');
    const f = useAppStore.getState().functions[0];
    expect(f && displayName(f, useAppStore.getState().renames)).toBe('novo_nome');
    useAppStore.getState().renameFunction('0x1', '   ');
    expect(useAppStore.getState().renames['0x1']).toBeUndefined();
  });

  it('selecionar tudo é zerado ao trocar de função', () => {
    useAppStore.getState().setSelectAll(true);
    useAppStore.getState().selectFunction('0x2');
    expect(useAppStore.getState().selectAllActive).toBe(false);
  });
});
