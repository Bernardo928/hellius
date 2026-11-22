//! `MockEngine`: dados determinísticos de demonstração.
//!
//! Os metadados do arquivo vêm do arquivo real (em `analyze.rs`). Funções, código,
//! strings e imports aqui são INVENTADOS para a interface funcionar sem Ghidra.
//! A UI mostra "Modo demonstração" sempre que este engine está ativo.

use std::collections::BTreeMap;

use super::{format_address, CodeKind, CodeLine, Engine, FunctionInfo, ImportInfo, StringInfo};
use crate::error::AppError;

/// Número de linhas da função sintética usada para teste de desempenho.
pub const SYNTHETIC_LINES: usize = 20_000;
pub const SYNTHETIC_ADDRESS: u64 = 0x1_4001_0000;

struct Ins {
    bytes: &'static str,
    text: &'static str,
    comment: Option<&'static str>,
    /// Destino de `call`/`jmp` para outra função de demonstração, por endereço.
    target: Option<u64>,
}

const fn i(bytes: &'static str, text: &'static str) -> Ins {
    Ins { bytes, text, comment: None, target: None }
}
const fn c(bytes: &'static str, text: &'static str, comment: &'static str) -> Ins {
    Ins { bytes, text, comment: Some(comment), target: None }
}
const fn call(bytes: &'static str, text: &'static str, target: u64) -> Ins {
    Ins { bytes, text, comment: None, target: Some(target) }
}

struct Def {
    name: &'static str,
    address: u64,
    asm: &'static [Ins],
    pseudo: &'static [&'static str],
}

const A_ENTRY: u64 = 0x1_4000_1000;
const A_MAIN: u64 = 0x1_4000_1040;
const A_PARSE: u64 = 0x1_4000_10a0;
const A_HEADER: u64 = 0x1_4000_1120;
const A_CHECKSUM: u64 = 0x1_4000_1180;
const A_HASH: u64 = 0x1_4000_11e0;
const A_LOG: u64 = 0x1_4000_1240;
const A_CLEANUP: u64 = 0x1_4000_1280;

const ASM_ENTRY: &[Ins] = &[
    c("48 83 ec 28", "sub rsp, 0x28", "reserva espaço de sombra"),
    call("e8 f7 ff ff ff", "call 0x140001040", A_MAIN),
    i("89 c1", "mov ecx, eax"),
    call("ff 15 e2 2f 00 00", "call qword ptr [0x140004008]", A_CLEANUP),
    i("48 83 c4 28", "add rsp, 0x28"),
    i("c3", "ret"),
];

const PSEUDO_ENTRY: &[&str] = &[
    "int entry(void) {",
    "    int code = main();",
    "    cleanup();",
    "    return code;",
    "}",
];

const ASM_MAIN: &[Ins] = &[
    c("48 83 ec 38", "sub rsp, 0x38", "quadro de pilha local"),
    i("48 89 4c 24 40", "mov qword ptr [rsp + 0x40], rcx"),
    call("e8 56 00 00 00", "call 0x1400010a0", A_PARSE),
    i("85 c0", "test eax, eax"),
    i("75 0c", "jne 0x140001066"),
    call("e8 b5 00 00 00", "call 0x140001120", A_HEADER),
    c("31 c0", "xor eax, eax", "retorno 0 = sucesso"),
    i("48 83 c4 38", "add rsp, 0x38"),
    i("c3", "ret"),
];

const PSEUDO_MAIN: &[&str] = &[
    "int main(int argc, char **argv) {",
    "    if (parse_config(argc, argv) != 0) {",
    "        return 1;",
    "    }",
    "    read_header();",
    "    return 0;",
    "}",
];

const ASM_PARSE: &[Ins] = &[
    c("48 83 ec 28", "sub rsp, 0x28", "espaço local"),
    i("48 8b 41 08", "mov rax, qword ptr [rcx + 8]"),
    i("48 85 c0", "test rax, rax"),
    i("74 1a", "je 0x1400010c2"),
    c("0f b6 08", "movzx ecx, byte ptr [rax]", "primeiro byte da entrada"),
    i("80 f9 23", "cmp cl, 0x23"),
    i("74 0b", "je 0x1400010c2"),
    call("e8 bb 00 00 00", "call 0x140001180", A_CHECKSUM),
    i("31 c0", "xor eax, eax"),
    i("48 83 c4 28", "add rsp, 0x28"),
    i("c3", "ret"),
];

