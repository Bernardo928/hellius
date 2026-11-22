//! Validação do caminho e leitura de metadados reais do arquivo.
//! O arquivo é lido em memória apenas para hash e cabeçalhos. Nunca é executado.

use std::fs;
use std::path::Path;

use goblin::mach::Mach;
use goblin::Object;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::AppError;

pub const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub name: String,
    pub address: String,
    pub size: u64,
    pub flags: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileSummary {
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub sha256: String,
    /// "PE" | "ELF" | "MachO" | "Raw"
    pub format: String,
    /// "x86" | "x86_64" | "arm" | "aarch64" | "mips" | "unknown"
    pub arch: String,
    /// 16 | 32 | 64, ou 0 quando desconhecido (formato Raw).
    pub bits: u8,
    pub entry_point: String,
    pub sections: Vec<Section>,
}

/// Verifica o caminho antes de qualquer leitura.
pub fn validate_path(path: &str) -> Result<(), AppError> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(AppError::NotFound(path.to_string()));
    }
    if p.is_dir() {
        return Err(AppError::IsDirectory);
    }
    let len = fs::metadata(p)?.len();
    if len == 0 {
        return Err(AppError::Empty);
    }
    if len > MAX_FILE_BYTES {
        return Err(AppError::TooLarge(len.div_ceil(1024 * 1024)));
    }
    Ok(())
}

/// Lê o arquivo e devolve os metadados. `progress(fase, passo, total)` informa a UI.
pub fn summarize(
    path: &str,
    progress: &mut dyn FnMut(&str, u32, u32),
) -> Result<FileSummary, AppError> {
    progress("Validando arquivo", 1, 4);
    validate_path(path)?;
    let bytes = fs::read(path)?;

    let summary = summary_from_bytes(path, &bytes, progress);
    Ok(summary)
}

/// Metadados a partir dos bytes já lidos (evita ler o arquivo duas vezes).
pub fn summary_from_bytes(
    path: &str,
    bytes: &[u8],
    progress: &mut dyn FnMut(&str, u32, u32),
) -> FileSummary {
    progress("Calculando SHA-256", 2, 4);
    let sha256 = format!("{:x}", Sha256::digest(bytes));

    progress("Lendo cabeçalhos", 3, 4);
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());

    let base = FileSummary {
        path: path.to_string(),
        name,
        size_bytes: bytes.len() as u64,
        sha256,
        format: "Raw".into(),
        arch: "unknown".into(),
        bits: 0,
        entry_point: "0x0".into(),
        sections: vec![],
    };

    progress("Montando resumo", 4, 4);
    match Object::parse(bytes) {
        Ok(Object::Elf(elf)) => FileSummary {
            format: "ELF".into(),
            arch: elf_arch(elf.header.e_machine).into(),
            bits: if elf.is_64 { 64 } else { 32 },
            entry_point: format!("0x{:x}", elf.entry),
            sections: elf
                .section_headers
                .iter()
                .map(|sh| Section {
                    name: elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("").to_string(),
                    address: format!("0x{:x}", sh.sh_addr),
                    size: sh.sh_size,
                    flags: elf_flags(sh.sh_flags),
                })
                .collect(),
            ..base
        },
        Ok(Object::PE(pe)) => FileSummary {
            format: "PE".into(),
            arch: pe_arch(pe.header.coff_header.machine).into(),
            bits: if pe.is_64 { 64 } else { 32 },
            entry_point: format!("0x{:x}", pe.image_base + pe.entry as u64),
            sections: pe
                .sections
                .iter()
                .map(|s| Section {
                    name: s.name().unwrap_or("").to_string(),
                    address: format!("0x{:x}", pe.image_base + s.virtual_address as u64),
                    size: u64::from(s.virtual_size),
                    flags: pe_flags(s.characteristics),
                })
                .collect(),
            ..base
        },
        Ok(Object::Mach(Mach::Binary(m))) => FileSummary {
            format: "MachO".into(),
            arch: mach_arch(m.header.cputype).into(),
            bits: if m.is_64 { 64 } else { 32 },
            entry_point: format!("0x{:x}", m.entry),
            ..base
        },
        Ok(Object::Mach(Mach::Fat(_))) => FileSummary {
            format: "MachO".into(),
            bits: 64,
            ..base
        },
        _ => base,
    }
}

