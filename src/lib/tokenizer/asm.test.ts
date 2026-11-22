import { describe, expect, it } from 'vitest';
import { tokenizeAsm, tokenizeC } from './asm';

describe('tokenizeAsm', () => {
  it('marca mnemônico, registrador e número', () => {
    const t = tokenizeAsm('mov eax, 0x10');
    expect(t[0]).toEqual({ kind: 'mnemonic', text: 'mov' });
    expect(t.find((x) => x.text === 'eax')?.kind).toBe('reg');
    expect(t.find((x) => x.text === '0x10')?.kind).toBe('num');
  });

  it('marca o destino de call e jmp como alvo navegável', () => {
    const t = tokenizeAsm('call 0x140001040');
    expect(t.find((x) => x.text === '0x140001040')?.kind).toBe('target');
  });

  it('não confunde texto com HTML: caracteres especiais voltam como texto', () => {
    const t = tokenizeAsm('mov rax, "<script>"');
    const s = t.find((x) => x.kind === 'str');
    expect(s?.text).toBe('"<script>"');
  });

  it('texto vazio não quebra', () => {
    expect(tokenizeAsm('')).toEqual([]);
  });
});

describe('tokenizeC', () => {
  it('reconhece palavras-chave e números', () => {
    const t = tokenizeC('int main(void) { return 0; }');
    expect(t[0]).toEqual({ kind: 'kw', text: 'int' });
    expect(t.find((x) => x.text === '0')?.kind).toBe('num');
  });
});