const PSEUDO_PARSE: &[&str] = &[
    "int parse_config(int argc, char **argv) {",
    "    char *line = argv[1];",
    "    if (!line) return -1;",
    "    if (line[0] == '#') return -1;",
    "    return validate_checksum(line);",
    "}",
];

const ASM_HEADER: &[Ins] = &[
    c("48 8b 05 d9 2e 00 00", "mov rax, qword ptr [rip + 0x2ed9]", "carrega ponteiro global"),
    i("8b 00", "mov eax, dword ptr [rax]"),
    i("3d 4d 5a 00 00", "cmp eax, 0x5a4d"),
    i("75 05", "jne 0x14000113e"),
    call("e8 e6 00 00 00", "call 0x1400011e0", A_HASH),
    i("c3", "ret"),
];

const PSEUDO_HEADER: &[&str] = &[
    "void read_header(void) {",
    "    uint32_t magic = *g_header;",
    "    if (magic == 0x5a4d) {",
    "        compute_hash();",
    "    }",
    "}",
];

const ASM_CHECKSUM: &[Ins] = &[
    c("31 c0", "xor eax, eax", "acumulador"),
    i("31 d2", "xor edx, edx"),
    i("0f b6 0c 11", "movzx ecx, byte ptr [rcx + rdx]"),
    i("01 c8", "add eax, ecx"),
    i("ff c2", "inc edx"),
    i("83 fa 40", "cmp edx, 0x40"),
    i("72 f3", "jb 0x140001187"),
    i("3d ff 00 00 00", "cmp eax, 0xff"),
    i("0f 94 c0", "sete al"),
    i("c3", "ret"),
];

const PSEUDO_CHECKSUM: &[&str] = &[
    "int validate_checksum(const uint8_t *buf) {",
    "    unsigned acc = 0;",
    "    for (int i = 0; i < 64; i++) acc += buf[i];",
    "    return acc == 0xff;",
    "}",
];

const ASM_HASH: &[Ins] = &[
    c("48 81 ec 80 00 00 00", "sub rsp, 0x80", "buffer de trabalho"),
    i("48 8d 0d 3b 2e 00 00", "lea rcx, [rip + 0x2e3b]"),
    call("e8 a0 ff ff ff", "call 0x140001240", A_LOG),
    c("b8 67 45 23 01", "mov eax, 0x01234567", "constante inicial do estado"),
    i("69 c0 01 00 01 00", "imul eax, eax, 0x10001"),
    i("35 ef cd ab 89", "xor eax, 0x89abcdef"),
    i("48 81 c4 80 00 00 00", "add rsp, 0x80"),
    i("c3", "ret"),
];

const PSEUDO_HASH: &[&str] = &[
    "uint32_t compute_hash(void) {",
    "    log_message(\"hash iniciado\");",
    "    uint32_t h = 0x01234567;",
    "    h = h * 0x10001 ^ 0x89abcdef;",
    "    return h;",
    "}",
];

const ASM_LOG: &[Ins] = &[
    c("48 83 ec 28", "sub rsp, 0x28", "espaço de shadow"),
    i("48 89 c8", "mov rax, rcx"),
    i("ff 25 0e 2e 00 00", "jmp qword ptr [rip + 0x2e0e]"),
];

const PSEUDO_LOG: &[&str] = &["void log_message(const char *msg) {", "    puts(msg);", "}"];

const ASM_CLEANUP: &[Ins] = &[
    c("48 83 ec 28", "sub rsp, 0x28", "espaço de shadow"),
    i("33 c9", "xor ecx, ecx"),
    i("e8 00 00 00 00", "call 0x140001289"),
    i("48 83 c4 28", "add rsp, 0x28"),
    i("c3", "ret"),
];

const PSEUDO_CLEANUP: &[&str] = &["void cleanup(void) {", "    free_all_buffers();", "}"];

const DEFS: &[Def] = &[
    Def { name: "entry", address: A_ENTRY, asm: ASM_ENTRY, pseudo: PSEUDO_ENTRY },
    Def { name: "main", address: A_MAIN, asm: ASM_MAIN, pseudo: PSEUDO_MAIN },
    Def { name: "parse_config", address: A_PARSE, asm: ASM_PARSE, pseudo: PSEUDO_PARSE },
    Def { name: "read_header", address: A_HEADER, asm: ASM_HEADER, pseudo: PSEUDO_HEADER },
    Def { name: "validate_checksum", address: A_CHECKSUM, asm: ASM_CHECKSUM, pseudo: PSEUDO_CHECKSUM },
    Def { name: "compute_hash", address: A_HASH, asm: ASM_HASH, pseudo: PSEUDO_HASH },
    Def { name: "log_message", address: A_LOG, asm: ASM_LOG, pseudo: PSEUDO_LOG },
    Def { name: "cleanup", address: A_CLEANUP, asm: ASM_CLEANUP, pseudo: PSEUDO_CLEANUP },
];

