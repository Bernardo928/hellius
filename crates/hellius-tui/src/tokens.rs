//! Tokenizador leve para realce (assembly e pseudocódigo C). Só classifica texto.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tok {
    Mnemonic,
    Reg,
    Num,
    Str,
    Target,
    Kw,
    Sym,
    Plain,
}

const REGS: &[&str] = &[
    "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "rip", "eax", "ebx", "ecx", "edx", "esi",
    "edi", "ebp", "esp", "eip", "ax", "bx", "cx", "dx", "si", "di", "bp", "sp", "al", "ah", "bl", "bh",
    "cl", "ch", "dl", "dh", "sil", "dil", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15",
];
const SIZE_KW: &[&str] = &["byte", "word", "dword", "qword", "ptr", "short", "near", "far"];
const C_KW: &[&str] = &[
    "int", "void", "char", "unsigned", "return", "if", "else", "for", "while", "do", "switch", "case",
    "break", "continue", "struct", "const", "static", "sizeof", "uint8_t", "uint32_t", "uint64_t",
];

/// Divide em pedaços: espaços, identificadores, números, strings ou um caractere.
fn split(text: &str) -> Vec<&str> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let start = i;
        let c = b[i];
        if c.is_ascii_whitespace() {
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
        } else if c == b'"' {
            i += 1;
            while i < b.len() && b[i] != b'"' {
                i += if b[i] == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(b.len());
        } else if c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'$' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.' || b[i] == b'$') {
                i += 1;
            }
        } else {
            // Um caractere (pode ser multibyte: avança até o próximo limite UTF-8).
            i += 1;
            while i < b.len() && !text.is_char_boundary(i) {
                i += 1;
            }
        }
        out.push(&text[start..i]);
    }
    out
}

fn is_num(s: &str) -> bool {
    let h = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    match h {
        Some(rest) => !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()),
        None => !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()),
    }
}

pub fn asm(text: &str) -> Vec<(Tok, &str)> {
    let mut out: Vec<(Tok, &str)> = Vec::new();
    let mut seen_mnemonic = false;
    for part in split(text) {
        let lower = part.to_ascii_lowercase();
        let kind = if part.trim().is_empty() {
            Tok::Plain
        } else if part.starts_with('"') {
            Tok::Str
        } else if is_num(part) {
            Tok::Num
        } else if !seen_mnemonic && part.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            seen_mnemonic = true;
            Tok::Mnemonic
        } else if REGS.contains(&lower.as_str()) {
            Tok::Reg
        } else if SIZE_KW.contains(&lower.as_str()) {
            Tok::Kw
        } else if part.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') {
            Tok::Sym
        } else {
            Tok::Plain
        };
        out.push((kind, part));
    }
    let is_branch = out
        .iter()
        .find(|(k, _)| *k == Tok::Mnemonic)
        .is_some_and(|(_, m)| {
            let m = m.to_ascii_lowercase();
            m == "call" || m == "jmp" || m == "loop" || (m.starts_with('j') && m.len() <= 4)
        });
    if is_branch {
        if let Some(t) = out.iter_mut().find(|(k, _)| *k == Tok::Num) {
            t.0 = Tok::Target;
        }
    }
    out
}

pub fn c(text: &str) -> Vec<(Tok, &str)> {
    split(text)
        .into_iter()
        .map(|part| {
            let kind = if part.trim().is_empty() {
                Tok::Plain
            } else if part.starts_with('"') {
                Tok::Str
            } else if is_num(part) {
                Tok::Num
            } else if C_KW.contains(&part) {
                Tok::Kw
            } else if part.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') {
                Tok::Sym
            } else {
                Tok::Plain
            };
            (kind, part)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mnemonic_register_number() {
        let t = asm("mov eax, 0x10");
        assert_eq!(t[0], (Tok::Mnemonic, "mov"));
        assert!(t.contains(&(Tok::Reg, "eax")));
        assert!(t.contains(&(Tok::Num, "0x10")));
    }

    #[test]
    fn call_target() {
        assert!(asm("call 0x140001040").contains(&(Tok::Target, "0x140001040")));
    }

    #[test]
    fn utf8_is_safe() {
        let t = asm("mov eax, ç");
        assert_eq!(t.iter().map(|(_, s)| *s).collect::<String>(), "mov eax, ç");
    }

    #[test]
    fn c_keywords() {
        assert_eq!(c("int x")[0], (Tok::Kw, "int"));
    }
}
