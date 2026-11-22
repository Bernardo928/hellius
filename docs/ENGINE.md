# Contrato do engine

A interface nunca fala direto com Ghidra ou rizin. Ela chama **comandos Tauri tipados**
(`src/api/hellius.ts`), e o Rust implementa a trait `Engine` (`src-tauri/src/engine/mod.rs`).
Trocar o engine não exige mudar a UI.

## Trait `Engine`

```rust
pub trait Engine: Send + Sync {
    fn name(&self) -> &'static str;                  // ex.: "Demonstração (MockEngine)"
    fn is_demo(&self) -> bool;                       // true => a UI mostra o aviso de demonstração
    fn list_functions(&self) -> Vec<FunctionInfo>;
    fn lines(&self, function_address: u64, kind: CodeKind) -> Result<&[CodeLine], AppError>;
    fn list_strings(&self) -> Vec<StringInfo>;
    fn list_imports(&self) -> Vec<ImportInfo>;
    fn function_addresses(&self) -> Vec<u64>;
    fn function_name(&self, address: u64) -> Option<String>;
}
```

`lines` devolve todas as linhas da função para o tipo pedido. Para o escopo "programa inteiro", a
UI itera `function_addresses()`. Um engine real pode gerar as linhas sob demanda e guardar um cache.

## Tipos (JSON, camelCase)

```ts
type Arch = 'x86' | 'x86_64' | 'arm' | 'aarch64' | 'mips' | 'unknown';
interface FileSummary {
  path: string; name: string; sizeBytes: number; sha256: string;
  format: 'PE' | 'ELF' | 'MachO' | 'Raw'; arch: Arch;
  bits: 0 | 16 | 32 | 64;          // 0 = desconhecido (formato Raw)
  entryPoint: string;               // hex, ex. "0x140001000"
  sections: { name: string; address: string; size: number; flags: string }[];
}
interface FunctionInfo {
  address: string; name: string; sizeBytes: number; blockCount: number;
  xrefsIn: string[]; xrefsOut: string[];   // endereços
}
interface CodeLine {
  address: string; bytes?: string; text: string; comment?: string; target?: string;
}
interface CodeSlice {
  functionAddress: string; kind: 'disasm' | 'pseudo' | 'hex';
  totalLines: number; startLine: number; lines: CodeLine[];
}
```

`flags` das seções: ELF usa `A` (alocada), `W` (gravável), `X` (executável). PE usa `R`, `W`, `X`.

## Comandos Tauri

| Comando | Entrada | Saída | Observação |
| --- | --- | --- | --- |
| `open_file` | `path` | `FileSummary` | Valida o caminho; emite eventos `analysis:progress` (`{phase, step, total}`). |
| `engine_info` | — | `{name, isDemo}` | |
| `list_functions` | — | `FunctionInfo[]` | |
| `get_code` | `functionAddress`, `kind`, `startLine`, `count` | `CodeSlice` | `count` é limitado a 2000 linhas por chamada. |
| `get_full_text` | `scope` (`function`\|`program`), `kind`, `functionAddress?` | `string` | Texto completo para copiar. |
| `list_strings` | — | `{address, value}[]` | |
| `list_imports` | — | `{library, name}[]` | |

Erros voltam como string em português.

## Validação do arquivo (antes de qualquer engine)

- Caminho precisa existir e não pode ser pasta.
- Arquivo vazio é rejeitado.
- Acima de 512 MB é rejeitado.
- O conteúdo é lido em memória apenas para o hash SHA-256 e para os cabeçalhos (`goblin`).
  Nada é executado, carregado como biblioteca ou aberto pelo sistema.

## `NativeEngine` (padrão)

`crates/hellius-core/src/engine/native.rs`. Lê o arquivo uma vez (`open_native`) e:

1. Mapeia endereços virtuais para o arquivo (segmentos `PT_LOAD` do ELF, seções do PE, segmentos do Mach-O).
2. Coleta sementes: ponto de entrada, símbolos de função, exports.
3. Faz descida recursiva por função com capstone: segue saltos condicionais e incondicionais, registra chamadas
   e referências a código (`lea`/`mov` de endereços executáveis) como novas sementes.
4. Em x86, preenche lacunas entre funções (código só alcançado por ponteiro), pulando o preenchimento.
5. Reconhece stubs de import (`jmp [slot]` em PLT/IAT) e os trata como nomes, não funções.
6. Monta xrefs a partir das chamadas. Disassembly, pseudocódigo e hex de cada função são gerados sob demanda
   e guardados (`OnceLock`).

`get_totals(kind)` devolve o total de linhas de todas as funções numa chamada (escopo "programa inteiro").
`engine_info` informa `partial` (parou num limite) e `hasCode` (há disassembler para a arquitetura).

## Explicar com IA

`crates/hellius-core/src/ai.rs`. Comandos Tauri: `ai_status`, `ai_save_settings`, `ai_explain(functionAddress,
requestId)` (texto em eventos `ai:delta`, retorno com o texto completo) e `ai_cancel`. A chave nunca volta
para o front. Pedido: `POST /v1/messages` com `stream: true`, `model: claude-opus-5-5`,
`output_config.effort: medium`, `fallbacks: "default"` (beta `server-side-fallback-2026-07-01`).

## `MockEngine`

- Funções fixas de exemplo (`entry`, `main`, `parse_config`, etc.) com assembly x86-64 e pseudocódigo.
  Os bytes são ilustrativos, não correspondem ao arquivo real.
- Uma função sintética, `funcao_sintetica_20k`, com 20.000 linhas em cada tipo (teste de desempenho).
- Xrefs são calculados a partir das chamadas entre as funções de exemplo.
- Usado só em testes. Os metadados de `FileSummary` são reais; funções e código do mock são inventados.

## Como adicionar um engine real

1. Crie `src-tauri/src/engine/ghidra.rs` (ou `rizin.rs`) implementando `Engine`.
2. Na abertura (`commands::open_file`), escolha o engine. Mantenha o `MockEngine` como fallback.
3. `is_demo()` deve devolver `false` para que o aviso "Modo demonstração" desapareça.
4. Não altere os tipos JSON sem atualizar `src/api/types.ts` e este documento.