struct Func {
    info: FunctionInfo,
    disasm: Vec<CodeLine>,
    pseudo: Vec<CodeLine>,
    hex: Vec<CodeLine>,
}

pub struct MockEngine {
    funcs: BTreeMap<u64, Func>,
}

impl Default for MockEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEngine {
    pub fn new() -> Self {
        let mut funcs: BTreeMap<u64, Func> = BTreeMap::new();

        // Primeiro passo: quem chama quem (para xrefs).
        let mut xrefs_in: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        let mut xrefs_out: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
        for d in DEFS {
            for ins in d.asm {
                if let Some(t) = ins.target {
                    xrefs_out.entry(d.address).or_default().push(t);
                    xrefs_in.entry(t).or_default().push(d.address);
                }
            }
        }

        for d in DEFS {
            let disasm: Vec<CodeLine> = d.asm.iter().enumerate().map(|(idx, ins)| CodeLine {
                address: format_address(d.address + offset_of(d.asm, idx)),
                bytes: Some(ins.bytes.to_string()),
                text: ins.text.to_string(),
                comment: ins.comment.map(str::to_string),
                target: ins.target.map(format_address),
            }).collect();
            let pseudo = pseudo_lines(d.address, d.pseudo);
            let hex = hex_lines(d.address, &disasm);
            let size: u64 = d.asm.iter().map(|ins| ins.bytes.split(' ').count() as u64).sum();
            funcs.insert(d.address, Func {
                info: FunctionInfo {
                    address: format_address(d.address),
                    name: d.name.to_string(),
                    size_bytes: size,
                    block_count: (d.asm.iter().filter(|x| x.target.is_some() || x.text.starts_with('j')).count() as u32) + 1,
                    xrefs_in: xrefs_in.get(&d.address).map(|v| v.iter().map(|a| format_address(*a)).collect()).unwrap_or_default(),
                    xrefs_out: xrefs_out.get(&d.address).map(|v| v.iter().map(|a| format_address(*a)).collect()).unwrap_or_default(),
                },
                disasm,
                pseudo,
                hex,
            });
        }

        funcs.insert(SYNTHETIC_ADDRESS, synthetic());
        Self { funcs }
    }
}

/// Deslocamento de uma instrução dentro da função, somando tamanhos em bytes.
fn offset_of(asm: &[Ins], idx: usize) -> u64 {
    asm.iter().take(idx).map(|ins| ins.bytes.split(' ').count() as u64).sum()
}

fn pseudo_lines(base: u64, src: &[&str]) -> Vec<CodeLine> {
    src.iter().enumerate().map(|(n, line)| CodeLine {
        address: format_address(base + n as u64),
        bytes: None,
        text: (*line).to_string(),
        comment: None,
        target: None,
    }).collect()
}

/// Linhas hexadecimais de 16 bytes, com texto ASCII ao lado.
fn hex_lines(base: u64, disasm: &[CodeLine]) -> Vec<CodeLine> {
    let all: Vec<u8> = disasm
        .iter()
        .filter_map(|l| l.bytes.as_deref())
        .flat_map(|b| b.split(' ').filter_map(|h| u8::from_str_radix(h, 16).ok()))
        .collect();
    all.chunks(16).enumerate().map(|(n, chunk)| CodeLine {
        address: format_address(base + (n as u64) * 16),
        bytes: Some(chunk.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" ")),
        text: chunk.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }).collect(),
        comment: None,
        target: None,
    }).collect()
}

