import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { Modal } from './Modal';

describe('Modal', () => {
  it('foca o campo de texto, não o botão de fechar', () => {
    render(
      <Modal open title="Ir para endereço" onClose={vi.fn()}>
        <input aria-label="Endereço" />
      </Modal>,
    );
    expect(document.activeElement).toBe(screen.getByLabelText('Endereço'));
  });

  it('não rouba o foco de novo quando o pai re-renderiza com outro onClose', () => {
    const { rerender } = render(
      <Modal open title="X" onClose={() => {}}>
        <input aria-label="A" />
        <input aria-label="B" />
      </Modal>,
    );
    screen.getByLabelText('B').focus();
    rerender(
      <Modal open title="X" onClose={() => {}}>
        <input aria-label="A" />
        <input aria-label="B" />
      </Modal>,
    );
    expect(document.activeElement).toBe(screen.getByLabelText('B'));
  });
});
