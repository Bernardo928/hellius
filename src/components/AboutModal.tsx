import { Modal } from './Modal';
import { ptBR } from '../i18n/pt-BR';

export function AboutModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Modal open={open} title={ptBR.about.title} onClose={onClose}>
      <p>
        {ptBR.app.name} · {ptBR.app.version}
      </p>
      <p className="inspector__muted">
        Análise de binários: disassembly real (x86, x86-64, ARM, AArch64, MIPS) com capstone, funções, imports,
        strings e referências cruzadas. Em x86 e x86-64 o pseudocódigo é decompilado (chamadas com argumentos,
        variáveis, if/while/for/switch, constantes e structs Win32); nas demais arquiteturas é tradução instrução a
        instrução.
      </p>
      <p className="inspector__muted">
        O arquivo analisado nunca é executado. A única chamada de rede é a explicação com IA, quando você pede: ela envia o
        código de uma função para a API da Anthropic.
      </p>
    </Modal>
  );
}
