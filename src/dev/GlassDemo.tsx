// Página de demonstração dos componentes, acessível em #/dev/glass.
import { useState } from 'react';
import {
  GlassButton,
  GlassChip,
  GlassInput,
  GlassPanel,
  GlassSearchField,
  GlassTabs,
  GlassToast,
  GlassToggle,
  GlassTooltip,
} from '../ui/glass';
import { IconCopy, IconSettings, IconSidebar } from '../ui/icons';

export function GlassDemo() {
  const [tab, setTab] = useState<'a' | 'b' | 'c'>('a');
  const [on, setOn] = useState(true);
  return (
    <div style={{ padding: 32, display: 'grid', gap: 24, maxWidth: 760 }}>
      <h1 style={{ fontWeight: 300, margin: 0 }}>Componentes</h1>

      <section style={{ display: 'grid', gap: 8 }}>
        <h2 style={{ fontSize: 13, margin: 0 }}>Barra de vidro com botões plain (sem moldura)</h2>
        <GlassPanel style={{ display: 'flex', gap: 4, alignItems: 'center', padding: 8 }}>
          <GlassButton iconOnly aria-label="Barra lateral" icon={<IconSidebar />} pressed />
          <GlassButton>Selecionar tudo</GlassButton>
          <GlassButton variant="primary" icon={<IconCopy />}>
            Copiar
          </GlassButton>
          <GlassTooltip label="Disponível numa fase futura">
            <GlassButton aria-disabled="true">Explicar com IA</GlassButton>
          </GlassTooltip>
          <GlassButton variant="danger">Apagar</GlassButton>
          <GlassButton iconOnly aria-label="Ajustes" icon={<IconSettings />} />
        </GlassPanel>
      </section>

      <section style={{ display: 'grid', gap: 10 }}>
        <h2 style={{ fontSize: 13, margin: 0 }}>Sobre o conteúdo (fora do vidro)</h2>
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', alignItems: 'center' }}>
          <GlassButton variant="primary" size="lg">
            Abrir arquivo
          </GlassButton>
          <GlassButton variant="secondary">Secundário</GlassButton>
          <GlassButton variant="secondary" disabled>
            Desabilitado
          </GlassButton>
          <GlassChip meta="0x140001000">main</GlassChip>
        </div>
        <GlassSearchField placeholder="Buscar" />
        <GlassInput placeholder="Campo de texto" />
        <GlassTabs
          label="Exemplo"
          tabs={[
            { id: 'a' as const, label: 'Disassembly' },
            { id: 'b' as const, label: 'Pseudocódigo' },
            { id: 'c' as const, label: 'Hex' },
          ]}
          value={tab}
          onChange={setTab}
        />
        <GlassToggle checked={on} onChange={setOn} label="Alternar" />
        <GlassToast tone="success">Copiado: 20000 linhas</GlassToast>
        <GlassToast tone="error">Arquivo grande demais (600 MB). O limite é de 512 MB.</GlassToast>
      </section>
    </div>
  );
}
