# Hellius

Ferramenta de engenharia reversa "tipo Ghidra, só que mais fácil". Duas interfaces sobre o mesmo núcleo Rust:
**GUI desktop** (Tauri v2 + React + TypeScript) e **versão de terminal** (`hellius-tui`, ratatui).

A análise é real e roda em Rust, sem Ghidra nem dependências externas:

- **Formatos:** PE (Windows), ELF (Linux), Mach-O (macOS). Outros arquivos abrem como dados (strings e hash).
- **Arquiteturas com disassembly:** x86, x86-64, ARM, Thumb, AArch64, MIPS 32 (capstone).
- **Funções:** encontradas pelo ponto de entrada, símbolos e exports (inclusive a tabela de símbolos COFF do MinGW), chamadas diretas, chamadas de cauda, ponteiros para código e lacunas entre funções (x86). Tabelas de `switch` são seguidas. Em binários glibc, `main` é reconhecida; em executáveis Windows sem símbolos, `WndProc` e `WinMain` são reconhecidas pelo que chamam.
- **Nomes:** imports resolvidos nas chamadas (`strrchr@plt`, `KERNEL32.dll!GetStartupInfoW`) e strings citadas ao lado da instrução.
- **Imports, strings (ASCII e UTF-16), seções e referências cruzadas** reais.
- **Decompilador x86 / x86-64** (próprio, em Rust): chamadas com os argumentos certos (Win64, System V, stdcall/cdecl 32 bits), parâmetros e variáveis locais, `if/else`, `while`, `do/while`, `for`, `switch` (cadeia de comparações e tabela de saltos), strings no lugar dos endereços, nomes de constantes Win32 (`WM_CREATE`, `MB_OK | MB_ICONINFORMATION`, `WS_OVERLAPPEDWINDOW`...) e campos de structs (`wc.lpfnWndProc`, `msg`). Em ARM, AArch64 e MIPS o pseudocódigo continua sendo tradução instrução a instrução.

### O que o decompilador recupera e o que não recupera

O executável não guarda o código-fonte. Guarda instruções. O decompilador reconstrói um C equivalente:

- **Recupera:** a lógica (chamadas, argumentos, condições, laços, switch), strings, constantes com nome quando a função é conhecida, parâmetros e assinaturas de `WinMain`/`WndProc`/`main`.
- **Não recupera:** nomes de variáveis locais (`bar` vira `hMenu`, nome sugerido pelo tipo), comentários, `#define` (`ID_HELLO` vira `1`), macros (`CreateWindowA` aparece como a função real `CreateWindowExA(0, ...)`), tipos exatos de variáveis internas.
- **Otimização muda a forma:** em `-O2` o compilador junta, reordena e desenrola código. O resultado é equivalente, não idêntico (ex.: um `for` pode virar `do/while`, e funções pequenas somem dentro de quem as chama).

Exemplo: o programa Win32 de `crates/hellius-core/tests/fixtures/hello.c`, compilado com MinGW `-O2 -s` (sem símbolos), sai assim:

```c
LRESULT CALLBACK WndProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam)
{
    HMENU hMenu;
    HMENU hMenu2;

    switch (uMsg) {
    case WM_CREATE:
        hMenu = CreateMenu();
        hMenu2 = CreatePopupMenu();
        AppendMenuA(hMenu2, MF_STRING, 1, "Hello World");
        AppendMenuA(hMenu2, MF_SEPARATOR, 0, NULL);
        AppendMenuA(hMenu2, MF_STRING, 2, "Sair");
        AppendMenuA(hMenu, MF_POPUP, hMenu2, "Menu");
        SetMenu(hwnd, hMenu);
        CreateWindowExA(0, "BUTTON", "Hello World", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, 110, 60, 160, 40, hwnd, (HMENU)1, NULL, NULL);
        return 0;
    case WM_DESTROY:
        PostQuitMessage(0);
        return 0;
    case WM_COMMAND:
        if (LOWORD(wParam) == 1) {
            MessageBoxA(hwnd, "Hello, World!", "Hello", MB_OK | MB_ICONINFORMATION);
        } else if (LOWORD(wParam) == 2) {
            DestroyWindow(hwnd);
        }
        return 0;
    }
    return DefWindowProcA(hwnd, uMsg, wParam, lParam);
}
```

O arquivo analisado nunca é executado. Funciona offline; a única chamada de rede é o **Explicar com IA**, e só quando você pede.

## Explicar com IA

Explica a função aberta com o Claude (modelo `claude-opus-5-5`), em português, com o texto chegando aos poucos.

