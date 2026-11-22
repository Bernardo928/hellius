# Hellius

Ferramenta de engenharia reversa "tipo Ghidra, só que mais fácil". Esta fase entrega **somente a GUI desktop**
(Tauri v2 + React + TypeScript). A análise ainda usa um `MockEngine`: os metadados do arquivo são reais, mas
funções e código são de demonstração e a interface avisa isso o tempo todo.

Não executa o arquivo analisado, não faz chamadas de rede e funciona offline (fontes empacotadas).

## Como rodar

Pré-requisitos (Linux): `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libxdo-dev build-essential pkg-config libssl-dev`.
Windows: WebView2 (já vem no Windows 11). Node 22+, Rust estável.

```bash
npm install
npm run tauri dev          # desenvolvimento
```

## Como compilar

```bash
npm run build                          # typecheck + build do front
npx tauri build --debug --no-bundle    # binário de depuração, sem instalador
npx tauri build                        # instaladores (exige as ferramentas de bundle da plataforma)
```

## Testes e verificação

```bash
npm run typecheck     # tsc
npm run lint          # eslint (zero warnings)
npm test              # vitest (front)
cd src-tauri && cargo test     # testes Rust
```

Testes cobrem: tokenizador de assembly e C, store (histórico, renomear, seleção), tema persistido, design
system de vidro, virtualização de uma função de 20.000 linhas, "Selecionar tudo" + cópia completa, Esc, e
validação de arquivo (caminho inexistente, pasta, vazio, hash SHA-256, metadados de ELF real).

## Estrutura

```
src-tauri/src/      lib.rs (builder), commands.rs (comandos Tauri), analyze.rs (validação e metadados),
                    engine/mod.rs (trait e tipos), engine/mock.rs (MockEngine), error.rs
src/api/            cliente tipado dos comandos Tauri e tipos espelhados
src/store/          Zustand: tela, tema, arquivo, histórico, seleção
src/ui/glass/       design system Liquid Glass (GlassButton, GlassPanel, GlassInput, GlassTabs, GlassToggle, GlassChip, GlassToast, GlassTooltip)
src/views/Portal/   tela inicial: portal em canvas, arrastar e soltar, recentes
src/views/Workspace/ painel de análise: listas, visualizador, inspetor, barra de status
src/lib/tokenizer/  realce de sintaxe (asm e C), sem HTML
src/theme/          temas (preto, grafite, claro, alto contraste) por variáveis CSS
src/i18n/pt-BR.ts   todos os textos da interface
docs/ENGINE.md      contrato do engine
```

## Decisões tomadas

- **Stack**: Tauri v2, React 18, TypeScript estrito, Vite, Zustand, `@tanstack/react-virtual`, Vitest. CSS puro, sem UI pronta.
- **Segurança**: CSP restritiva em `tauri.conf.json`. Permissões mínimas em `capabilities/default.json` (`dialog:allow-open`, `core:default`). O front recebe só metadados e fatias de código. Texto do binário sempre vai como texto (React escapa), nunca `dangerouslySetInnerHTML`.
- **Código sob demanda**: o front busca fatias de 400 linhas. Busca, salto e "programa inteiro" carregam o que precisam. O texto completo de cópia vem do Rust.
- **Drag-and-drop** usa `getCurrentWebview().onDragDropEvent`, porque o `drop` do HTML5 não traz o caminho no Tauri.
- **Liquid Glass**: vidro = fundo translúcido + borda com cor por lado + highlight interno + brilho que segue o ponteiro (`--mx`/`--my`). Blur (`backdrop-filter`) só em contêineres flutuantes, não em botões nem em listas.
- **Refração SVG** (`feDisplacementMap`) só é ativada onde há WebView2 (`Edg/` no User-Agent). WebKitGTK usa o fallback sem refração.
- **Fundo ambiente**: três manchas radiais com opacidade máxima 0.25 atrás de tudo, para o vidro aparecer sobre preto.
- **Tema padrão**: preto. Persistido em `localStorage`, com `try/catch`.
- **Layout das colunas** (larguras, recolhidas) é salvo em `localStorage`.
- **Renomear função** é só em memória, nesta sessão.
- **Ctrl+C e "Copiar"** copiam sempre o escopo completo, por pedido da especificação. Isso significa que copiar com um trecho selecionado também copia a função inteira.
- **Desempenho**: se o FPS medido nos 2 primeiros segundos ficar abaixo de 50, `data-perf="low"` desliga o blur dos contêineres não essenciais.

## Limitações conhecidas

- **Engine de demonstração.** Funções, código, strings e imports são inventados. Só os metadados do arquivo são reais.
- **Arquivo lido inteiro na memória** (até 512 MB), para o hash e os cabeçalhos. Um engine real deve usar leitura parcial ou mapeamento de memória.
- **Entrada do PE**: o endereço usa `image_base + entry`. Não testado com um PE real, porque não havia PE no ambiente de verificação.
- **Refração SVG** não foi verificada num WebView2 real.
- **Drag-and-drop de arquivo** não foi testado (sem automação de arrasto no ambiente).
- **Ctrl+A/Ctrl+C e cópia** foram verificados no app só no escopo "Função". O escopo "Programa inteiro" não tem teste automatizado nem verificação na interface.
- **Rolagem suave** na função de 20.000 linhas não foi medida; só a contagem de linhas copiadas foi verificada.
- **Aviso de demonstração** aparece na barra de status e no visualizador; a janela "Sobre" também explica.
- **Minimapa, paleta Ctrl+K, exportação e anotações** (P2) não foram implementados.
- **Linhas de cabeçalho** de função no modo "programa inteiro" não recebem o realce de seleção ("Selecionar tudo").
- **Macs**: não são alvo desta fase.

## Verificação feita nesta fase

- `typecheck`, `lint`, `vitest` (20 testes) e `cargo test` (9 testes) passam.
- `npx tauri build --debug --no-bundle` compila o app.
- O app abriu no Xvfb (WebKitGTK), mostrou o portal, abriu `/bin/ls` copiado (via seletor de arquivo nativo), mostrou metadados reais (ELF, x86_64, 64 bits, SHA-256), funções de demonstração e o aviso de demonstração.
- "Selecionar tudo" + Ctrl+C na função de 20.000 linhas copiou 20.000 linhas completas para a área de transferência.
- Não verificados: FPS real, `prefers-reduced-motion` e `prefers-reduced-transparency` no sistema (só no CSS), temas graphite e alto contraste no app (apenas a página de componentes, no Chromium), Windows.

## Achado durante a verificação

No WebKitGTK, `backdrop-filter` num botão com fundo próprio fez o fundo sumir. O blur ficou nos contêineres e os botões ficaram translúcidos, sem blur próprio. Por isso `GlassButton` só aplica blur quando `floating` é explicitamente pedido.
