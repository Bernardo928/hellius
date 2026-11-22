// Página de demonstração dos componentes de vidro, acessível em #/dev/glass.
import { GlassButton, GlassChip, GlassInput, GlassPanel, GlassTabs, GlassToast, GlassToggle, GlassTooltip } from '../ui/glass';
import { useState } from 'react';

export function GlassDemo() {
  const [tab, setTab] = useState<'a' | 'b' | 'c'>('a');
  const [on, setOn] = useState(true);
  return (
    <div style={{ padding: 32, display: 'grid', gap: 20, maxWidth: 720 }}>
      <h1 style={{ fontWeight: 300 }}>Glass — demonstração</h1>
      <GlassPanel style={{ display: 'grid', gap: 12, padding: 16 }}>
        <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
          <GlassButton variant="primary">Primário</GlassButton>
          <GlassButton variant="secondary">Secundário</GlassButton>
          <GlassButton variant="ghost">Fantasma</GlassButton>
          <GlassButton variant="danger">Perigo</GlassButton>
          <GlassButton disabled>Desabilitado</GlassButton>
          <GlassTooltip label="Dica de ferramenta">
            <GlassButton size="sm">Pequeno</GlassButton>
          </GlassTooltip>
          <GlassButton size="lg" iconOnly aria-label="Favorito" icon="★" />
        </div>
        <GlassInput placeholder="Campo de texto" />
        <GlassTabs
          label="Exemplo"
          tabs={[
            { id: 'a' as const, label: 'Uma' },
            { id: 'b' as const, label: 'Duas' },
            { id: 'c' as const, label: 'Três' },
          ]}
          value={tab}
          onChange={setTab}
        />
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <GlassToggle checked={on} onChange={setOn} label="Alternar" />
          <GlassChip meta="0x140001000">main</GlassChip>
        </div>
        <GlassToast tone="success">Toast de sucesso</GlassToast>
        <GlassToast tone="error">Toast de erro</GlassToast>
      </GlassPanel>
    </div>
  );
}
