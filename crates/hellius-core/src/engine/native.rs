//! `NativeEngine`: análise real do arquivo, em Rust, sem dependências externas.
//!
//! - Disassembly com capstone (x86, x86-64, ARM, Thumb, AArch64, MIPS).
//! - Funções descobertas a partir do ponto de entrada, dos símbolos/exports e
//!   das chamadas encontradas (descida recursiva).
//! - Imports, strings e referências cruzadas reais.
//! - Pseudocódigo **simplificado**: tradução instrução a instrução para um C
//!   aproximado. Não é um decompilador (não reconstrói tipos, laços nem variáveis).
//!
//! O arquivo é só lido. Nada é executado.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::OnceLock;

use capstone::prelude::*;
use goblin::mach::Mach;
use goblin::Object;

use super::{format_address, CodeKind, CodeLine, Engine, FunctionInfo, ImportInfo, StringInfo};
use crate::error::AppError;

/// Limites para arquivos grandes (a análise para e marca o resultado como parcial).
const MAX_FUNCTIONS: usize = 30_000;
const MAX_INSNS_PER_FUNCTION: usize = 200_000;
const MAX_TOTAL_INSNS: usize = 6_000_000;
const MAX_STRINGS: usize = 60_000;
const MIN_STRING: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuArch {
    X86,
    X64,
    Arm,
    Thumb,
    Arm64,
    MipsLe,
    MipsBe,
}

impl CpuArch {
    fn capstone(self) -> Result<Capstone, AppError> {
        let cs = match self {
            CpuArch::X86 => Capstone::new()
                .x86()
                .mode(arch::x86::ArchMode::Mode32)
                .syntax(arch::x86::ArchSyntax::Intel)
                .build(),
            CpuArch::X64 => Capstone::new()
                .x86()
                .mode(arch::x86::ArchMode::Mode64)
                .syntax(arch::x86::ArchSyntax::Intel)
                .build(),
            CpuArch::Arm => Capstone::new().arm().mode(arch::arm::ArchMode::Arm).build(),
            CpuArch::Thumb => Capstone::new().arm().mode(arch::arm::ArchMode::Thumb).build(),
            CpuArch::Arm64 => Capstone::new().arm64().mode(arch::arm64::ArchMode::Arm).build(),
            CpuArch::MipsLe => Capstone::new()
                .mips()
                .mode(arch::mips::ArchMode::Mips32)
                .endian(capstone::Endian::Little)
                .build(),
            CpuArch::MipsBe => Capstone::new()
                .mips()
                .mode(arch::mips::ArchMode::Mips32)
                .endian(capstone::Endian::Big)
                .build(),
        };
        cs.map_err(|e| AppError::Analysis(format!("disassembler indisponível: {e}")))
    }

    fn is_x86(self) -> bool {
        matches!(self, CpuArch::X86 | CpuArch::X64)
    }
}

/// Faixa do arquivo mapeada em memória virtual.
#[derive(Debug, Clone)]
struct Region {
    va: u64,
    size: u64,
    off: usize,
    exec: bool,
}

/// Como uma instrução afeta o fluxo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flow {
    Normal,
    Call(Option<u64>),
    Jump(Option<u64>),
    CondJump(Option<u64>),
    Return,
    Stop,
}

struct Func {
    start: u64,
    name: String,
    /// (endereço, tamanho) de cada instrução, em ordem de endereço.
    insns: Vec<(u64, u8)>,
    blocks: BTreeSet<u64>,
    calls: Vec<u64>,
    /// Endereços de código citados como dado (ex.: `lea rdi, [main]`): viram novas funções.
    code_refs: Vec<u64>,
    disasm: OnceLock<Vec<CodeLine>>,
    pseudo: OnceLock<Vec<CodeLine>>,
    hex: OnceLock<Vec<CodeLine>>,
}

pub struct NativeEngine {
    data: Vec<u8>,
    regions: Vec<Region>,
    cpu: Option<CpuArch>,
    funcs: Vec<Func>,
    by_addr: HashMap<u64, usize>,
    /// Nomes de destinos que não são funções analisadas (imports, stubs PLT, slots IAT).
    extern_names: HashMap<u64, String>,
    strings: Vec<StringInfo>,
    string_at: HashMap<u64, String>,
    imports: Vec<ImportInfo>,
    info: Vec<FunctionInfo>,
    partial: bool,
}

impl NativeEngine {
    /// Analisa os bytes de um arquivo já validado.
    pub fn analyze(data: Vec<u8>, progress: &mut dyn FnMut(&str)) -> Self {
        let mut eng = NativeEngine {
            data,
            regions: vec![],
            cpu: None,
            funcs: vec![],
            by_addr: HashMap::new(),
            extern_names: HashMap::new(),
            strings: vec![],
            string_at: HashMap::new(),
            imports: vec![],
            info: vec![],
            partial: false,
        };

        progress("Lendo cabeçalhos e símbolos");
        let mut seeds: BTreeMap<u64, String> = BTreeMap::new();
        eng.load_object(&mut seeds);

        progress("Extraindo strings");
        eng.scan_strings();

        if eng.cpu.is_some() {
            progress("Identificando funções");
            eng.discover(seeds);
            progress("Montando referências cruzadas");
            eng.build_info();
        }
        eng
    }

    // ---------------------------------------------------------------- formato

    fn load_object(&mut self, seeds: &mut BTreeMap<u64, String>) {
        let data = std::mem::take(&mut self.data);
        match Object::parse(&data) {
            Ok(Object::Elf(elf)) => self.load_elf(&elf, seeds),
            Ok(Object::PE(pe)) => self.load_pe(&pe, seeds),
            Ok(Object::Mach(Mach::Binary(m))) => self.load_macho(&m, seeds),
            _ => {}
        }
        // Formato desconhecido: o arquivo inteiro vira uma região de dados (para as strings).
        if self.regions.is_empty() {
            self.regions.push(Region { va: 0, size: data.len() as u64, off: 0, exec: false });
        }
        self.data = data;
    }