- Precisa de uma chave da API da Anthropic (crie em console.anthropic.com). O uso é cobrado na sua conta.
- Informe a chave no painel **IA** do inspetor, ou defina `ANTHROPIC_API_KEY`. A chave fica em `ai.json` na pasta de configuração do Hellius (`%APPDATA%\Hellius` no Windows, `~/.config/hellius` no Linux), com permissão só para o seu usuário. A interface nunca recebe a chave de volta.
- Na primeira vez, o app mostra exatamente o que será enviado e pede consentimento: o disassembly da função (até 3.000 linhas, com aviso se for cortado), o nome, o formato e a arquitetura do arquivo. O arquivo inteiro nunca sai do computador.
- O código vai marcado como dado, com instrução para o modelo não seguir ordens escritas dentro dele (strings de um malware podem tentar).
- A resposta é mostrada como texto simples, nunca como HTML.
- Se o modelo recusar por política, a API tenta um modelo alternativo automaticamente (`fallbacks: "default"`).
- No terminal: tecla `e`. Mesma chave e mesmo consentimento.

## Como rodar

Pré-requisitos (Linux): `libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev libxdo-dev build-essential pkg-config libssl-dev`.
Windows: WebView2 (já vem no Windows 11). Node 22+, Rust estável.

```bash
npm install
npm run tauri dev          # desenvolvimento
```

## Versão de terminal

```bash
cargo run -p hellius-tui -- caminho/do/arquivo     # ou sem argumento para digitar o caminho
cargo build -p hellius-tui --release               # binário em target/release/hellius-tui
```

Atalhos: `Tab` foco, `↑↓`/`jk` mover, `Enter` abrir função ou seguir call/jmp, `/` filtrar ou buscar,
`n`/`N` ocorrências, `:` ou `Ctrl+G` ir para endereço, `a`/`Ctrl+A` selecionar tudo, `y` copiar o escopo
inteiro (OSC 52), `e` explicar com IA, `1 2 3` tipo de código, `p` função/programa, `[ ]` histórico, `?` ajuda, `q` sair.
`NO_COLOR=1` desliga as cores. Não depende de GTK nem de servidor gráfico.

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
cargo test --workspace         # testes Rust (núcleo, TUI, app)
```

Testes cobrem: análise nativa do próprio binário de teste (um ELF no Linux, um PE no Windows: funções,
disassembly, pseudocódigo, strings, xrefs consistentes), classificação de fluxo x86/ARM64, endereços relativos
ao RIP, IA (parser de streaming, prompt com código marcado como dado e aviso de corte, corpo do pedido,
arquivo de chave com permissão 0600), painel de IA (chave, consentimento, streaming como texto, erros),
tokenizador, store, temas, design system, virtualização com "Selecionar tudo" + cópia completa, e a TUI
(abertura, seleção, busca, ir para endereço, seguir chamada, histórico, IA sem chave).

## Estrutura

```
crates/hellius-core/ núcleo: analyze.rs (validação e metadados), engine/native.rs (análise real),
                    engine/mock.rs (dados fixos para testes), ai.rs (Explicar com IA), render.rs, error.rs
                    examples/probe.rs: `cargo run -p hellius-core --example probe -- <arquivo> [função]`
crates/hellius-tui/ versão de terminal: app.rs (estado e teclado), ui.rs (desenho), tokens.rs (realce)
src-tauri/src/      lib.rs (builder), commands.rs (comandos Tauri)
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
- **Liquid Glass (redesenho)**: vidro só na camada funcional flutuante (barra de ferramentas, barra lateral, inspetor, popover, diálogos, toasts), como pede a diretriz da Apple (`liquid-glass.md`). Os controles dentro do vidro são `plain`: sem borda nem fundo em repouso, com preenchimento no hover e no estado pressionado. Isso acaba com o "botão dentro de botão". Um único botão de destaque por tela (`Copiar` no painel, `Abrir arquivo` no portal). O código fica na camada de conteúdo, opaca, sem vidro.
- **Metadados do arquivo** ficam no inspetor (texto), não em pílulas na barra. Pílula que parecia botão e não era foi removida.
- **Linhas de código não são botões**: o clique é delegado ao contêiner; o único clicável na linha é o destino de call/jmp (link de texto). Teclado: setas movem o cursor, Enter segue a chamada.
- **Reduzir transparência**: ajuste em Ajustes (ícone de engrenagem). Necessário porque o WebKitGTK (Linux) não expõe `prefers-reduced-transparency`.
- **Refração SVG** (`feDisplacementMap`) só é ativada onde há WebView2 (`Edg/` no User-Agent). WebKitGTK usa o fallback sem refração.
- **Fundo ambiente**: três manchas radiais com opacidade máxima 0.25 atrás de tudo, para o vidro aparecer sobre preto.
- **Tema padrão**: preto. Persistido em `localStorage`, com `try/catch`.
- **Layout das colunas** (larguras, recolhidas) é salvo em `localStorage`.
- **Renomear função** é só em memória, nesta sessão.
- **Ctrl+C e "Copiar"** copiam sempre o escopo completo, por pedido da especificação. Isso significa que copiar com um trecho selecionado também copia a função inteira.
- **Desempenho**: se o FPS medido nos 2 primeiros segundos ficar abaixo de 50, `data-perf="low"` desliga o blur dos contêineres não essenciais.

