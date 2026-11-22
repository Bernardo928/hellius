// Tokenizador leve para realce de sintaxe. Retorna pedaços de TEXTO, nunca HTML.
// A renderização usa React (texto escapado): nada vindo do binário vira HTML.

export type TokenKind =
  | 'mnemonic'
  | 'reg'
  | 'num'
  | 'str'
  | 'target'
  | 'kw'
  | 'sym'
  | 'comment'
  | 'plain';

export interface Token {
  kind: TokenKind;
  text: string;
}

const REGISTERS = new Set([
  'rax', 'rbx', 'rcx', 'rdx', 'rsi', 'rdi', 'rbp', 'rsp', 'rip',
  'eax', 'ebx', 'ecx', 'edx', 'esi', 'edi', 'ebp', 'esp', 'eip',
  'ax', 'bx', 'cx', 'dx', 'si', 'di', 'bp', 'sp',
  'al', 'ah', 'bl', 'bh', 'cl', 'ch', 'dl', 'dh', 'sil', 'dil', 'bpl', 'spl',
  'xmm0', 'xmm1', 'xmm2', 'xmm3', 'xmm4', 'xmm5', 'xmm6', 'xmm7',
  'lr', 'sp', 'pc', 'x0', 'x1', 'x2', 'x3', 'x29', 'x30', 'w0', 'w1', 'w2', 'w3',
]);

const SIZE_KW = new Set(['byte', 'word', 'dword', 'qword', 'ptr', 'short', 'near', 'far', 'offset']);

const C_KW = new Set([
  'int', 'void', 'char', 'unsigned', 'return', 'if', 'else', 'for', 'while', 'do', 'switch',
  'case', 'break', 'continue', 'struct', 'const', 'static', 'sizeof', 'uint8_t', 'uint32_t',
  'uint64_t', 'int32_t', 'int64_t',
]);

const TOKEN_RE = /"(?:[^"\\]|\\.)*"|0x[0-9a-fA-F]+|\b\d+\b|[A-Za-z_.$][A-Za-z0-9_.$]*|\s+|[\s\S]/g;

/** Tokeniza uma instrução de assembly (texto sem o comentário). */
export function tokenizeAsm(text: string): Token[] {
  const out: Token[] = [];
  const parts = text.match(TOKEN_RE) ?? [];
  let seenMnemonic = false;
  for (const part of parts) {
    const lower = part.toLowerCase();
    if (/^\s+$/.test(part) || part.length === 0) {
      out.push({ kind: 'plain', text: part });
    } else if (part.startsWith('"')) {
      out.push({ kind: 'str', text: part });
    } else if (/^0x[0-9a-fA-F]+$/.test(part) || /^\d+$/.test(part)) {
      out.push({ kind: 'num', text: part });
    } else if (!seenMnemonic && /^[A-Za-z]/.test(part)) {
      seenMnemonic = true;
      out.push({ kind: 'mnemonic', text: part });
    } else if (REGISTERS.has(lower)) {
      out.push({ kind: 'reg', text: part });
    } else if (SIZE_KW.has(lower)) {
      out.push({ kind: 'kw', text: part });
    } else if (/^[A-Za-z_.$]/.test(part)) {
      out.push({ kind: 'sym', text: part });
    } else {
      out.push({ kind: 'plain', text: part });
    }
  }
  // Destino de salto/chamada: primeiro número depois de call/jmp/jxx.
  const first = out.find((t) => t.kind === 'mnemonic');
  if (first && /^(call|jmp|j[a-z]+|loop)$/i.test(first.text)) {
    const target = out.find((t) => t.kind === 'num' || t.kind === 'sym' && /^0x/.test(t.text));
    if (target) target.kind = 'target';
  }
  return out;
}

/** Tokeniza uma linha de pseudocódigo C. */
export function tokenizeC(text: string): Token[] {
  const out: Token[] = [];
  const parts = text.match(TOKEN_RE) ?? [];
  for (const part of parts) {
    if (/^\s+$/.test(part)) out.push({ kind: 'plain', text: part });
    else if (part.startsWith('"')) out.push({ kind: 'str', text: part });
    else if (/^(0x[0-9a-fA-F]+|\d+)$/.test(part)) out.push({ kind: 'num', text: part });
    else if (C_KW.has(part)) out.push({ kind: 'kw', text: part });
    else if (/^[A-Za-z_]/.test(part)) out.push({ kind: 'sym', text: part });
    else out.push({ kind: 'plain', text: part });
  }
  return out;
}

/** Tokens de hex: o texto ASCII ao lado dos bytes não é destacado. */
export function tokenizePlain(text: string): Token[] {
  return [{ kind: 'plain', text }];
}