    fn load_elf(&mut self, elf: &goblin::elf::Elf, seeds: &mut BTreeMap<u64, String>) {
        use goblin::elf::header::*;
        self.cpu = match elf.header.e_machine {
            EM_386 => Some(CpuArch::X86),
            EM_X86_64 => Some(CpuArch::X64),
            EM_ARM => Some(if elf.entry & 1 == 1 { CpuArch::Thumb } else { CpuArch::Arm }),
            EM_AARCH64 => Some(CpuArch::Arm64),
            EM_MIPS if !elf.is_64 => {
                Some(if elf.little_endian { CpuArch::MipsLe } else { CpuArch::MipsBe })
            }
            _ => None,
        };
        for ph in &elf.program_headers {
            if ph.p_type == goblin::elf::program_header::PT_LOAD && ph.p_filesz > 0 {
                self.regions.push(Region {
                    va: ph.p_vaddr,
                    size: ph.p_filesz,
                    off: ph.p_offset as usize,
                    exec: ph.p_flags & goblin::elf::program_header::PF_X != 0,
                });
            }
        }
        // Objetos relocáveis (.o) não têm segmentos: usa as seções.
        if self.regions.is_empty() {
            for sh in &elf.section_headers {
                if sh.sh_type != goblin::elf::section_header::SHT_NOBITS && sh.sh_size > 0 {
                    self.regions.push(Region {
                        va: sh.sh_addr,
                        size: sh.sh_size,
                        off: sh.sh_offset as usize,
                        exec: sh.sh_flags & u64::from(goblin::elf::section_header::SHF_EXECINSTR) != 0,
                    });
                }
            }
        }
        let thumb_fix = |v: u64| if self.cpu == Some(CpuArch::Thumb) { v & !1 } else { v };
        if elf.entry != 0 {
            seeds.insert(thumb_fix(elf.entry), "entry".into());
        }
        for (syms, strtab) in [(&elf.syms, &elf.strtab), (&elf.dynsyms, &elf.dynstrtab)] {
            for s in syms.iter() {
                if s.st_type() == goblin::elf::sym::STT_FUNC && s.st_value != 0 && s.st_shndx != 0 {
                    if let Some(name) = strtab.get_at(s.st_name).filter(|n| !n.is_empty()) {
                        seeds.insert(thumb_fix(s.st_value), name.to_string());
                    }
                }
            }
        }
        // Imports: símbolos dinâmicos indefinidos.
        let libs = elf.libraries.join(", ");
        for s in elf.dynsyms.iter() {
            if s.st_shndx == 0 {
                if let Some(name) = elf.dynstrtab.get_at(s.st_name).filter(|n| !n.is_empty()) {
                    self.imports.push(ImportInfo {
                        library: if libs.is_empty() { "dinâmico".into() } else { libs.clone() },
                        name: name.to_string(),
                    });
                }
            }
        }
        // Slots da GOT apontados por relocações: `call [rip + slot]` vira o nome do import.
        for rel in elf.dynrelas.iter().chain(elf.dynrels.iter()).chain(elf.pltrelocs.iter()) {
            if rel.r_sym != 0 {
                if let Some(name) = elf.dynsyms.get(rel.r_sym).and_then(|s| elf.dynstrtab.get_at(s.st_name)).filter(|n| !n.is_empty()) {
                    self.extern_names.insert(rel.r_offset, name.to_string());
                }
            }
        }
        // Stubs da PLT (x86/x86-64): cada entrada de 16 bytes chama um import.
        if self.cpu.is_some_and(CpuArch::is_x86) {
            let sec = |n: &str| {
                elf.section_headers
                    .iter()
                    .find(|sh| elf.shdr_strtab.get_at(sh.sh_name) == Some(n))
                    .map(|sh| sh.sh_addr)
            };
            let (base, skip) = match (sec(".plt.sec"), sec(".plt")) {
                (Some(b), _) => (Some(b), 0),
                (None, Some(b)) => (Some(b), 1),
                _ => (None, 0),
            };
            if let Some(base) = base {
                for (i, rel) in elf.pltrelocs.iter().enumerate() {
                    if let Some(name) = elf.dynsyms.get(rel.r_sym).and_then(|s| elf.dynstrtab.get_at(s.st_name)) {
                        let stub = base + 16 * (i as u64 + skip);
                        self.extern_names.insert(stub, format!("{name}@plt"));
                    }
                }
            }
        }
    }

    fn load_pe(&mut self, pe: &goblin::pe::PE, seeds: &mut BTreeMap<u64, String>) {
        self.cpu = match pe.header.coff_header.machine {
            0x014c => Some(CpuArch::X86),
            0x8664 => Some(CpuArch::X64),
            0x01c0 => Some(CpuArch::Arm),
            0x01c4 => Some(CpuArch::Thumb),
            0xaa64 => Some(CpuArch::Arm64),
            _ => None,
        };
        let base = pe.image_base;
        for s in &pe.sections {
            if s.size_of_raw_data > 0 {
                self.regions.push(Region {
                    va: base + u64::from(s.virtual_address),
                    size: u64::from(s.size_of_raw_data.min(s.virtual_size.max(s.size_of_raw_data))),
                    off: s.pointer_to_raw_data as usize,
                    exec: s.characteristics & 0x2000_0000 != 0,
                });
            }
        }
        if pe.entry != 0 {
            seeds.insert(base + u64::from(pe.entry), "entry".into());
        }
        for e in &pe.exports {
            if let (Some(name), Some(rva)) = (e.name, (e.rva != 0).then_some(e.rva)) {
                seeds.insert(base + rva as u64, name.to_string());
            }
        }
        for imp in &pe.imports {
            let name = imp.name.to_string();
            self.imports.push(ImportInfo { library: imp.dll.to_string(), name: name.clone() });
            // Slot da IAT (goblin: `offset` é o RVA do slot): `call [slot]` aponta para cá.
            self.extern_names.insert(base + imp.offset as u64, format!("{}!{}", imp.dll, name));
        }
    }

