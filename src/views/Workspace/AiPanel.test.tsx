import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const h = vi.hoisted(() => ({
  status: { configured: false, source: null as 'app' | 'env' | null, model: 'claude-opus-5-5', consent: false },
  deltaCb: null as null | ((d: { requestId: number; text: string }) => void),
  explain: vi.fn(),
  save: vi.fn(),
}));

vi.mock('../../api/hellius', () => ({
  api: {
    aiStatus: vi.fn(async () => h.status),
    aiSaveSettings: (p: unknown) => h.save(p),
    aiExplain: (...a: unknown[]) => h.explain(...a),
    aiCancel: vi.fn(async () => null),
  },
  onAiDelta: vi.fn(async (cb: (d: { requestId: number; text: string }) => void) => {
    h.deltaCb = cb;
    return () => {};
  }),
  onAnalysisProgress: vi.fn(),
}));

import { useAppStore } from '../../store/appStore';
import { AiPanel } from './AiPanel';

const fn = { address: '0x401000', name: 'main', sizeBytes: 10, blockCount: 1, xrefsIn: [], xrefsOut: [] };

describe('AiPanel', () => {
  beforeEach(() => {
    h.explain.mockReset();
    h.save.mockReset();
    useAppStore.setState({
      functions: [fn],
      selectedFunction: fn.address,
      renames: {},
      toasts: [],
      ai: { status: null, requestId: 0, running: false, text: '', error: null, functionAddress: null },
    });
  });

  it('sem chave: pede a chave e salva sem mostrar de volta', async () => {
    h.status = { configured: false, source: null, model: 'claude-opus-5-5', consent: false };
    h.save.mockResolvedValue({ ...h.status, configured: true, source: 'app' });
    render(<AiPanel />);
    const input = await screen.findByLabelText('Chave da API');
    expect(input).toHaveAttribute('type', 'password');
    fireEvent.change(input, { target: { value: 'sk-ant-teste' } });
    fireEvent.click(screen.getByRole('button', { name: 'Salvar chave' }));
    await waitFor(() => expect(h.save).toHaveBeenCalledWith({ apiKey: 'sk-ant-teste' }));
  });

  it('com chave e sem consentimento: explica o que será enviado antes', async () => {
    h.status = { configured: true, source: 'env', model: 'claude-opus-5-5', consent: false };
    render(<AiPanel />);
    expect(await screen.findByText(/envia para a API da Anthropic/)).toBeInTheDocument();
    expect(h.explain).not.toHaveBeenCalled();
  });

  it('com consentimento: mostra o texto chegando em streaming, como texto', async () => {
    h.status = { configured: true, source: 'app', model: 'claude-opus-5-5', consent: true };
    let resolve: (v: string) => void = () => {};
    h.explain.mockReturnValue(new Promise<string>((r) => (resolve = r)));
    render(<AiPanel />);
    fireEvent.click(await screen.findByRole('button', { name: 'Explicar esta função' }));
    await waitFor(() => expect(h.explain).toHaveBeenCalledWith('0x401000', 1));
    act(() => h.deltaCb?.({ requestId: 1, text: '<b>Resumo</b>: chama puts.' }));
    expect(await screen.findByText('<b>Resumo</b>: chama puts.')).toBeInTheDocument();
    await act(async () => resolve('<b>Resumo</b>: chama puts.'));
    expect(screen.getByRole('button', { name: 'Explicar de novo' })).toBeInTheDocument();
  });

  it('erro da API aparece legível', async () => {
    h.status = { configured: true, source: 'app', model: 'claude-opus-5-5', consent: true };
    h.explain.mockRejectedValue('A chave de API foi recusada (401). Confira se ela está correta e ativa.');
    render(<AiPanel />);
    fireEvent.click(await screen.findByRole('button', { name: 'Explicar esta função' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('401');
  });
});