fn elf_arch(m: u16) -> &'static str {
    match m {
        3 => "x86",
        62 => "x86_64",
        40 => "arm",
        183 => "aarch64",
        8 => "mips",
        _ => "unknown",
    }
}

fn pe_arch(m: u16) -> &'static str {
    match m {
        0x014c => "x86",
        0x8664 => "x86_64",
        0x01c0 | 0x01c4 => "arm",
        0xaa64 => "aarch64",
        0x0166 => "mips",
        _ => "unknown",
    }
}

fn mach_arch(cpu: u32) -> &'static str {
    match cpu {
        0x7 => "x86",
        0x0100_0007 => "x86_64",
        0xc => "arm",
        0x0100_000c => "aarch64",
        _ => "unknown",
    }
}

/// Flags de seção ELF no formato curto: A (alocada), W (gravável), X (executável).
fn elf_flags(f: u64) -> String {
    let mut s = String::new();
    if f & 0x2 != 0 { s.push('A'); }
    if f & 0x1 != 0 { s.push('W'); }
    if f & 0x4 != 0 { s.push('X'); }
    if s.is_empty() { s.push('-'); }
    s
}

fn pe_flags(c: u32) -> String {
    let mut s = String::new();
    if c & 0x4000_0000 != 0 { s.push('R'); }
    if c & 0x8000_0000 != 0 { s.push('W'); }
    if c & 0x2000_0000 != 0 { s.push('X'); }
    if s.is_empty() { s.push('-'); }
    s
}

/// Abre um arquivo: valida, lê uma vez, extrai metadados e analisa o código.
pub fn open_native(
    path: &str,
    progress: &mut dyn FnMut(&str, u32, u32),
) -> Result<(FileSummary, crate::engine::native::NativeEngine), AppError> {
    progress("Validando arquivo", 1, 6);
    validate_path(path)?;
    let bytes = fs::read(path)?;
    let summary = summary_from_bytes(path, &bytes, &mut |p, _, _| progress(p, 2, 6));
    let mut step = 3;
    let engine = crate::engine::native::NativeEngine::analyze(bytes, &mut |p| {
        progress(p, step.min(6), 6);
        step += 1;
    });
    Ok((summary, engine))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_path() {
        assert!(matches!(validate_path("/nao/existe/xyz"), Err(AppError::NotFound(_))));
    }

    #[test]
    fn rejects_directory() {
        let dir = std::env::temp_dir();
        assert!(matches!(validate_path(dir.to_str().unwrap()), Err(AppError::IsDirectory)));
    }

    #[test]
    fn rejects_empty_file() {
        let dir = std::env::temp_dir().join("hellius-test-empty");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("vazio.bin");
        std::fs::write(&f, b"").unwrap();
        assert!(matches!(validate_path(f.to_str().unwrap()), Err(AppError::Empty)));
    }

    #[test]
    fn raw_file_is_raw_format_with_sha256() {
        let dir = std::env::temp_dir().join("hellius-test-raw");
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("dados.bin");
        std::fs::write(&f, b"abc").unwrap();
        let s = summarize(f.to_str().unwrap(), &mut |_, _, _| {}).unwrap();
        assert_eq!(s.format, "Raw");
        assert_eq!(
            s.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn reads_real_elf_metadata() {
        let p = "/bin/ls";
        if !std::path::Path::new(p).exists() {
            return;
        }
        let s = summarize(p, &mut |_, _, _| {}).unwrap();
        assert_eq!(s.format, "ELF");
        assert_eq!(s.arch, "x86_64");
        assert_eq!(s.bits, 64);
        assert!(!s.sections.is_empty());
    }
}