    fn load_macho(&mut self, m: &goblin::mach::MachO, seeds: &mut BTreeMap<u64, String>) {
        self.cpu = match m.header.cputype {
            0x7 => Some(CpuArch::X86),
            0x0100_0007 => Some(CpuArch::X64),
            0xc => Some(CpuArch::Arm),
            0x0100_000c => Some(CpuArch::Arm64),
            _ => None,
        };
        for seg in m.segments.iter() {
            if seg.filesize > 0 {
                self.regions.push(Region {
                    va: seg.vmaddr,
                    size: seg.filesize,
                    off: seg.fileoff as usize,
                    exec: seg.initprot & 0x4 != 0,
                });
            }
        }
        if m.entry != 0 {
            seeds.insert(m.entry, "entry".into());
        }
        if let Some(syms) = &m.symbols {
            for (name, nl) in syms.iter().flatten() {
                if nl.n_sect != 0 && nl.n_value != 0 && !name.is_empty() {
                    let addr = nl.n_value;
                    if self.region_of(addr).is_some_and(|r| r.exec) {
                        seeds.insert(addr, name.trim_start_matches('_').to_string());
                    }
                }
            }
        }
        if let Ok(imps) = m.imports() {
            for i in imps {
                self.imports.push(ImportInfo {
                    library: i.dylib.to_string(),
                    name: i.name.trim_start_matches('_').to_string(),
                });
            }
        }
    }

    // ---------------------------------------------------------------- memória

    fn region_of(&self, va: u64) -> Option<&Region> {
        self.regions.iter().find(|r| va >= r.va && va < r.va + r.size)
    }

    /// Bytes a partir de um endereço virtual, até o fim da região.
    fn bytes_at(&self, va: u64, max: usize) -> Option<&[u8]> {
        let r = self.region_of(va)?;
        let start = r.off + (va - r.va) as usize;
        let end = (r.off + r.size as usize).min(self.data.len()).min(start + max);
        (start < end).then(|| &self.data[start..end])
    }

    fn is_code(&self, va: u64) -> bool {
        self.region_of(va).is_some_and(|r| r.exec)
    }

    // ---------------------------------------------------------------- strings