/// Função sintética com 20.000 linhas de disassembly, pseudocódigo e hex.
fn synthetic() -> Func {
    let base = SYNTHETIC_ADDRESS;
    let mut disasm = Vec::with_capacity(SYNTHETIC_LINES);
    let mut pseudo = Vec::with_capacity(SYNTHETIC_LINES);
    let mut addr = base;
    for n in 0..SYNTHETIC_LINES {
        let (bytes, text) = match n % 4 {
            0 => ("8b 45 f0", format!("mov eax, dword ptr [rbp - 0x{:x}]", (n % 64) * 4)),
            1 => ("05 0d 00 00 00", format!("add eax, 0x{:x}", n % 4096)),
            2 => ("3d 00 10 00 00", "cmp eax, 0x1000".to_string()),
            _ => ("89 45 f0", format!("mov dword ptr [rbp - 0x{:x}], eax", (n % 64) * 4)),
        };
        disasm.push(CodeLine {
            address: format_address(addr),
            bytes: Some(bytes.to_string()),
            text,
            comment: None,
            target: None,
        });
        pseudo.push(CodeLine {
            address: format_address(base + n as u64),
            bytes: None,
            text: format!("acc = (acc * 31 + table[{}]) & 0xffff;", n % 256),
            comment: None,
            target: None,
        });
        addr += bytes.split(' ').count() as u64;
    }
    let hex = hex_lines(base, &disasm);
    // O tamanho em bytes da função sintética é a soma das instruções acima.
    let size = addr - base;
    Func {
        info: FunctionInfo {
            address: format_address(base),
            name: "funcao_sintetica_20k".to_string(),
            size_bytes: size,
            block_count: (SYNTHETIC_LINES / 4) as u32,
            xrefs_in: vec![],
            xrefs_out: vec![],
        },
        disasm,
        pseudo,
        hex,
    }
}

impl Engine for MockEngine {
    fn name(&self) -> &'static str {
        "Demonstração (MockEngine)"
    }

    fn is_demo(&self) -> bool {
        true
    }

    fn list_functions(&self) -> Vec<FunctionInfo> {
        self.funcs.values().map(|f| f.info.clone()).collect()
    }

    fn lines(&self, function_address: u64, kind: CodeKind) -> Result<&[CodeLine], AppError> {
        let f = self
            .funcs
            .get(&function_address)
            .ok_or_else(|| AppError::FunctionNotFound(format_address(function_address)))?;
        Ok(match kind {
            CodeKind::Disasm => &f.disasm,
            CodeKind::Pseudo => &f.pseudo,
            CodeKind::Hex => &f.hex,
        })
    }

    fn list_strings(&self) -> Vec<StringInfo> {
        [
            (0x1_4000_4100u64, "Uso: hellius <arquivo>"),
            (0x1_4000_4120, "configuração inválida"),
            (0x1_4000_4140, "hash iniciado"),
            (0x1_4000_4160, "erro: cabeçalho corrompido"),
        ]
        .iter()
        .map(|(a, v)| StringInfo { address: format_address(*a), value: (*v).to_string() })
        .collect()
    }

    fn list_imports(&self) -> Vec<ImportInfo> {
        [
            ("kernel32.dll", "ReadFile"),
            ("kernel32.dll", "CloseHandle"),
            ("msvcrt.dll", "puts"),
            ("msvcrt.dll", "free"),
        ]
        .iter()
        .map(|(l, n)| ImportInfo { library: (*l).to_string(), name: (*n).to_string() })
        .collect()
    }

    fn function_addresses(&self) -> Vec<u64> {
        self.funcs.keys().copied().collect()
    }

    fn function_name(&self, address: u64) -> Option<String> {
        self.funcs.get(&address).map(|f| f.info.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_has_exactly_20k_lines_per_kind() {
        let e = MockEngine::new();
        assert_eq!(e.lines(SYNTHETIC_ADDRESS, CodeKind::Disasm).unwrap().len(), SYNTHETIC_LINES);
        assert_eq!(e.lines(SYNTHETIC_ADDRESS, CodeKind::Pseudo).unwrap().len(), SYNTHETIC_LINES);
    }

    #[test]
    fn xrefs_are_symmetric() {
        let e = MockEngine::new();
        let fns = e.list_functions();
        let main = fns.iter().find(|f| f.name == "main").unwrap();
        assert!(main.xrefs_in.contains(&"0x140001000".to_string()));
        let parse = fns.iter().find(|f| f.name == "parse_config").unwrap();
        assert!(parse.xrefs_in.contains(&"0x140001040".to_string()));
    }

    #[test]
    fn unknown_function_is_error_not_panic() {
        let e = MockEngine::new();
        assert!(e.lines(0xdead, CodeKind::Disasm).is_err());
    }

    #[test]
    fn hex_lines_are_16_bytes() {
        let e = MockEngine::new();
        let hex = e.lines(A_MAIN, CodeKind::Hex).unwrap();
        assert!(!hex.is_empty());
        assert!(hex[0].bytes.as_deref().unwrap().split(' ').count() <= 16);
    }
}
