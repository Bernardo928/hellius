import { render, screen, fireEvent } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { GlassButton, GlassTabs, GlassToggle, GlassTooltip } from './Glass';

describe('design system Liquid Glass', () => {
  it('todo botão recebe a classe de vidro', () => {
    render(<GlassButton variant="danger">Apagar</GlassButton>);
    const btn = screen.getByRole('button', { name: 'Apagar' });
    expect(btn.className).toContain('glass-button');
    expect(btn.className).toContain('glass-button--danger');
  });

  it('botão só com ícone exige aria-label e mantém o nome acessível', () => {
    render(<GlassButton iconOnly aria-label="Fechar" icon="×" />);
    expect(screen.getByRole('button', { name: 'Fechar' })).toBeInTheDocument();
  });

  it('abas trocam com as setas do teclado', () => {
    const onChange = vi.fn();
    render(
      <GlassTabs
        label="teste"
        tabs={[
          { id: 'a', label: 'A' },
          { id: 'b', label: 'B' },
        ]}
        value="a"
        onChange={onChange}
      />,
    );
    fireEvent.keyDown(screen.getByRole('tablist'), { key: 'ArrowRight' });
    expect(onChange).toHaveBeenCalledWith('b');
  });

  it('toggle expõe o estado como switch', () => {
    const onChange = vi.fn();
    render(<GlassToggle checked={false} onChange={onChange} label="Minimapa" />);
    const sw = screen.getByRole('switch', { name: 'Minimapa' });
    expect(sw).toHaveAttribute('aria-checked', 'false');
    fireEvent.click(sw);
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('botão padrão é plain: sem classe de variante (sem moldura dentro do vidro)', () => {
    render(<GlassButton>Copiar</GlassButton>);
    expect(screen.getByRole('button', { name: 'Copiar' }).className).toBe('glass-button');
  });

  it('dica fica no próprio botão (aria-describedby)', () => {
    render(
      <GlassTooltip label="Disponível numa fase futura">
        <GlassButton aria-disabled="true">Explicar</GlassButton>
      </GlassTooltip>,
    );
    const btn = screen.getByRole('button', { name: 'Explicar' });
    expect(btn).toHaveAccessibleDescription('Disponível numa fase futura');
  });

  it('aria-disabled ignora o clique mas continua focável', () => {
    const onClick = vi.fn();
    render(
      <GlassButton aria-disabled="true" onClick={onClick}>
        Explicar
      </GlassButton>,
    );
    const btn = screen.getByRole('button', { name: 'Explicar' });
    fireEvent.click(btn);
    expect(onClick).not.toHaveBeenCalled();
    btn.focus();
    expect(document.activeElement).toBe(btn);
  });
});