    fn scan_strings(&mut self) {
        let mut out = Vec::new();
        let regions = self.regions.clone();
        'outer: for r in &regions {
            let end = (r.off + r.size as usize).min(self.data.len());
            if r.off >= end {
                continue;
            }
            let bytes = &self.data[r.off..end];
            let mut i = 0;
            while i < bytes.len() {
                // ASCII
                let start = i;
                while i < bytes.len() && is_printable(bytes[i]) {
                    i += 1;
                }
                if i - start >= MIN_STRING {
                    let s = String::from_utf8_lossy(&bytes[start..i]).into_owned();
                    out.push((r.va + start as u64, s));
                } else if i == start {
                    // UTF-16LE (comum em PE)
                    let mut j = start;
                    while j + 1 < bytes.len() && is_printable(bytes[j]) && bytes[j + 1] == 0 {
                        j += 2;
                    }
                    if (j - start) / 2 >= MIN_STRING {
                        let units: Vec<u16> = bytes[start..j].chunks(2).map(|c| u16::from(c[0])).collect();
                        out.push((r.va + start as u64, String::from_utf16_lossy(&units)));
                        i = j;
                        continue;
                    }
                    i += 1;
                }
                if out.len() >= MAX_STRINGS {
                    self.partial = true;
                    break 'outer;
                }
            }
        }
        out.sort_by_key(|(a, _)| *a);
        out.dedup_by_key(|(a, _)| *a);
        for (a, s) in out {
            self.string_at.insert(a, s.clone());
            self.strings.push(StringInfo { address: format_address(a), value: s });
        }
    }

    // ---------------------------------------------------------------- funções

    fn discover(&mut self, seeds: BTreeMap<u64, String>) {
        let Some(cpu) = self.cpu else { return };
        let Ok(cs) = cpu.capstone() else { return };
        let mut names = seeds;
        let mut queue: Vec<u64> = names.keys().copied().filter(|a| self.is_code(*a)).collect();
        let mut seen: BTreeSet<u64> = queue.iter().copied().collect();
        let mut total = 0usize;

        while let Some(start) = queue.pop() {
            if self.funcs.len() >= MAX_FUNCTIONS || total >= MAX_TOTAL_INSNS {
                self.partial = true;
                break;
            }
            if self.extern_names.contains_key(&start) {
                continue; // stub de import: não vira função
            }
            let f = self.trace(&cs, cpu, start, &names);
            total += f.insns.len();
            // Stub de import (`jmp [slot]`): vira nome, não função.
            if let Some(n) = self.stub_target(&cs, cpu, &f) {
                self.extern_names.insert(start, n);
                continue;
            }
            // glibc: o ponto de entrada passa `main` para __libc_start_main.
            if names.get(&start).map(String::as_str) == Some("entry")
                && self.extern_names.values().any(|n| n.starts_with("__libc_start_main"))
            {
                if let Some(&m) = f.code_refs.first() {
                    names.entry(m).or_insert_with(|| "main".to_string());
                }
            }
            for &c in f.calls.iter().chain(f.code_refs.iter()) {
                if self.is_code(c) && !self.extern_names.contains_key(&c) && seen.insert(c) {
                    queue.push(c);
                }
            }
            if f.insns.is_empty() {
                continue;
            }
            let name = names.remove(&start).unwrap_or_else(|| format!("sub_{start:x}"));
            self.by_addr.insert(start, self.funcs.len());
            self.funcs.push(Func { name, ..f });
        }
        // Preenchimento de lacunas (x86): código entre funções conhecidas que só é
        // alcançado por ponteiros (callbacks, vtables). Pula o preenchimento
        // (nop, int3, zeros) e começa uma função onde houver código válido.
        if cpu.is_x86() {
            for _round in 0..8 {
                let gaps = self.gap_starts(&cs);
                if gaps.is_empty() || self.funcs.len() >= MAX_FUNCTIONS || total >= MAX_TOTAL_INSNS {
                    break;
                }
                for g in gaps {
                    if self.funcs.len() >= MAX_FUNCTIONS || total >= MAX_TOTAL_INSNS {
                        self.partial = true;
                        break;
                    }
                    let f = self.trace(&cs, cpu, g, &names);
                    if f.insns.is_empty() {
                        continue;
                    }
                    if let Some(n) = self.stub_target(&cs, cpu, &f) {
                        self.extern_names.insert(g, n);
                        continue;
                    }
                    total += f.insns.len();
                    let name = names.remove(&g).unwrap_or_else(|| format!("sub_{g:x}"));
                    self.by_addr.insert(g, self.funcs.len());
                    self.funcs.push(Func { name, ..f });
                }
            }
        }
        // Ordem estável por endereço.
        self.funcs.sort_by_key(|f| f.start);
        self.by_addr = self.funcs.iter().enumerate().map(|(i, f)| (f.start, i)).collect();
    }

    /// Descida recursiva dentro de uma função: segue saltos, não entra em chamadas.
    fn trace(&self, cs: &Capstone, cpu: CpuArch, start: u64, names: &BTreeMap<u64, String>) -> Func {
        let mut insns: BTreeMap<u64, u8> = BTreeMap::new();
        let mut blocks = BTreeSet::from([start]);
        let mut calls = Vec::new();
        let mut code_refs = Vec::new();
        let mut work = vec![start];
        while let Some(mut pc) = work.pop() {
            loop {
                if insns.contains_key(&pc) || insns.len() >= MAX_INSNS_PER_FUNCTION {
                    break;
                }
                // Outra função conhecida começa aqui: fim deste caminho.
                if pc != start && names.contains_key(&pc) {
                    break;
                }
                let Some(code) = self.bytes_at(pc, 16) else { break };
                let Ok(dis) = cs.disasm_count(code, pc, 1) else { break };
                let Some(ins) = dis.iter().next() else { break };
                let len = ins.len() as u8;
                insns.insert(pc, len);
                let mn = ins.mnemonic().unwrap_or("");
                let op = ins.op_str().unwrap_or("");
                let flow = classify(cpu, mn, op);
                let next = pc + u64::from(len);
                if flow == Flow::Normal && (mn == "lea" || mn == "mov" || mn == "push" || mn == "adr" || mn == "adrp") {
                    let r = if cpu == CpuArch::X64 { rip_relative(op, next) } else { imm_target(op) };
                    if let Some(r) = r.filter(|r| self.is_code(*r)) {
                        code_refs.push(r);
                    }
                }
                match flow {
                    Flow::Normal => pc = next,
                    Flow::Call(t) => {
                        if let Some(t) = t {
                            calls.push(t);
                        }
                        pc = next;
                    }
                    Flow::CondJump(t) => {
                        if let Some(t) = t.filter(|t| self.is_code(*t)) {
                            blocks.insert(t);
                            work.push(t);
                        }
                        blocks.insert(next);
                        pc = next;
                    }
                    Flow::Jump(t) => {
                        if let Some(t) = t.filter(|t| self.is_code(*t)) {
                            if names.contains_key(&t) && t != start {
                                calls.push(t); // salto de cauda para outra função
                            } else {
                                blocks.insert(t);
                                work.push(t);
                            }
                        }
                        break;
                    }
                    Flow::Return | Flow::Stop => break,
                }
            }
        }
        // Blocos só valem onde há instrução.
        blocks.retain(|b| insns.contains_key(b));
        calls.sort_unstable();
        calls.dedup();
        Func {
            start,
            name: String::new(),
            insns: insns.into_iter().collect(),
            blocks,
            calls,
            code_refs,
            disasm: OnceLock::new(),
            pseudo: OnceLock::new(),
            hex: OnceLock::new(),
        }
    }


    /// Se a "função" só salta para um slot de import (PLT, thunk da IAT), devolve o nome.
    fn stub_target(&self, cs: &Capstone, cpu: CpuArch, f: &Func) -> Option<String> {
        if !cpu.is_x86() || f.insns.len() > 3 {
            return None;
        }
        for &(addr, len) in &f.insns {
            let code = self.bytes_at(addr, len as usize)?;
            let dis = cs.disasm_count(code, addr, 1).ok()?;
            let ins = dis.iter().next()?;
            if ins.mnemonic() != Some("jmp") {
                continue;
            }
            let op = ins.op_str().unwrap_or("");
            let slot = if cpu == CpuArch::X64 {
                rip_relative(op, addr + u64::from(len))
            } else {
                op.split(['[', ']']).nth(1).and_then(parse_num)
            };
            let name = slot.and_then(|a| self.extern_names.get(&a))?;
            return Some(if name.ends_with("@plt") || name.contains('!') { name.clone() } else { format!("{name}@plt") });
        }
        None
    }

    /// Inícios prováveis de função nas lacunas entre funções já encontradas.
    fn gap_starts(&self, cs: &Capstone) -> Vec<u64> {
        // Trechos contíguos de instruções (uma função pode ter blocos espalhados).
        let mut extents: Vec<(u64, u64)> = Vec::new();
        for f in &self.funcs {
            let mut run: Option<(u64, u64)> = None;
            for &(a, l) in &f.insns {
                let e = a + u64::from(l);
                run = match run {
                    Some((s0, e0)) if a <= e0 + 16 => Some((s0, e0.max(e))),
                    Some(r) => {
                        extents.push(r);
                        Some((a, e))
                    }
                    None => Some((a, e)),
                };
            }
            extents.extend(run);
        }
        extents.sort_unstable();
        let mut out = Vec::new();
        for r in self.regions.iter().filter(|r| r.exec) {
            let mut cursor = r.va;
            let end = r.va + r.size;
            let mut iter = extents.iter().filter(|(s, e)| *e > r.va && *s < end).peekable();
            loop {
                let next_start = iter.peek().map_or(end, |(s, _)| *s).min(end);
                if next_start > cursor {
                    if let Some(g) = self.skip_padding(cs, cursor, next_start) {
                        if !self.by_addr.contains_key(&g) && !self.extern_names.contains_key(&g) {
                            out.push(g);
                        }
                    }
                }
                match iter.next() {
                    Some((_, e)) => cursor = cursor.max(*e),
                    None => break,
                }
            }
        }
        out
    }

    /// Avança sobre preenchimento; devolve o primeiro endereço com código real.
    fn skip_padding(&self, cs: &Capstone, mut pc: u64, limit: u64) -> Option<u64> {
        while pc < limit {
            let code = self.bytes_at(pc, (limit - pc).min(16) as usize)?;
            if code[0] == 0 || code[0] == 0xcc {
                pc += 1;
                continue;
            }
            let dis = cs.disasm_count(code, pc, 1).ok()?;
            let ins = dis.iter().next()?;
            let m = ins.mnemonic().unwrap_or("");
            if m == "nop" || m == "int3" {
                pc += ins.len() as u64;
                continue;
            }
            return Some(pc);
        }
        None
    }

    fn build_info(&mut self) {
        let mut xin: HashMap<u64, Vec<u64>> = HashMap::new();
        for f in &self.funcs {
            for c in &f.calls {
                if self.by_addr.contains_key(c) {
                    xin.entry(*c).or_default().push(f.start);
                }
            }
        }
        self.info = self
            .funcs
            .iter()
            .map(|f| {
                FunctionInfo {
                    address: format_address(f.start),
                    name: f.name.clone(),
                    // Soma das instruções: blocos podem estar espalhados pelo binário.
                    size_bytes: f.insns.iter().map(|(_, l)| u64::from(*l)).sum(),
                    block_count: f.blocks.len() as u32,
                    xrefs_in: xin.get(&f.start).map(|v| v.iter().map(|a| format_address(*a)).collect()).unwrap_or_default(),
                    xrefs_out: f
                        .calls
                        .iter()
                        .filter(|c| self.by_addr.contains_key(c))
                        .map(|a| format_address(*a))
                        .collect(),
                }
            })
            .collect();
    }

    // ---------------------------------------------------------------- linhas

    fn name_of(&self, addr: u64) -> Option<String> {
        if let Some(&i) = self.by_addr.get(&addr) {
            return Some(self.funcs[i].name.clone());
        }
        self.extern_names.get(&addr).cloned()
    }

    /// Comentário útil: nome do destino, import via IAT ou string referenciada.
    fn annotate(&self, cpu: CpuArch, addr: u64, len: u8, op: &str, target: Option<u64>) -> Option<String> {
        if let Some(t) = target {
            return self.name_of(t);
        }
        if cpu == CpuArch::X64 {
            if let Some(ea) = rip_relative(op, addr + u64::from(len)) {
                if let Some(n) = self.extern_names.get(&ea) {
                    return Some(n.clone());
                }
                if let Some(s) = self.string_at.get(&ea) {
                    return Some(format!("\"{}\"", truncate(s, 60)));
                }
            }
        }
        // Endereço absoluto que aponta para uma string (comum em x86 32 bits e ARM).
        for tok in op.split([',', '[', ']', ' ']) {
            if let Some(v) = parse_num(tok) {
                if let Some(s) = self.string_at.get(&v) {
                    return Some(format!("\"{}\"", truncate(s, 60)));
                }
                if let Some(n) = self.extern_names.get(&v) {
                    return Some(n.clone());
                }
            }
        }
        None
    }

    fn decode(&self, f: &Func) -> Vec<Decoded> {
        let Some(cpu) = self.cpu else { return vec![] };
        let Ok(cs) = cpu.capstone() else { return vec![] };
        f.insns
            .iter()
            .filter_map(|&(addr, len)| {
                let code = self.bytes_at(addr, len as usize)?;
                let dis = cs.disasm_count(code, addr, 1).ok()?;
                let ins = dis.iter().next()?;
                let mn = ins.mnemonic().unwrap_or("").to_string();
                let op = ins.op_str().unwrap_or("").to_string();
                let flow = classify(cpu, &mn, &op);
                let target = match flow {
                    Flow::Call(t) | Flow::Jump(t) | Flow::CondJump(t) => t,
                    _ => None,
                };
                Some(Decoded {
                    addr,
                    bytes: code.to_vec(),
                    comment: self.annotate(cpu, addr, len, &op, target),
                    mnemonic: mn,
                    op,
                    flow,
                    target,
                })
            })
            .collect()
    }

    fn disasm_lines(&self, f: &Func) -> Vec<CodeLine> {
        self.decode(f)
            .into_iter()
            .map(|d| CodeLine {
                address: format_address(d.addr),
                bytes: Some(hex_bytes(&d.bytes)),
                text: if d.op.is_empty() { d.mnemonic.clone() } else { format!("{} {}", d.mnemonic, d.op) },
                comment: d.comment,
                target: d.target.map(format_address),
            })
            .collect()
    }

    fn hex_lines(&self, f: &Func) -> Vec<CodeLine> {
        let end = f.insns.last().map_or(f.start, |(a, l)| a + u64::from(*l));
        let size = (end - f.start) as usize;
        let bytes = self.bytes_at(f.start, size).unwrap_or(&[]);
        bytes
            .chunks(16)
            .enumerate()
            .map(|(n, chunk)| CodeLine {
                address: format_address(f.start + n as u64 * 16),
                bytes: Some(hex_bytes(chunk)),
                text: chunk.iter().map(|&b| if is_printable(b) { b as char } else { '.' }).collect(),
                comment: None,
                target: None,
            })
            .collect()
    }

    fn pseudo_lines(&self, f: &Func) -> Vec<CodeLine> {
        let cpu = self.cpu.unwrap_or(CpuArch::X64);
        let decoded = self.decode(f);
        let mut out = Vec::with_capacity(decoded.len() + 4);
        let line = |addr: u64, text: String| CodeLine {
            address: format_address(addr),
            bytes: None,
            text,
            comment: None,
            target: None,
        };
        out.push(line(f.start, "// Pseudocódigo simplificado (tradução instrução a instrução, não é decompilação)".into()));
        out.push(line(f.start, format!("void {}(void) {{", f.name)));
        let mut last_cmp: Option<(String, String)> = None;
        for d in &decoded {
            if d.addr != f.start && f.blocks.contains(&d.addr) {
                out.push(line(d.addr, format!("loc_{:x}:", d.addr)));
            }
            if matches!(d.mnemonic.as_str(), "nop" | "endbr64" | "endbr32") {
                continue;
            }
            let stmt = if cpu.is_x86() {
                // Endereços relativos ao RIP viram absolutos: mais legível.
                let mut d2 = Decoded { op: d.op.clone(), ..d.clone() };
                if cpu == CpuArch::X64 {
                    d2.op = resolve_rip(&d.op, d.addr + d.bytes.len() as u64);
                }
                lift_x86(&d2, &mut last_cmp, |a| self.name_of(a))
            } else {
                lift_generic(d, |a| self.name_of(a))
            };
            // Comentário só quando acrescenta algo (o nome já pode estar na linha).
            let comment = d.comment.clone().filter(|c| !stmt.contains(c.as_str()));
            let mut l = line(d.addr, format!("    {stmt}"));
            l.target = d.target.map(format_address);
            l.comment = comment;
            out.push(l);
        }
        let end = f.insns.last().map_or(f.start, |(a, _)| *a);
        out.push(line(end, "}".into()));
        out
    }
}

