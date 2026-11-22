// Ir para endereço ou nome: usado pela barra superior e pelo Ctrl+G.
import { useAppStore } from '../store/appStore';
import { ptBR } from '../i18n/pt-BR';

export function parseHex(text: string): number | null {
  const t = text.trim().replace(/^0x/i, '');
  if (!/^[0-9a-fA-F]{1,16}$/.test(t)) return null;
  const n = parseInt(t, 16);
  return Number.isSafeInteger(n) ? n : null;
}

const toNum = (addr: string) => parseHex(addr) ?? -1;

/** Vai para uma função pelo endereço, ou para um endereço dentro de uma função. */
export function goToInput(input: string): boolean {
  const s = useAppStore.getState();
  const text = input.trim();
  if (!text) return false;

  const n = parseHex(text);
  if (n !== null) {
    const exact = s.functions.find((f) => toNum(f.address) === n);
    if (exact) {
      s.selectFunction(exact.address);
      return true;
    }
    const inside = s.functions.find((f) => n >= toNum(f.address) && n < toNum(f.address) + f.sizeBytes);
    if (inside) {
      s.selectFunction(inside.address);
      s.requestJump('0x' + n.toString(16));
      return true;
    }
  }

  const lower = text.toLowerCase();
  const byName = s.functions.find(
    (f) => (s.renames[f.address] ?? f.name).toLowerCase().includes(lower),
  );
  if (byName) {
    s.selectFunction(byName.address);
    return true;
  }

  s.pushToast({ tone: 'info', message: `Nada encontrado para "${text}". ${ptBR.lists.empty}` });
  return false;
}