## Limitações conhecidas

- **Decompilador só em x86/x86-64.** ARM, AArch64 e MIPS ficam com a tradução instrução a instrução.
- **Decompilação é reconstrução**, não o fonte original (veja acima). Código muito otimizado ou ofuscado ainda gera `goto` e `__asm(...)` para instruções sem tradução (SSE/ponto flutuante, por exemplo). Tipos de variáveis locais saem genéricos (`int`, `int64_t`), e ponteiros aparecem como inteiros (`*(int *)(p + 8)`).
- **Corretude testada por comparação**: funções de teste compiladas com gcc e MinGW (`-O0` e `-O2`), decompiladas, recompiladas a partir do pseudocódigo e comparadas com as originais em várias entradas. Não é prova para todo binário.
- **Descoberta de funções é heurística.** Tabelas de `switch` são seguidas nos padrões de GCC e MSVC em x86/x86-64; outros formatos de tabela podem gerar funções falsas no meio de outra. Funções só alcançadas por ponteiros podem faltar fora do x86 (o preenchimento de lacunas é só x86). Em binários sem símbolos as funções se chamam `sub_<endereço>`.
- **Nomes de imports nas chamadas**: ELF x86/x86-64 (PLT e GOT) e PE (IAT). Em Mach-O e em ARM/MIPS os imports aparecem na lista, mas as chamadas ainda não recebem o nome.
- **Arquivo lido inteiro na memória** (até 512 MB). Binários muito grandes param em limites (30 mil funções, 6 milhões de instruções) e a interface avisa que a análise é parcial.
- **IA**: o caminho de rede foi testado até a API (chave inválida devolve o erro certo, em português), mas a resposta completa não foi vista aqui por falta de uma chave válida no ambiente.
- **Refração SVG** não foi verificada num WebView2 real. **Drag-and-drop** de arquivo não foi testado. **Rolagem suave** e FPS não foram medidos.
- **Minimapa, paleta Ctrl+K, exportação e anotações** (P2) não foram implementados.
- **Linhas de cabeçalho** de função no modo "programa inteiro" não recebem o realce de seleção.

## Verificação feita

- `typecheck`, `lint`, `vitest` (34 testes) e `cargo test --workspace` (50 testes: núcleo e TUI) passam; `cargo clippy` sem avisos.
- Decompilador: as funções de `tests/fixtures/t2.c` (recursão, `for`, `switch` por tabela, `while` com ponteiro) compiladas com gcc e MinGW em `-O0` e `-O2` foram decompiladas, o pseudocódigo foi recompilado como C e comparado com as originais em várias entradas: resultados iguais nas 4 combinações. O programa Win32 de `tests/fixtures/hello.c` sai com as mesmas chamadas, constantes e estrutura do fonte em 64 e 32 bits, com e sem símbolos. Varredura sem pânico em todas as funções de `ls`, `libc.so.6` e do `gcc` do MinGW.
- Análise conferida em `/bin/ls` (ELF x86-64, 301 funções em 0,12 s, `main` reconhecida, chamadas como `strrchr@plt` e strings anotadas), em `libc.so.6` (≈5 mil funções em ~1 s) e em PEs do Windows x86, x86-64 e ARM64 (chamadas como `KERNEL32.dll!GetStartupInfoW`).
- App no Xvfb (WebKitGTK): disassembly e pseudocódigo reais de `main` do `ls`, painel de IA pedindo a chave.
- TUI em terminal real (tmux): disassembly, pseudocódigo e mensagem de chave ausente na tecla `e`.
- Não verificados: Windows real (o build do Windows roda no GitHub Actions), resposta completa da IA, FPS, temas grafite e alto contraste no app.

## Achado durante a verificação

No WebKitGTK, `backdrop-filter` num botão com fundo próprio fez o fundo sumir. Por isso os botões não têm blur próprio; o vidro fica nos contêineres.