#[derive(Clone)]
struct Decoded {
    addr: u64,
    bytes: Vec<u8>,
    mnemonic: String,
    op: String,
    flow: Flow,
    target: Option<u64>,
    comment: Option<String>,
}

impl Engine for NativeEngine {
    fn name(&self) -> &'static str {
        "Nativo (capstone)"
    }

    fn is_demo(&self) -> bool {
        false
    }

    fn list_functions(&self) -> Vec<FunctionInfo> {
        self.info.clone()
    }

    fn lines(&self, function_address: u64, kind: CodeKind) -> Result<&[CodeLine], AppError> {
        let &i = self
            .by_addr
            .get(&function_address)
            .ok_or_else(|| AppError::FunctionNotFound(format_address(function_address)))?;
        let f = &self.funcs[i];
        Ok(match kind {
            CodeKind::Disasm => f.disasm.get_or_init(|| self.disasm_lines(f)),
            CodeKind::Pseudo => f.pseudo.get_or_init(|| self.pseudo_lines(f)),
            CodeKind::Hex => f.hex.get_or_init(|| self.hex_lines(f)),
        })
    }

    fn list_strings(&self) -> Vec<StringInfo> {
        self.strings.clone()
    }

    fn list_imports(&self) -> Vec<ImportInfo> {
        self.imports.clone()
    }

    fn function_addresses(&self) -> Vec<u64> {
        self.funcs.iter().map(|f| f.start).collect()
    }

    fn function_name(&self, address: u64) -> Option<String> {
        self.by_addr.get(&address).map(|&i| self.funcs[i].name.clone())
    }

    fn is_partial(&self) -> bool {
        self.partial
    }

    fn has_code(&self) -> bool {
        self.cpu.is_some()
    }
}

