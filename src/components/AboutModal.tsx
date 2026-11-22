import { Modal } from './Modal';
import { ptBR } from '../i18n/pt-BR';

export function AboutModal({ open, onClose }: { open: boolean; onClose: () => void }) {
  return (
    <Modal open={open} title={ptBR.about.title} onClose={onClose}>
      <p>
        {ptBR.app.name} · {ptBR.app.version}
      </p>
      <p className="inspector__muted">
        Análise de binários simplificada. Esta versão usa um engine de demonstração: os metadados do arquivo são reais,
        mas as funções e o código exibidos são exemplos.
      </p>
      <p className="inspector__muted">O app não faz chamadas de rede e nunca executa o arquivo analisado.</p>
    </Modal>
  );
}