// -------------------------------------------------------------------- auxiliares

fn is_printable(b: u8) -> bool {
    (0x20..0x7f).contains(&b) || b == b'\t'
}

fn hex_bytes(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

/// "0x1234", "#0x1234", "1234" (decimal) ou "#-8".
fn parse_num(tok: &str) -> Option<u64> {
    let t = tok.trim().trim_start_matches('#').trim_start_matches('$');
    if let Some(h) = t.strip_prefix("0x") {
        u64::from_str_radix(h, 16).ok()
    } else if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) && t.len() > 3 {
        t.parse().ok()
    } else {
        None
    }
}

/// Endereço de `[rip + 0x..]` / `[rip - 0x..]`.
fn rip_relative(op: &str, next: u64) -> Option<u64> {
    let i = op.find("[rip ")?;
    let rest = &op[i + 5..];
    let (sign, rest) = match rest.strip_prefix("+ ") {
        Some(r) => (1i128, r),
        None => (-1i128, rest.strip_prefix("- ")?),
    };
    let num = rest.split(']').next()?;
    let v = num.strip_prefix("0x").and_then(|h| u64::from_str_radix(h, 16).ok())?;
    Some((next as i128 + sign * v as i128) as u64)
}

/// Troca `[rip + 0x..]` pelo endereço absoluto `[0x..]`.
fn resolve_rip(op: &str, next: u64) -> String {
    match (rip_relative(op, next), op.find("[rip "), op.find(']')) {
        (Some(ea), Some(i), Some(j)) if j > i => format!("{}[0x{ea:x}]{}", &op[..i], &op[j + 1..]),
        _ => op.to_string(),
    }
}

/// Destino imediato: o último operando, se for número.
fn imm_target(op: &str) -> Option<u64> {
    let last = op.rsplit(',').next()?.trim();
    let t = last.trim_start_matches('#');
    t.strip_prefix("0x").and_then(|h| u64::from_str_radix(h, 16).ok())
}

fn classify(cpu: CpuArch, mn: &str, op: &str) -> Flow {
    let m = mn.to_ascii_lowercase();
    let target = imm_target(op);
    match cpu {
        CpuArch::X86 | CpuArch::X64 => match m.as_str() {
            "ret" | "retn" | "retf" | "iret" | "iretd" | "iretq" => Flow::Return,
            "hlt" | "ud2" | "int3" => Flow::Stop,
            "call" => Flow::Call(target),
            "jmp" => Flow::Jump(target),
            _ if m.starts_with('j') || m.starts_with("loop") => Flow::CondJump(target),
            _ => Flow::Normal,
        },
        CpuArch::Arm64 => match m.as_str() {
            "ret" | "eret" => Flow::Return,
            "br" => Flow::Stop,
            "bl" | "blr" => Flow::Call(target),
            "b" => Flow::Jump(target),
            _ if m.starts_with("b.") || m.starts_with("cb") || m.starts_with("tb") => Flow::CondJump(target),
            _ => Flow::Normal,
        },
        CpuArch::Arm | CpuArch::Thumb => {
            if (m == "bx" && op.trim() == "lr") || (m.starts_with("pop") && op.contains("pc")) {
                Flow::Return
            } else if m == "bl" || m == "blx" {
                Flow::Call(target)
            } else if m == "b" || m == "b.w" {
                Flow::Jump(target)
            } else if m == "bx" {
                Flow::Stop
            } else if m.starts_with("cb")
                || (m.starts_with('b') && m.len() <= 5 && !["bic", "bfc", "bfi", "bkpt"].iter().any(|p| m.starts_with(p)))
            {
                Flow::CondJump(target)
            } else {
                Flow::Normal
            }
        }
        CpuArch::MipsLe | CpuArch::MipsBe => {
            if m == "jr" && op.contains("ra") {
                Flow::Return
            } else if m == "jal" || m == "jalr" || m == "bal" {
                Flow::Call(target)
            } else if m == "j" || m == "b" {
                Flow::Jump(target)
            } else if m == "jr" {
                Flow::Stop
            } else if m.starts_with('b') {
                Flow::CondJump(target)
            } else {
                Flow::Normal
            }
        }
    }
}

/// Tradução aproximada de x86 (sintaxe Intel) para C.
fn lift_x86(d: &Decoded, last_cmp: &mut Option<(String, String)>, name_of: impl Fn(u64) -> Option<String>) -> String {
    let ops: Vec<String> = split_ops(&d.op);
    let a = ops.first().cloned().unwrap_or_default();
    let b = ops.get(1).cloned().unwrap_or_default();
    let tgt = |t: Option<u64>| {
        t.map(|t| name_of(t).unwrap_or_else(|| format!("loc_{t:x}"))).unwrap_or_else(|| deref(&a))
    };
    let m = d.mnemonic.as_str();
    match m {
        "mov" | "movabs" | "movzx" | "movsx" | "movsxd" | "movss" | "movsd" | "movaps" | "movups" | "movq" | "movd" => {
            format!("{} = {};", deref(&a), deref(&b))
        }
        "lea" => format!("{} = {};", deref(&a), b.trim_start_matches('[').trim_end_matches(']').trim()),
        "add" => format!("{} += {};", deref(&a), deref(&b)),
        "sub" => format!("{} -= {};", deref(&a), deref(&b)),
        "and" => format!("{} &= {};", deref(&a), deref(&b)),
        "or" => format!("{} |= {};", deref(&a), deref(&b)),
        "xor" if a == b => format!("{} = 0;", deref(&a)),
        "xor" => format!("{} ^= {};", deref(&a), deref(&b)),
        "shl" | "sal" => format!("{} <<= {};", deref(&a), deref(&b)),
        "shr" | "sar" => format!("{} >>= {};", deref(&a), deref(&b)),
        "imul" if ops.len() == 3 => format!("{} = {} * {};", deref(&a), deref(&b), deref(&ops[2])),
        "imul" | "mul" if ops.len() == 2 => format!("{} *= {};", deref(&a), deref(&b)),
        "inc" => format!("{}++;", deref(&a)),
        "dec" => format!("{}--;", deref(&a)),
        "neg" => format!("{0} = -{0};", deref(&a)),
        "not" => format!("{0} = ~{0};", deref(&a)),
        "push" => format!("push({});", deref(&a)),
        "pop" => format!("{} = pop();", deref(&a)),
        "cmp" => {
            *last_cmp = Some((deref(&a), deref(&b)));
            format!("/* compara {} com {} */", deref(&a), deref(&b))
        }
        "test" => {
            let cond = if a == b { (deref(&a), "0".to_string()) } else { (format!("({} & {})", deref(&a), deref(&b)), "0".into()) };
            *last_cmp = Some(cond);
            format!("/* testa {} */", deref(&a))
        }
        "call" => match (d.target, &d.comment) {
            (Some(_), _) => format!("{}();", tgt(d.target)),
            (None, Some(c)) if !c.starts_with('"') => format!("{c}();"),
            (None, _) => format!("(*{})();", deref(&a)),
        },
        "jmp" => match d.target {
            Some(_) => format!("goto {};", tgt(d.target)),
            None => format!("goto *{};", deref(&a)),
        },
        "ret" | "retn" => "return;".into(),
        "nop" | "endbr64" | "endbr32" => "/* nop */".into(),
        "leave" => "/* restaura a pilha (leave) */".into(),
        _ if m.starts_with('j') => {
            let (l, r) = last_cmp.clone().unwrap_or_else(|| ("flags".into(), "0".into()));
            let op = match &m[1..] {
                "e" | "z" => "==",
                "ne" | "nz" => "!=",
                "l" | "nge" | "b" | "nae" | "c" => "<",
                "le" | "ng" | "be" | "na" => "<=",
                "g" | "nle" | "a" | "nbe" => ">",
                "ge" | "nl" | "ae" | "nb" | "nc" => ">=",
                "s" => "< 0 /* sinal */ &&",
                _ => "?",
            };
            if op == "?" {
                format!("if ({m}) goto {};", tgt(d.target))
            } else {
                format!("if ({l} {op} {r}) goto {};", tgt(d.target))
            }
        }
        _ => {
            if d.op.is_empty() {
                format!("__asm__(\"{m}\");")
            } else {
                format!("__asm__(\"{m} {}\");", d.op)
            }
        }
    }
}

fn lift_generic(d: &Decoded, name_of: impl Fn(u64) -> Option<String>) -> String {
    match d.flow {
        Flow::Return => "return;".into(),
        Flow::Call(Some(t)) => format!("{}();", name_of(t).unwrap_or_else(|| format!("sub_{t:x}"))),
        Flow::Jump(Some(t)) => format!("goto loc_{t:x};"),
        Flow::CondJump(Some(t)) => format!("if ({}) goto loc_{t:x};", d.mnemonic),
        _ => {
            if d.op.is_empty() {
                format!("__asm__(\"{}\");", d.mnemonic)
            } else {
                format!("__asm__(\"{} {}\");", d.mnemonic, d.op)
            }
        }
    }
}

/// Divide operandos por vírgula fora de colchetes.
fn split_ops(op: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for c in op.chars() {
        match c {
            '[' => {
                depth += 1;
                cur.push(c);
            }
            ']' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// "qword ptr [rbp - 0x10]" vira "*(u64*)(rbp - 0x10)".
fn deref(op: &str) -> String {
    let op = op.trim();
    let Some(i) = op.find('[') else { return op.to_string() };
    let ty = if op.starts_with("byte") {
        "u8"
    } else if op.starts_with("word") {
        "u16"
    } else if op.starts_with("dword") {
        "u32"
    } else if op.starts_with("qword") {
        "u64"
    } else {
        ""
    };
    let inner = op[i + 1..].trim_end_matches(']');
    // Segmento (fs:/gs:) faz parte do endereço: preserva.
    let seg = ["fs:", "gs:", "es:", "ds:", "ss:", "cs:"].iter().find(|s| op[..i].ends_with(*s)).copied().unwrap_or("");
    let inner = &format!("{seg}{inner}");
    if ty.is_empty() {
        format!("*({inner})")
    } else {
        format!("*({ty}*)({inner})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn self_binary() -> NativeEngine {
        let exe = std::env::current_exe().unwrap();
        let data = std::fs::read(exe).unwrap();
        NativeEngine::analyze(data, &mut |_| {})
    }

    #[test]
    fn analyzes_its_own_test_binary() {
        let e = self_binary();
        assert!(e.has_code(), "arquitetura do binário de teste deveria ser suportada");
        let fns = e.list_functions();
        assert!(fns.len() > 50, "poucas funções encontradas: {}", fns.len());
        let entry = fns.iter().find(|f| f.name == "entry").or_else(|| fns.first()).unwrap();
        let addr = crate::engine::parse_address(&entry.address).unwrap();
        let dis = e.lines(addr, CodeKind::Disasm).unwrap();
        assert!(!dis.is_empty());
        assert!(dis.iter().all(|l| l.bytes.as_deref().is_some_and(|b| !b.is_empty())));
        let pseudo = e.lines(addr, CodeKind::Pseudo).unwrap();
        assert!(pseudo.len() >= dis.len());
        assert!(!e.list_strings().is_empty());
    }

    #[test]
    fn xrefs_are_consistent() {
        let e = self_binary();
        let fns = e.list_functions();
        let by: HashMap<&str, &FunctionInfo> = fns.iter().map(|f| (f.address.as_str(), f)).collect();
        for f in &fns {
            for out in &f.xrefs_out {
                let callee = by.get(out.as_str()).expect("xref de saída aponta para função conhecida");
                assert!(callee.xrefs_in.contains(&f.address));
            }
        }
    }

    #[test]
    fn x86_flow_classification() {
        assert_eq!(classify(CpuArch::X64, "call", "0x401000"), Flow::Call(Some(0x401000)));
        assert_eq!(classify(CpuArch::X64, "jne", "0x10"), Flow::CondJump(Some(0x10)));
        assert_eq!(classify(CpuArch::X64, "jmp", "rax"), Flow::Jump(None));
        assert_eq!(classify(CpuArch::X64, "ret", ""), Flow::Return);
        assert_eq!(classify(CpuArch::Arm64, "b.eq", "#0x400100"), Flow::CondJump(Some(0x400100)));
        assert_eq!(classify(CpuArch::Arm64, "bl", "#0x400200"), Flow::Call(Some(0x400200)));
    }

    #[test]
    fn rip_relative_address() {
        assert_eq!(rip_relative("rax, qword ptr [rip + 0x10]", 0x1000), Some(0x1010));
        assert_eq!(rip_relative("rcx, [rip - 0x8]", 0x1000), Some(0xff8));
        assert_eq!(rip_relative("rax, rbx", 0x1000), None);
    }

    #[test]
    fn deref_and_split() {
        assert_eq!(split_ops("rax, qword ptr [rbp - 0x10]"), vec!["rax", "qword ptr [rbp - 0x10]"]);
        assert_eq!(deref("qword ptr [rbp - 0x10]"), "*(u64*)(rbp - 0x10)");
        assert_eq!(deref("eax"), "eax");
    }

    #[test]
    fn raw_data_has_no_functions_but_does_not_panic() {
        let e = NativeEngine::analyze(b"apenas texto qualquer, nada de codigo".to_vec(), &mut |_| {});
        assert!(!e.has_code());
        assert!(e.list_functions().is_empty());
        assert_eq!(e.list_strings().len(), 1);
    }
}
