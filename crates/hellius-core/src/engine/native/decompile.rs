//! Decompilador x86 / x86-64 → pseudocódigo C.
//!
//! Etapas:
//! 1. decodifica cada instrução com operandos estruturados (capstone, modo detalhe);
//! 2. monta o grafo de blocos (CFG) da função;
//! 3. execução simbólica por bloco: registradores guardam *expressões*, não
//!    valores. Chamadas recebem os argumentos pela convenção (Win64, System V,
//!    stdcall/cdecl 32 bits). Pilha vira variáveis (`param_N`, `local_XX`);
//! 4. fluxo de dados entre blocos até estabilizar (valores que chegam
//!    diferentes viram variáveis);
//! 5. estruturação: `if/else`, `while`, `do/while`, `switch` (cadeia de
//!    comparações ou tabela de saltos), `goto` só quando não há forma estruturada;
//! 6. limpeza (chamadas usadas uma vez entram na expressão, cópias mortas saem)
//!    e impressão, com nomes de constantes e structs Win32 quando conhecidos.
//!
//! O resultado é uma *reconstrução*. Nomes de variáveis locais, comentários,
//! macros e tipos exatos do código-fonte não existem no binário.

use std::collections::{BTreeMap, HashMap, HashSet};

use capstone::arch::x86::X86OperandType;
use capstone::arch::ArchOperand;
use capstone::prelude::*;

use super::winapi::{self, Api};
use super::{CpuArch, Func, NativeEngine};
use crate::engine::{format_address, CodeLine};

const MAX_INSNS: usize = 25_000;
const MAX_BLOCKS: usize = 4_000;
const MAX_ITER: usize = 12;
/// Acima disso uma expressão em registrador vira variável temporária.
const MAX_EXPR: usize = 24;

/// Convenção de chamada do binário.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abi {
    Win64,
    SysV64,
    X86,
}

// ===================================================================== registradores

type Reg = usize;
const RAX: Reg = 0;
const RCX: Reg = 1;
const RDX: Reg = 2;
const RBX: Reg = 3;
const RSP: Reg = 4;
const RBP: Reg = 5;
const RSI: Reg = 6;
const RDI: Reg = 7;
const R8: Reg = 8;
const R9: Reg = 9;
const R10: Reg = 10;
const R11: Reg = 11;
const XMM0: Reg = 16;
const NREG: usize = 48;

const GPR64: [&str; 16] = ["rax", "rcx", "rdx", "rbx", "rsp", "rbp", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15"];
const GPR32: [&str; 16] = ["eax", "ecx", "edx", "ebx", "esp", "ebp", "esi", "edi", "r8d", "r9d", "r10d", "r11d", "r12d", "r13d", "r14d", "r15d"];
const GPR16: [&str; 16] = ["ax", "cx", "dx", "bx", "sp", "bp", "si", "di", "r8w", "r9w", "r10w", "r11w", "r12w", "r13w", "r14w", "r15w"];
const GPR8: [&str; 16] = ["al", "cl", "dl", "bl", "spl", "bpl", "sil", "dil", "r8b", "r9b", "r10b", "r11b", "r12b", "r13b", "r14b", "r15b"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegRef {
    r: Reg,
    size: u8,
    high: bool,
}

fn parse_reg(n: &str) -> Option<RegRef> {
    let rr = |r, size| Some(RegRef { r, size, high: false });
    if let Some(i) = GPR64.iter().position(|x| *x == n) {
        return rr(i, 8);
    }
    if let Some(i) = GPR32.iter().position(|x| *x == n) {
        return rr(i, 4);
    }
    if let Some(i) = GPR16.iter().position(|x| *x == n) {
        return rr(i, 2);
    }
    if let Some(i) = GPR8.iter().position(|x| *x == n) {
        return rr(i, 1);
    }
    if let Some(i) = ["ah", "ch", "dh", "bh"].iter().position(|x| *x == n) {
        return Some(RegRef { r: i, size: 1, high: true });
    }
    for p in ["xmm", "ymm", "zmm"] {
        if let Some(k) = n.strip_prefix(p).and_then(|d| d.parse::<usize>().ok()) {
            if k < 32 {
                return Some(RegRef { r: XMM0 + k, size: 16, high: false });
            }
        }
    }
    None
}

fn reg_label(r: Reg) -> String {
    if r < 16 {
        GPR64[r].to_string()
    } else {
        format!("xmm{}", r - XMM0)
    }
}

// ===================================================================== instruções

#[derive(Debug, Clone)]
struct Mem {
    size: u8,
    seg: Option<String>,
    base: Option<RegRef>,
    rip: bool,
    index: Option<RegRef>,
    scale: i64,
    disp: i64,
}

#[derive(Debug, Clone)]
enum Opnd {
    R(RegRef),
    I(i64),
    M(Mem),
    Other(String),
}

#[derive(Debug, Clone)]
struct Ins {
    addr: u64,
    next: u64,
    mn: String,
    ops: Vec<Opnd>,
    text: String,
}

// ===================================================================== expressões

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Bo {
    Add,
    Mul,
    Div,
    UDiv,
    Mod,
    UMod,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Sar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Uo {
    Neg,
    Not,
    LNot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Co {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    ULt,
    ULe,
    UGt,
    UGe,
}

impl Co {
    fn neg(self) -> Co {
        match self {
            Co::Eq => Co::Ne,
            Co::Ne => Co::Eq,
            Co::Lt => Co::Ge,
            Co::Ge => Co::Lt,
            Co::Le => Co::Gt,
            Co::Gt => Co::Le,
            Co::ULt => Co::UGe,
            Co::UGe => Co::ULt,
            Co::ULe => Co::UGt,
            Co::UGt => Co::ULe,
        }
    }
    fn sym(self) -> &'static str {
        match self {
            Co::Eq => "==",
            Co::Ne => "!=",
            Co::Lt | Co::ULt => "<",
            Co::Le | Co::ULe => "<=",
            Co::Gt | Co::UGt => ">",
            Co::Ge | Co::UGe => ">=",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CallE {
    addr: u64,
    target: E,
    /// Nome bruto do destino (`USER32.dll!MessageBoxA`, `sub_401000`).
    name: Option<String>,
    args: Vec<E>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum E {
    C(i64),
    /// Endereço de uma string.
    Str(u64),
    /// Função ou import (como valor: ponteiro de função).
    Sym(String),
    /// Endereço do slot da IAT (carregar dele dá a função).
    Imp(String),
    /// Endereço de dado global.
    Glob(u64),
    /// Endereço na pilha (deslocamento a partir do `rsp` na entrada).
    Sp(i64),
    V(u32),
    /// Retorno da chamada feita no endereço.
    Res(u64),
    /// Valor de entrada de um registrador preservado (callee-saved).
    Saved(Reg),
    Ld(Box<E>, u8),
    Bin(Bo, Box<E>, Box<E>),
    Un(Uo, Box<E>),
    Cast(u8, bool, Box<E>),
    Cmp(Co, Box<E>, Box<E>),
    Tern(Box<E>, Box<E>, Box<E>),
    Call(Box<CallE>),
    Raw(String),
}

fn bx(e: E) -> Box<E> {
    Box::new(e)
}

fn bin(op: Bo, a: E, b: E) -> E {
    use E::*;
    match (op, &a, &b) {
        (Bo::Add, C(x), C(y)) => C(x.wrapping_add(*y)),
        (Bo::Mul, C(x), C(y)) => C(x.wrapping_mul(*y)),
        (Bo::And, C(x), C(y)) => C(x & y),
        (Bo::Or, C(x), C(y)) => C(x | y),
        (Bo::Xor, C(x), C(y)) => C(x ^ y),
        (Bo::Shl, C(x), C(y)) if (0..64).contains(y) => C(x.wrapping_shl(*y as u32)),
        (Bo::Add, Sp(o), C(k)) => Sp(o + k),
        (Bo::Add, C(k), Sp(o)) => Sp(o + k),
        (Bo::Add, Glob(g), C(k)) | (Bo::Add, C(k), Glob(g)) => Glob(g.wrapping_add(*k as u64)),
        (Bo::Add, Str(g), C(k)) if *k != 0 => Glob(g.wrapping_add(*k as u64)),
        (Bo::Add, _, C(0)) | (Bo::Or, _, C(0)) | (Bo::Xor, _, C(0)) | (Bo::Shl, _, C(0)) | (Bo::Shr, _, C(0)) | (Bo::Sar, _, C(0)) => a,
        (Bo::Mul, _, C(1)) | (Bo::Div, _, C(1)) => a,
        (Bo::And, _, C(-1)) => a,
        (Bo::Mul, _, C(0)) | (Bo::And, _, C(0)) => C(0),
        (Bo::Add, C(_), _) | (Bo::Mul, C(_), _) | (Bo::And, C(_), _) | (Bo::Or, C(_), _) => bin(op, b, a),
        (Bo::Add, Bin(Bo::Add, x, y), C(k)) => match **y {
            C(j) => bin(Bo::Add, (**x).clone(), C(j.wrapping_add(*k))),
            _ => Bin(op, bx(a), bx(b)),
        },
        _ => Bin(op, bx(a), bx(b)),
    }
}

fn sub(a: E, b: E) -> E {
    match b {
        E::C(k) => bin(Bo::Add, a, E::C(k.wrapping_neg())),
        _ if a == b => E::C(0),
        _ => bin(Bo::Add, a, E::Un(Uo::Neg, bx(b))),
    }
}

fn cast(size: u8, signed: bool, e: E) -> E {
    match e {
        E::C(v) => {
            let bits = u32::from(size) * 8;
            if bits >= 64 {
                return E::C(v);
            }
            let m = (v as u64) & ((1u64 << bits) - 1);
            E::C(if signed && m >> (bits - 1) & 1 == 1 { (m | !((1u64 << bits) - 1)) as i64 } else { m as i64 })
        }
        E::Cmp(..) => e,
        E::Ld(_, s) if s <= size => e,
        E::Cast(s, sg, inner) if s <= size => E::Cast(s, sg, inner),
        other => E::Cast(size, signed, bx(other)),
    }
}

fn negate(c: E) -> E {
    match c {
        E::Cmp(op, a, b) => E::Cmp(op.neg(), a, b),
        E::Un(Uo::LNot, x) => *x,
        E::C(v) => E::C(i64::from(v == 0)),
        other => E::Un(Uo::LNot, bx(other)),
    }
}

fn cmp(op: Co, a: E, b: E) -> E {
    // (x - k) == 0  →  x == k
    if let (E::Bin(Bo::Add, x, k), E::C(0)) = (&a, &b) {
        if let E::C(k) = **k {
            if matches!(op, Co::Eq | Co::Ne) {
                return E::Cmp(op, x.clone(), bx(E::C(k.wrapping_neg())));
            }
        }
    }
    E::Cmp(op, bx(a), bx(b))
}

impl E {
    /// Visita todas as subexpressões (pré-ordem).
    fn walk(&self, f: &mut dyn FnMut(&E)) {
        f(self);
        match self {
            E::Ld(a, _) | E::Un(_, a) | E::Cast(_, _, a) => a.walk(f),
            E::Bin(_, a, b) | E::Cmp(_, a, b) => {
                a.walk(f);
                b.walk(f);
            }
            E::Tern(a, b, c) => {
                a.walk(f);
                b.walk(f);
                c.walk(f);
            }
            E::Call(c) => {
                c.target.walk(f);
                for a in &c.args {
                    a.walk(f);
                }
            }
            _ => {}
        }
    }

    fn any(&self, p: &dyn Fn(&E) -> bool) -> bool {
        let mut hit = false;
        self.walk(&mut |e| hit |= p(e));
        hit
    }

    /// Reescreve de baixo para cima.
    fn map(self, f: &mut dyn FnMut(E) -> E) -> E {
        let e = match self {
            E::Ld(a, s) => E::Ld(bx(a.map(f)), s),
            E::Un(o, a) => E::Un(o, bx(a.map(f))),
            E::Cast(s, g, a) => E::Cast(s, g, bx(a.map(f))),
            E::Bin(o, a, b) => E::Bin(o, bx(a.map(f)), bx(b.map(f))),
            E::Cmp(o, a, b) => E::Cmp(o, bx(a.map(f)), bx(b.map(f))),
            E::Tern(a, b, c) => E::Tern(bx(a.map(f)), bx(b.map(f)), bx(c.map(f))),
            E::Call(c) => {
                let c = *c;
                E::Call(Box::new(CallE {
                    addr: c.addr,
                    target: c.target.map(f),
                    name: c.name,
                    args: c.args.into_iter().map(|a| a.map(f)).collect(),
                }))
            }
            other => other,
        };
        f(e)
    }

    /// Número de nós, parando em `limit` (barato para expressões grandes).
    fn size_upto(&self, limit: usize) -> usize {
        let mut n = 0;
        self.walk_until(&mut n, limit);
        n
    }

    fn walk_until(&self, n: &mut usize, limit: usize) {
        *n += 1;
        if *n >= limit {
            return;
        }
        match self {
            E::Ld(a, _) | E::Un(_, a) | E::Cast(_, _, a) => a.walk_until(n, limit),
            E::Bin(_, a, b) | E::Cmp(_, a, b) => {
                a.walk_until(n, limit);
                b.walk_until(n, limit);
            }
            E::Tern(a, b, c) => {
                a.walk_until(n, limit);
                b.walk_until(n, limit);
                c.walk_until(n, limit);
            }
            E::Call(c) => {
                c.target.walk_until(n, limit);
                for a in &c.args {
                    a.walk_until(n, limit);
                }
            }
            _ => {}
        }
    }

    fn has_side_effects(&self) -> bool {
        self.any(&|e| matches!(e, E::Call(_)))
    }

    fn is_simple(&self) -> bool {
        matches!(self, E::C(_) | E::Str(_) | E::Sym(_) | E::Imp(_) | E::Glob(_) | E::Sp(_) | E::Saved(_) | E::Res(_) | E::Raw(_))
    }
}

// ===================================================================== variáveis, comandos

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum VKey {
    Param(u32),
    Stack(i64),
    Merge(usize, Reg),
    Temp(u64, Reg),
    In(Reg),
    Fresh(u64),
    Ret(u64),
}

#[derive(Debug, Clone)]
enum St {
    Set(u32, E),
    Store(E, E, u8),
    Call(CallE),
    Asm(String),
}

#[derive(Debug, Clone)]
struct Stmt {
    addr: u64,
    st: St,
}

impl Stmt {
    fn exprs(&self) -> Vec<&E> {
        match &self.st {
            St::Set(_, e) => vec![e],
            St::Store(a, v, _) => vec![a, v],
            St::Call(c) => {
                let mut v = vec![&c.target];
                v.extend(c.args.iter());
                v
            }
            St::Asm(_) => vec![],
        }
    }

    fn map_exprs(self, f: &mut dyn FnMut(E) -> E) -> Stmt {
        let st = match self.st {
            St::Set(v, e) => St::Set(v, e.map(f)),
            St::Store(a, v, s) => St::Store(a.map(f), v.map(f), s),
            St::Call(c) => match E::Call(Box::new(c)).map(f) {
                E::Call(c) => St::Call(*c),
                other => St::Set(u32::MAX, other), // não acontece (map preserva Call)
            },
            St::Asm(t) => St::Asm(t),
        };
        Stmt { addr: self.addr, st }
    }
}

#[derive(Debug, Clone)]
enum Term {
    Goto(usize),
    Br(E, usize, usize),
    Ret(Option<E>),
    Tail(CallE),
    /// Índice, valor do primeiro caso, destinos por posição na tabela.
    Switch(E, i64, Vec<usize>),
    Stop(String),
}

#[derive(Debug, Clone, PartialEq)]
enum Fl {
    No,
    Cmp(E, E),
    Res(E),
}

#[derive(Debug, Clone, PartialEq)]
struct State {
    r: Vec<Option<E>>,
    fl: Fl,
}

struct Pending {
    off: i64,
    size: u8,
    val: E,
    addr: u64,
}

#[derive(Debug, Clone)]
struct Lifted {
    stmts: Vec<Stmt>,
    term: Term,
    exit: State,
}

struct Block {
    start: u64,
    ins: Vec<usize>,
    succs: Vec<usize>,
    preds: Vec<usize>,
}

// ===================================================================== contexto

struct Ctx<'a> {
    eng: &'a NativeEngine,
    f: &'a Func,
    cpu: CpuArch,
    abi: Abi,
    ptr: u8,
    ins: Vec<Ins>,
    blocks: Vec<Block>,
    block_at: HashMap<u64, usize>,
    vars: Vec<VKey>,
    var_id: HashMap<VKey, u32>,
    var_size: HashMap<u32, u8>,
    merged: HashSet<(usize, Reg)>,
    lifted: Vec<Option<Lifted>>,
    exits: Vec<Option<State>>,
    strings: HashMap<u64, (String, bool)>,
    callee_params: HashMap<u64, usize>,
    callee_ret_imm: HashMap<u64, i64>,
    api_cache: HashMap<String, Option<Api>>,
    /// Registradores escritos desde a última chamada (inferência de argumentos).
    written: HashSet<Reg>,
    /// Última comparação com constante no bloco (ou herdada): limite do `switch`.
    last_cmp: Fl,
    /// Cópias de cada aresta já em ordem sequencial válida.
    seq_copies: HashMap<(usize, usize), Vec<(u32, E)>>,
}

/// Ponto de entrada: `None` quando a função não é x86 ou é grande demais.
pub(super) fn decompile(eng: &NativeEngine, f: &Func, abi: Abi) -> Option<Vec<CodeLine>> {
    let cpu = eng.cpu?;
    if !cpu.is_x86() || f.insns.len() > MAX_INSNS || f.insns.is_empty() {
        return None;
    }
    let ins = decode_all(eng, f, cpu)?;
    let mut cx = Ctx {
        eng,
        f,
        cpu,
        abi,
        ptr: if cpu == CpuArch::X64 { 8 } else { 4 },
        ins,
        blocks: vec![],
        block_at: HashMap::new(),
        vars: vec![],
        var_id: HashMap::new(),
        var_size: HashMap::new(),
        merged: HashSet::new(),
        lifted: vec![],
        exits: vec![],
        strings: HashMap::new(),
        callee_params: HashMap::new(),
        callee_ret_imm: HashMap::new(),
        api_cache: HashMap::new(),
        written: HashSet::new(),
        last_cmp: Fl::No,
        seq_copies: HashMap::new(),
    };
    cx.build_cfg();
    if cx.blocks.is_empty() || cx.blocks.len() > MAX_BLOCKS {
        return None;
    }
    cx.dataflow();
    cx.sequentialize_copies();
    let entry = *cx.block_at.get(&f.start)?;
    cx.lifted[entry].as_ref()?;
    let ast = Structurer::run(&cx, entry);
    let ast = cleanup(&mut cx, ast);
    Some(render(&mut cx, ast))
}

fn decode_all(eng: &NativeEngine, f: &Func, cpu: CpuArch) -> Option<Vec<Ins>> {
    let mode = if cpu == CpuArch::X64 { arch::x86::ArchMode::Mode64 } else { arch::x86::ArchMode::Mode32 };
    let cs = Capstone::new().x86().mode(mode).syntax(arch::x86::ArchSyntax::Intel).detail(true).build().ok()?;
    let mut out = Vec::with_capacity(f.insns.len());
    for &(addr, len) in &f.insns {
        let code = eng.bytes_at(addr, len as usize)?;
        let dis = cs.disasm_count(code, addr, 1).ok()?;
        let i = dis.iter().next()?;
        let det = cs.insn_detail(i).ok()?;
        let mut ops = vec![];
        for o in det.arch_detail().operands() {
            let ArchOperand::X86Operand(x) = o else { continue };
            let op = match x.op_type {
                X86OperandType::Reg(r) => {
                    let n = cs.reg_name(r).unwrap_or_default();
                    parse_reg(&n).map_or(Opnd::Other(n), Opnd::R)
                }
                X86OperandType::Imm(v) => Opnd::I(v),
                X86OperandType::Mem(m) => {
                    let name = |id: RegId| if id.0 == 0 { None } else { cs.reg_name(id) };
                    let base = name(m.base());
                    let rip = matches!(base.as_deref(), Some("rip") | Some("eip"));
                    Opnd::M(Mem {
                        size: x.size,
                        seg: name(m.segment()),
                        base: if rip { None } else { base.as_deref().and_then(parse_reg) },
                        rip,
                        index: name(m.index()).as_deref().and_then(parse_reg),
                        scale: i64::from(m.scale()),
                        disp: m.disp(),
                    })
                }
                X86OperandType::Invalid => continue,
            };
            ops.push(op);
        }
        let mn = super::strip_prefixes(i.mnemonic().unwrap_or("")).to_string();
        let op = i.op_str().unwrap_or("");
        out.push(Ins {
            addr,
            next: addr + u64::from(len),
            text: if op.is_empty() { mn.clone() } else { format!("{mn} {op}") },
            mn,
            ops,
        });
    }
    Some(out)
}

fn is_jcc(mn: &str) -> bool {
    mn.starts_with('j') && mn != "jmp"
}

impl<'a> Ctx<'a> {
    // ------------------------------------------------------------- CFG

    fn call_target_name(&self, ins: &Ins) -> Option<String> {
        match ins.ops.first()? {
            Opnd::I(t) => self.eng.name_of(*t as u64),
            Opnd::M(m) if m.rip => self.eng.extern_names.get(&((ins.next as i64).wrapping_add(m.disp) as u64)).cloned(),
            Opnd::M(m) if m.base.is_none() && m.index.is_none() => self.eng.extern_names.get(&(m.disp as u64)).cloned(),
            _ => None,
        }
    }

    fn build_cfg(&mut self) {
        let starts: HashSet<u64> = self.f.blocks.iter().copied().collect();
        let switches: HashMap<u64, &Vec<u64>> = self.f.switches.iter().map(|(a, t)| (*a, t)).collect();
        let mut cur: Option<usize> = None;
        let mut ends_after: Vec<bool> = vec![];
        for (ii, ins) in self.ins.iter().enumerate() {
            let new_block = cur.is_none() || starts.contains(&ins.addr) || ends_after.last().copied().unwrap_or(false)
                || self.blocks.last().is_some_and(|b| {
                    // instrução não contígua (blocos espalhados)
                    let last = *b.ins.last().unwrap();
                    self.ins[last].next != ins.addr
                });
            if new_block {
                self.block_at.insert(ins.addr, self.blocks.len());
                self.blocks.push(Block { start: ins.addr, ins: vec![], succs: vec![], preds: vec![] });
                cur = Some(self.blocks.len() - 1);
                ends_after.push(false);
            }
            let b = cur.unwrap();
            self.blocks[b].ins.push(ii);
            let mn = ins.mn.as_str();
            let ends = mn == "jmp"
                || is_jcc(mn)
                || mn.starts_with("ret")
                || matches!(mn, "hlt" | "ud2" | "int3")
                || (mn == "call" && self.call_target_name(ins).is_some_and(|n| winapi::is_noreturn(&n)));
            *ends_after.last_mut().unwrap() = ends;
        }
        // Sucessores.
        for b in 0..self.blocks.len() {
            let last = &self.ins[*self.blocks[b].ins.last().unwrap()];
            let next_block = self.block_at.get(&last.next).copied();
            let target = match last.ops.first() {
                Some(Opnd::I(t)) => self.block_at.get(&(*t as u64)).copied(),
                _ => None,
            };
            let mn = last.mn.as_str();
            let succs: Vec<usize> = if mn == "jmp" {
                if let Some(ts) = switches.get(&last.addr) {
                    let mut v: Vec<usize> = ts.iter().filter_map(|t| self.block_at.get(t).copied()).collect();
                    v.dedup();
                    let mut seen = HashSet::new();
                    v.retain(|x| seen.insert(*x));
                    v
                } else {
                    target.into_iter().collect()
                }
            } else if is_jcc(mn) {
                match (target, next_block) {
                    (Some(t), Some(n)) => vec![t, n],
                    (Some(t), None) => vec![t],
                    (None, Some(n)) => vec![n],
                    _ => vec![],
                }
            } else if mn.starts_with("ret")
                || matches!(mn, "hlt" | "ud2" | "int3")
                || (mn == "call" && self.call_target_name(last).is_some_and(|n| winapi::is_noreturn(&n)))
            {
                vec![]
            } else {
                next_block.into_iter().collect()
            };
            self.blocks[b].succs = succs;
        }
        for b in 0..self.blocks.len() {
            for s in self.blocks[b].succs.clone() {
                if !self.blocks[s].preds.contains(&b) {
                    self.blocks[s].preds.push(b);
                }
            }
        }
        self.lifted = vec![None; self.blocks.len()];
        self.exits = vec![None; self.blocks.len()];
    }

    fn rpo(&self, entry: usize) -> Vec<usize> {
        let n = self.blocks.len();
        let mut seen = vec![false; n];
        let mut post = Vec::with_capacity(n);
        let mut stack: Vec<(usize, usize)> = vec![(entry, 0)];
        seen[entry] = true;
        while let Some((b, i)) = stack.pop() {
            if i < self.blocks[b].succs.len() {
                stack.push((b, i + 1));
                let s = self.blocks[b].succs[i];
                if !seen[s] {
                    seen[s] = true;
                    stack.push((s, 0));
                }
            } else {
                post.push(b);
            }
        }
        post.reverse();
        post
    }

    // ------------------------------------------------------------- variáveis

    fn var(&mut self, k: VKey) -> u32 {
        if let Some(&id) = self.var_id.get(&k) {
            return id;
        }
        let id = self.vars.len() as u32;
        self.vars.push(k);
        self.var_id.insert(k, id);
        id
    }

    fn note_size(&mut self, v: u32, size: u8) {
        let e = self.var_size.entry(v).or_insert(size);
        *e = (*e).max(size);
    }

    fn stack_key(&self, off: i64) -> VKey {
        let p = i64::from(self.ptr);
        match self.abi {
            Abi::Win64 if off >= 8 && off % 8 == 0 => VKey::Param(((off - 8) / 8) as u32),
            Abi::SysV64 if off >= 8 && off % 8 == 0 => VKey::Param(6 + ((off - 8) / 8) as u32),
            Abi::X86 if off >= p && off % p == 0 => VKey::Param(((off - p) / p) as u32),
            _ => VKey::Stack(off),
        }
    }

    fn arg_regs(&self) -> &'static [Reg] {
        match self.abi {
            Abi::Win64 => &[RCX, RDX, R8, R9],
            Abi::SysV64 => &[RDI, RSI, RDX, RCX, R8, R9],
            Abi::X86 => &[],
        }
    }

    fn volatile(&self) -> Vec<Reg> {
        let mut v: Vec<Reg> = match self.abi {
            Abi::Win64 => vec![RAX, RCX, RDX, R8, R9, R10, R11],
            Abi::SysV64 => vec![RAX, RCX, RDX, RSI, RDI, R8, R9, R10, R11],
            Abi::X86 => vec![RAX, RCX, RDX],
        };
        let xmm_volatile = if self.abi == Abi::Win64 { 6 } else { 32 };
        v.extend((0..xmm_volatile).map(|k| XMM0 + k));
        v
    }

    fn callee_saved(&self) -> Vec<Reg> {
        match self.abi {
            Abi::Win64 => vec![RBX, RBP, RSI, RDI, 12, 13, 14, 15],
            Abi::SysV64 => vec![RBX, RBP, 12, 13, 14, 15],
            Abi::X86 => vec![RBX, RBP, RSI, RDI],
        }
    }

    fn initial_state(&mut self) -> State {
        let mut r: Vec<Option<E>> = vec![None; NREG];
        r[RSP] = Some(E::Sp(0));
        for s in self.callee_saved() {
            r[s] = Some(E::Saved(s));
        }
        for (i, &reg) in self.arg_regs().iter().enumerate() {
            let v = self.var(VKey::Param(i as u32));
            r[reg] = Some(E::V(v));
        }
        State { r, fl: Fl::No }
    }

    // ------------------------------------------------------------- endereços e strings

    fn read_string(&mut self, ea: u64) -> Option<()> {
        if self.strings.contains_key(&ea) {
            return Some(());
        }
        if self.eng.is_code(ea) {
            return None;
        }
        if self.read_raw_string(ea).is_some() {
            return Some(());
        }
        if let Some(s) = self.eng.string_at.get(&ea) {
            let wide = self.eng.bytes_at(ea, 2).is_some_and(|b| b.len() == 2 && b[1] == 0 && b[0] != 0);
            self.strings.insert(ea, (s.clone(), wide));
            return Some(());
        }
        None
    }

    /// Texto terminado em NUL direto dos bytes (mantém `\n`, `\t`...).
    fn read_raw_string(&mut self, ea: u64) -> Option<()> {
        let bytes = self.eng.bytes_at(ea, 1024)?;
        let ok = |b: u8| (0x20..0x7f).contains(&b) || matches!(b, b'\n' | b'\r' | b'\t') || b >= 0xa0;
        // UTF-16LE
        if bytes.len() >= 6 && bytes[1] == 0 && ok(bytes[0]) && bytes[3] == 0 {
            let mut s = String::new();
            let mut i = 0;
            while i + 1 < bytes.len() {
                let u = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
                if u == 0 {
                    if s.chars().count() >= 2 {
                        self.strings.insert(ea, (s, true));
                        return Some(());
                    }
                    return None;
                }
                let c = char::from_u32(u32::from(u))?;
                if c.is_control() && !matches!(c, '\n' | '\r' | '\t') {
                    return None;
                }
                s.push(c);
                i += 2;
            }
            return None;
        }
        let end = bytes.iter().position(|&b| b == 0)?;
        if end < 2 || !bytes[..end].iter().all(|&b| ok(b)) {
            return None;
        }
        // Precisa ter pelo menos uma letra (evita confundir número com texto).
        if !bytes[..end].iter().any(|b| b.is_ascii_alphabetic()) {
            return None;
        }
        self.strings.insert(ea, (String::from_utf8_lossy(&bytes[..end]).into_owned(), false));
        Some(())
    }

    /// O que existe em um endereço absoluto usado como valor.
    fn classify(&mut self, ea: u64) -> E {
        if let Some(n) = self.eng.extern_names.get(&ea) {
            return if self.eng.is_code(ea) { E::Sym(n.clone()) } else { E::Imp(n.clone()) };
        }
        if let Some(&i) = self.eng.by_addr.get(&ea) {
            return E::Sym(self.eng.funcs[i].name.clone());
        }
        if self.read_string(ea).is_some() {
            return E::Str(ea);
        }
        E::Glob(ea)
    }

    fn imm(&mut self, v: i64) -> E {
        let a = v as u64;
        if a >= 0x1000 && self.eng.region_of(a).is_some() {
            if self.eng.by_addr.contains_key(&a) || self.eng.extern_names.contains_key(&a) {
                return self.classify(a);
            }
            if self.read_string(a).is_some() {
                return E::Str(a);
            }
        }
        E::C(v)
    }

    fn rd_reg(&mut self, st: &State, rr: RegRef) -> E {
        let v = match &st.r[rr.r] {
            Some(e) => e.clone(),
            None => {
                let id = self.var(VKey::In(rr.r));
                E::V(id)
            }
        };
        if rr.high {
            return cast(1, false, bin(Bo::Shr, v, E::C(8)));
        }
        match rr.size {
            1 | 2 => cast(rr.size, false, v),
            4 if self.ptr == 8 => match v {
                E::C(c) => cast(4, false, E::C(c)),
                other => other,
            },
            _ => v,
        }
    }

    fn addr_of(&mut self, st: &State, m: &Mem, ins: &Ins) -> E {
        if let Some(seg) = m.seg.as_deref().filter(|s| matches!(*s, "fs" | "gs")) {
            return E::Raw(format!("{seg}:0x{:x}", m.disp));
        }
        if m.rip {
            return self.classify((ins.next as i64).wrapping_add(m.disp) as u64);
        }
        let mut e: Option<E> = None;
        if let Some(b) = m.base {
            let v = self.rd_reg(st, RegRef { size: self.ptr, ..b });
            e = Some(v);
        }
        if let Some(i) = m.index {
            let v = self.rd_reg(st, RegRef { size: self.ptr, ..i });
            let scaled = bin(Bo::Mul, v, E::C(m.scale.max(1)));
            e = Some(match e {
                Some(x) => bin(Bo::Add, x, scaled),
                None => scaled,
            });
        }
        match e {
            None => self.classify(m.disp as u64),
            Some(x) => match bin(Bo::Add, x, E::C(m.disp)) {
                E::C(a) if a > 0x1000 => self.classify(a as u64),
                other => other,
            },
        }
    }

    fn stack_read(&mut self, off: i64, size: u8, pend: &mut Vec<Pending>, out: &mut Vec<Stmt>, st: &mut State) -> E {
        if let Some(i) = pend.iter().position(|p| p.off == off) {
            // Lido de volta antes da chamada: vira uma variável normal.
            let p = pend.remove(i);
            self.stack_store(off, p.size, p.val, p.addr, st, pend, out, false);
        }
        let v = self.var(self.stack_key(off));
        self.note_size(v, size);
        E::V(v)
    }

    fn read(&mut self, st: &mut State, op: &Opnd, ins: &Ins, pend: &mut Vec<Pending>, out: &mut Vec<Stmt>) -> E {
        match op {
            Opnd::R(rr) => self.rd_reg(st, *rr),
            Opnd::I(v) => self.imm(*v),
            Opnd::M(m) => {
                let a = self.addr_of(st, m, ins);
                self.load(a, m.size, st, pend, out)
            }
            Opnd::Other(n) => E::Raw(n.clone()),
        }
    }

    fn load(&mut self, a: E, size: u8, st: &mut State, pend: &mut Vec<Pending>, out: &mut Vec<Stmt>) -> E {
        match a {
            E::Sp(off) => self.stack_read(off, size, pend, out, st),
            E::Imp(n) => E::Sym(n),
            other => E::Ld(bx(other), size),
        }
    }

    /// Antes de escrever em `var`, registradores que ainda guardam o valor antigo
    /// viram temporários (senão a expressão mudaria de significado).
    fn materialize(&mut self, st: &mut State, out: &mut Vec<Stmt>, addr: u64, pred: &dyn Fn(&E) -> bool) {
        for r in 0..NREG {
            let Some(e) = st.r[r].clone() else { continue };
            if r == RSP || e.is_simple() || !e.any(pred) {
                continue;
            }
            let t = self.var(VKey::Temp(addr, r));
            out.push(Stmt { addr, st: St::Set(t, e) });
            st.r[r] = Some(E::V(t));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn stack_store(&mut self, off: i64, size: u8, val: E, addr: u64, st: &mut State, pend: &mut Vec<Pending>, out: &mut Vec<Stmt>, allow_pending: bool) {
        if matches!(val, E::Saved(_)) {
            return; // registrador preservado sendo guardado
        }
        let key = self.stack_key(off);
        let v = self.var(key);
        if let VKey::Param(_) = key {
            if val == E::V(v) {
                return; // parâmetro copiado para a própria área (home space)
            }
            if let E::Cast(sz, _, inner) = &val {
                if **inner == E::V(v) {
                    self.note_size(v, *sz); // `mov [rbp+0x18], dl`: parâmetro char
                    self.var_size.insert(v, *sz);
                    return;
                }
            }
        }
        if allow_pending {
            let sp = match &st.r[RSP] {
                Some(E::Sp(s)) => Some(*s),
                _ => None,
            };
            let window = match self.abi {
                Abi::Win64 => 0x20 + 8 * 16,
                Abi::SysV64 => 8 * 16,
                Abi::X86 => 4 * 20,
            };
            if let Some(sp) = sp {
                if off >= sp && off < sp + window {
                    pend.retain(|p| p.off != off);
                    pend.push(Pending { off, size, val, addr });
                    return;
                }
            }
        }
        self.note_size(v, size);
        self.materialize(st, out, addr, &|e| *e == E::V(v));
        out.push(Stmt { addr, st: St::Set(v, val) });
    }

    fn flush(&mut self, st: &mut State, pend: &mut Vec<Pending>, out: &mut Vec<Stmt>) {
        let items: Vec<Pending> = std::mem::take(pend);
        for p in items {
            self.stack_store(p.off, p.size, p.val, p.addr, st, pend, out, false);
        }
    }

    fn write(&mut self, st: &mut State, op: &Opnd, val: E, ins: &Ins, pend: &mut Vec<Pending>, out: &mut Vec<Stmt>) {
        match op {
            Opnd::R(rr) => {
                self.written.insert(rr.r);
                // Expressão grande demais (ex.: `lea eax, [rax+rax*2]` repetido): vira variável.
                let val = if val.size_upto(MAX_EXPR) >= MAX_EXPR && rr.r != RSP {
                    let t = self.var(VKey::Temp(ins.addr, rr.r));
                    out.push(Stmt { addr: ins.addr, st: St::Set(t, val) });
                    E::V(t)
                } else {
                    val
                };
                st.r[rr.r] = Some(val);
            }
            Opnd::M(m) => {
                let a = self.addr_of(st, m, ins);
                match a {
                    // Argumentos de pilha são escritos via rsp/esp; locais (O0) via rbp.
                    E::Sp(off) => {
                        let via_sp = m.base.is_some_and(|b| b.r == RSP);
                        self.stack_store(off, m.size, val, ins.addr, st, pend, out, via_sp)
                    }
                    a => {
                        self.flush(st, pend, out);
                        self.materialize(st, out, ins.addr, &|e| matches!(e, E::Ld(..)));
                        out.push(Stmt { addr: ins.addr, st: St::Store(a, val, m.size) });
                    }
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------- chamadas

    fn api(&mut self, name: &str) -> Option<Api> {
        if let Some(a) = self.api_cache.get(name) {
            return a.clone();
        }
        let a = winapi::lookup(name);
        self.api_cache.insert(name.to_string(), a.clone());
        a
    }

    /// Quantos argumentos em registrador uma função interna lê antes de escrever.
    fn params_of(&mut self, addr: u64) -> Option<usize> {
        if let Some(&n) = self.callee_params.get(&addr) {
            return Some(n);
        }
        let &i = self.eng.by_addr.get(&addr)?;
        let callee = &self.eng.funcs[i];
        let ins = decode_all(self.eng, &Func { insns: callee.insns.iter().take(400).copied().collect(), ..callee.shallow() }, self.cpu)?;
        let regs = self.arg_regs();
        let mut written: HashSet<Reg> = HashSet::new();
        let mut used = 0usize;
        let mut ret_imm = 0i64;
        for x in &ins {
            if x.mn == "call" || x.mn == "jmp" || is_jcc(&x.mn) {
                break;
            }
            if x.mn.starts_with("ret") {
                if let Some(Opnd::I(n)) = x.ops.first() {
                    ret_imm = *n;
                }
                break;
            }
            let dst_written = matches!(x.mn.as_str(), "mov" | "movzx" | "movsx" | "movsxd" | "lea" | "pop") || (x.mn == "xor" && x.ops.len() == 2 && matches!((&x.ops[0], &x.ops[1]), (Opnd::R(a), Opnd::R(b)) if a.r == b.r));
            for (k, o) in x.ops.iter().enumerate() {
                let reads: Vec<Reg> = match o {
                    Opnd::R(rr) if !(k == 0 && dst_written) => vec![rr.r],
                    Opnd::M(m) => m.base.iter().chain(m.index.iter()).map(|r| r.r).collect(),
                    _ => vec![],
                };
                for r in reads {
                    if let Some(p) = regs.iter().position(|x| *x == r) {
                        if !written.contains(&r) {
                            used = used.max(p + 1);
                        }
                    }
                }
            }
            if dst_written {
                if let Some(Opnd::R(rr)) = x.ops.first() {
                    written.insert(rr.r);
                }
            }
        }
        // `ret N` (stdcall 32 bits): procura em toda a função.
        if self.abi == Abi::X86 && ret_imm == 0 {
            if let Some(all) = decode_all(self.eng, callee, self.cpu) {
                if let Some(Opnd::I(n)) = all.iter().find(|x| x.mn.starts_with("ret")).and_then(|x| x.ops.first()) {
                    ret_imm = *n;
                }
            }
        }
        self.callee_params.insert(addr, used);
        self.callee_ret_imm.insert(addr, ret_imm);
        Some(used)
    }

    fn do_call(&mut self, st: &mut State, out: &mut Vec<Stmt>, pend: &mut Vec<Pending>, ins: &Ins, target: E, tail: bool) -> (CallE, bool) {
        let raw_name = match &target {
            E::Sym(n) => Some(n.clone()),
            _ => None,
        };
        let api = raw_name.as_deref().and_then(|n| self.api(n));
        let internal = match ins.ops.first() {
            Some(Opnd::I(t)) if self.eng.by_addr.contains_key(&(*t as u64)) => Some(*t as u64),
            _ => None,
        };
        let sp = match &st.r[RSP] {
            Some(E::Sp(s)) => *s,
            _ => 0,
        };
        let p = i64::from(self.ptr);
        let extra = if tail { p } else { 0 };
        let regs = self.arg_regs();
        let stack_base = match self.abi {
            Abi::Win64 => sp + 0x20 + extra,
            Abi::SysV64 | Abi::X86 => sp + extra,
        };
        let pending_count = {
            let mut n = 0;
            while pend.iter().any(|q| q.off == stack_base + n * p) {
                n += 1;
            }
            n as usize
        };
        let reg_count = regs.iter().rposition(|r| self.written.contains(r)).map_or(0, |i| i + 1);
        let inferred = if pending_count > 0 && self.abi != Abi::X86 { regs.len() + pending_count } else { reg_count.max(pending_count) };
        let nargs = match (&api, internal) {
            (Some(a), _) if a.variadic => a.params.len().max(inferred),
            (Some(a), _) => a.params.len(),
            (None, Some(t)) => match self.abi {
                Abi::X86 => pending_count,
                _ => self.params_of(t).unwrap_or(reg_count).max(if pending_count > 0 { regs.len() + pending_count } else { 0 }),
            },
            (None, None) => inferred,
        };
        let mut args = Vec::with_capacity(nargs);
        for i in 0..nargs {
            if i < regs.len() {
                let rr = RegRef { r: regs[i], size: 8, high: false };
                args.push(self.rd_reg(st, rr));
            } else {
                let k = (i - regs.len()) as i64;
                let off = stack_base + k * p;
                if let Some(j) = pend.iter().position(|q| q.off == off) {
                    args.push(pend.remove(j).val);
                } else {
                    let v = self.var(self.stack_key(off));
                    args.push(E::V(v));
                }
            }
        }
        self.flush(st, pend, out);
        let saved = self.callee_saved();
        // Valores em registradores preservados que dependem de memória: congela.
        let mut keep: Vec<Option<E>> = vec![];
        for r in 0..NREG {
            keep.push(if saved.contains(&r) { None } else { st.r[r].take() });
        }
        let stack_vars: HashSet<u32> = self.vars.iter().enumerate().filter(|(_, k)| matches!(k, VKey::Stack(_))).map(|(i, _)| i as u32).collect();
        self.materialize(st, out, ins.addr, &|e| matches!(e, E::Ld(..)) || matches!(e, E::V(v) if stack_vars.contains(v)));
        for (r, k) in keep.into_iter().enumerate() {
            if k.is_some() {
                st.r[r] = k;
            }
        }
        let call = CallE { addr: ins.addr, target, name: raw_name.clone(), args };
        if !tail {
            out.push(Stmt { addr: ins.addr, st: St::Call(call.clone()) });
        }
        for r in self.volatile() {
            st.r[r] = None;
        }
        let void = api.as_ref().is_some_and(Api::returns_void);
        if !void {
            st.r[RAX] = Some(E::Res(ins.addr));
        }
        // stdcall 32 bits: a função chamada limpa os argumentos.
        if self.abi == Abi::X86 && !tail {
            let cdecl_lib = raw_name.as_deref().is_some_and(|n| {
                let l = n.to_ascii_lowercase();
                l.contains("msvcr") || l.contains("ucrt") || l.contains("api-ms-win-crt") || l.ends_with("@plt") || !l.contains('!')
            });
            let pop = match (&api, internal) {
                (Some(a), _) if !a.variadic && !cdecl_lib => i64::from(self.ptr) * a.params.len() as i64,
                (None, Some(t)) => self.callee_ret_imm.get(&t).copied().unwrap_or(0),
                (None, None) if raw_name.as_deref().is_some_and(|n| n.contains('!') && !cdecl_lib) => p * nargs as i64,
                _ => 0,
            };
            if let Some(E::Sp(s)) = st.r[RSP].clone() {
                st.r[RSP] = Some(E::Sp(s + pop));
            }
        }
        st.fl = Fl::No;
        self.written.clear();
        let noreturn = raw_name.as_deref().is_some_and(winapi::is_noreturn);
        (call, noreturn)
    }

    // ------------------------------------------------------------- condições

    fn cond(&self, fl: &Fl, mn: &str) -> E {
        let cc = mn.trim_start_matches(['j', 's']).trim_start_matches("cmov").trim_start_matches("et");
        let cc = mn.strip_prefix("set").or_else(|| mn.strip_prefix("cmov")).unwrap_or(cc);
        let co = match cc {
            "e" | "z" => Co::Eq,
            "ne" | "nz" => Co::Ne,
            "l" | "nge" => Co::Lt,
            "le" | "ng" => Co::Le,
            "g" | "nle" => Co::Gt,
            "ge" | "nl" => Co::Ge,
            "b" | "c" | "nae" => Co::ULt,
            "be" | "na" => Co::ULe,
            "a" | "nbe" => Co::UGt,
            "ae" | "nb" | "nc" => Co::UGe,
            "s" => Co::Lt,
            "ns" => Co::Ge,
            _ => return E::Raw(format!("/* {mn} */ cond")),
        };
        match fl {
            Fl::Cmp(a, b) => {
                if matches!(cc, "s" | "ns") {
                    return cmp(co, sub(a.clone(), b.clone()), E::C(0));
                }
                cmp(co, a.clone(), b.clone())
            }
            Fl::Res(v) => {
                let co = match co {
                    Co::UGt => Co::Ne,
                    Co::ULe => Co::Eq,
                    Co::ULt => return E::C(0),
                    Co::UGe => return E::C(1),
                    c => c,
                };
                cmp(co, v.clone(), E::C(0))
            }
            Fl::No => E::Raw(format!("/* {mn} */ cond")),
        }
    }

    // ------------------------------------------------------------- execução simbólica

    fn lift(&mut self, b: usize, entry: State) -> Lifted {
        let mut st = entry;
        let mut out: Vec<Stmt> = vec![];
        let mut pend: Vec<Pending> = vec![];
        let mut term: Option<Term> = None;
        self.written.clear();
        self.last_cmp = st.fl.clone();
        let list = self.blocks[b].ins.clone();
        for ii in list {
            let ins = self.ins[ii].clone();
            if let Fl::Cmp(_, E::C(_)) = st.fl {
                self.last_cmp = st.fl.clone();
            }
            if let Some(t) = self.step(b, &mut st, &mut out, &mut pend, &ins) {
                term = Some(t);
                break;
            }
        }
        self.flush(&mut st, &mut pend, &mut out);
        let term = term.unwrap_or_else(|| match self.blocks[b].succs.first() {
            Some(&s) => Term::Goto(s),
            None => Term::Stop(String::new()),
        });
        Lifted { stmts: out, term, exit: st }
    }

    fn fresh(&mut self, st: &mut State, out: &mut Vec<Stmt>, ins: &Ins) {
        out.push(Stmt { addr: ins.addr, st: St::Asm(ins.text.clone()) });
        if let Some(Opnd::R(rr)) = ins.ops.first() {
            let v = self.var(VKey::Fresh(ins.addr));
            st.r[rr.r] = Some(E::V(v));
        }
        st.fl = Fl::No;
    }

    fn step(&mut self, b: usize, st: &mut State, out: &mut Vec<Stmt>, pend: &mut Vec<Pending>, ins: &Ins) -> Option<Term> {
        let mn = ins.mn.as_str();
        let ops = ins.ops.clone();
        let o0 = ops.first();
        let o1 = ops.get(1);
        let ptr = self.ptr;
        let rd = |s: &mut Self, st: &mut State, o: &Opnd, pend: &mut Vec<Pending>, out: &mut Vec<Stmt>| s.read(st, o, ins, pend, out);

        // rep stos / rep movs
        if mn.starts_with("rep") && (mn.contains("stos") || mn.contains("movs")) {
            let unit: i64 = match mn.chars().last() {
                Some('b') => 1,
                Some('w') => 2,
                Some('d') => 4,
                _ => 8,
            };
            let dst = self.rd_reg(st, RegRef { r: RDI, size: ptr, high: false });
            let cnt = self.rd_reg(st, RegRef { r: RCX, size: ptr, high: false });
            let n = bin(Bo::Mul, cnt, E::C(unit));
            self.flush(st, pend, out);
            let (name, src) = if mn.contains("stos") {
                ("memset", self.rd_reg(st, RegRef { r: RAX, size: 1, high: false }))
            } else {
                ("memcpy", self.rd_reg(st, RegRef { r: RSI, size: ptr, high: false }))
            };
            out.push(Stmt { addr: ins.addr, st: St::Call(CallE { addr: ins.addr, target: E::Sym(name.into()), name: Some(name.into()), args: vec![dst, src, n] }) });
            st.r[RCX] = Some(E::C(0));
            st.r[RDI] = None;
            st.r[RSI] = None;
            return None;
        }

        match mn {
            "nop" | "endbr64" | "endbr32" | "fnop" | "pause" | "prefetcht0" | "prefetchnta" | "lfence" | "mfence" | "sfence" => {}
            "mov" | "movabs" | "movaps" | "movups" | "movdqa" | "movdqu" | "movapd" | "movupd" | "movq" | "movd" | "movss" | "movsd" | "movlpd" | "movhps" | "vmovaps" | "vmovups" | "vmovdqa" | "vmovdqu" => {
                let (Some(d), Some(s)) = (o0, o1) else { return None };
                let v = rd(self, st, s, pend, out);
                self.write(st, d, v, ins, pend, out);
            }
            "movzx" | "movsx" | "movsxd" => {
                let (Some(d), Some(s)) = (o0, o1) else { return None };
                let size = match s {
                    Opnd::R(r) => r.size,
                    Opnd::M(m) => m.size,
                    _ => 4,
                };
                let v = rd(self, st, s, pend, out);
                let v = cast(size, mn != "movzx", v);
                self.write(st, d, v, ins, pend, out);
            }
            "lea" => {
                let (Some(d), Some(Opnd::M(m))) = (o0, o1) else { return None };
                let a = self.addr_of(st, m, ins);
                self.write(st, d, a, ins, pend, out);
            }
            "xchg" => {
                let (Some(a), Some(c)) = (o0, o1) else { return None };
                let va = rd(self, st, a, pend, out);
                let vb = rd(self, st, c, pend, out);
                self.write(st, a, vb, ins, pend, out);
                self.write(st, c, va, ins, pend, out);
            }
            "add" | "sub" | "and" | "or" | "xor" | "imul" | "shl" | "sal" | "shr" | "sar" | "adc" | "sbb" | "pxor" | "xorps" | "xorpd" | "vpxor" | "vxorps" => {
                let d = o0?;
                // imul de 3 operandos
                if mn == "imul" && ops.len() == 3 {
                    let a = rd(self, st, &ops[1], pend, out);
                    let c = rd(self, st, &ops[2], pend, out);
                    let v = bin(Bo::Mul, a, c);
                    st.fl = Fl::Res(v.clone());
                    self.write(st, d, v, ins, pend, out);
                    return None;
                }
                if mn == "imul" && ops.len() == 1 {
                    let a = self.rd_reg(st, RegRef { r: RAX, size: ptr, high: false });
                    let c = rd(self, st, d, pend, out);
                    st.r[RAX] = Some(bin(Bo::Mul, a, c));
                    st.r[RDX] = None;
                    return None;
                }
                let s = if ops.len() >= 3 { ops.get(2) } else { o1 };
                let s = s?;
                // xor r, r  /  sub r, r  /  pxor x, x
                let same = match (d, s) {
                    (Opnd::R(a), Opnd::R(c)) => a.r == c.r,
                    _ => false,
                };
                if same && matches!(mn, "xor" | "sub" | "pxor" | "xorps" | "xorpd" | "vpxor" | "vxorps") {
                    self.write(st, d, E::C(0), ins, pend, out);
                    st.fl = Fl::Res(E::C(0));
                    return None;
                }
                if mn == "sbb" || mn == "adc" {
                    self.fresh(st, out, ins);
                    return None;
                }
                let a = rd(self, st, d, pend, out);
                let c = rd(self, st, s, pend, out);
                // `and rsp, -16`: alinhamento, ignora.
                if matches!(d, Opnd::R(r) if r.r == RSP) && mn == "and" {
                    return None;
                }
                let v = match mn {
                    "add" => bin(Bo::Add, a.clone(), c.clone()),
                    "sub" => sub(a.clone(), c.clone()),
                    "and" => bin(Bo::And, a.clone(), c.clone()),
                    "or" => bin(Bo::Or, a.clone(), c.clone()),
                    "xor" | "pxor" | "xorps" | "xorpd" | "vpxor" | "vxorps" => bin(Bo::Xor, a.clone(), c.clone()),
                    "imul" => bin(Bo::Mul, a.clone(), c.clone()),
                    "shl" | "sal" => bin(Bo::Shl, a.clone(), c.clone()),
                    "shr" => bin(Bo::Shr, a.clone(), c.clone()),
                    _ => bin(Bo::Sar, a.clone(), c.clone()),
                };
                st.fl = if mn == "sub" { Fl::Cmp(a, c) } else { Fl::Res(v.clone()) };
                self.write(st, d, v, ins, pend, out);
            }
            "inc" | "dec" | "neg" | "not" => {
                let d = o0?;
                let a = rd(self, st, d, pend, out);
                let v = match mn {
                    "inc" => bin(Bo::Add, a, E::C(1)),
                    "dec" => bin(Bo::Add, a, E::C(-1)),
                    "neg" => E::Un(Uo::Neg, bx(a)),
                    _ => E::Un(Uo::Not, bx(a)),
                };
                if mn != "not" {
                    st.fl = Fl::Res(v.clone());
                }
                self.write(st, d, v, ins, pend, out);
            }
            "cmp" => {
                let (Some(a), Some(c)) = (o0, o1) else { return None };
                let va = rd(self, st, a, pend, out);
                let vb = rd(self, st, c, pend, out);
                st.fl = Fl::Cmp(va, vb);
            }
            "test" => {
                let (Some(a), Some(c)) = (o0, o1) else { return None };
                let va = rd(self, st, a, pend, out);
                let same = match (a, c) {
                    (Opnd::R(x), Opnd::R(y)) => x.r == y.r,
                    _ => false,
                };
                let v = if same { va } else { bin(Bo::And, va, rd(self, st, c, pend, out)) };
                st.fl = Fl::Res(v);
            }
            "cdqe" | "cwde" | "cbw" => {
                let size = match mn {
                    "cdqe" => 4,
                    "cwde" => 2,
                    _ => 1,
                };
                let v = self.rd_reg(st, RegRef { r: RAX, size, high: false });
                st.r[RAX] = Some(cast(size, true, v));
            }
            "cdq" | "cqo" | "cwd" => {
                st.r[RDX] = Some(E::Raw("SIGN".into()));
            }
            "div" | "idiv" => {
                let d = o0?;
                let a = self.rd_reg(st, RegRef { r: RAX, size: ptr, high: false });
                let c = rd(self, st, d, pend, out);
                let signed = mn == "idiv";
                st.r[RAX] = Some(bin(if signed { Bo::Div } else { Bo::UDiv }, a.clone(), c.clone()));
                st.r[RDX] = Some(bin(if signed { Bo::Mod } else { Bo::UMod }, a, c));
            }
            "mul" => {
                let d = o0?;
                let a = self.rd_reg(st, RegRef { r: RAX, size: ptr, high: false });
                let c = rd(self, st, d, pend, out);
                st.r[RAX] = Some(bin(Bo::Mul, a, c));
                st.r[RDX] = None;
            }
            "push" => {
                let s = o0?;
                let v = rd(self, st, s, pend, out);
                let size = match s {
                    Opnd::R(r) => r.size.max(ptr),
                    _ => ptr,
                };
                if let Some(E::Sp(sp)) = st.r[RSP].clone() {
                    let nsp = sp - i64::from(size);
                    st.r[RSP] = Some(E::Sp(nsp));
                    self.stack_store(nsp, size, v, ins.addr, st, pend, out, true);
                }
            }
            "pop" => {
                let d = o0?;
                if let Some(E::Sp(sp)) = st.r[RSP].clone() {
                    let v = match d {
                        Opnd::R(r) if self.callee_saved().contains(&r.r) => E::Saved(r.r),
                        _ => {
                            pend.retain(|p| p.off != sp);
                            let k = self.stack_key(sp);
                            E::V(self.var(k))
                        }
                    };
                    st.r[RSP] = Some(E::Sp(sp + i64::from(ptr)));
                    self.write(st, d, v, ins, pend, out);
                }
            }
            "leave" => {
                if let Some(E::Sp(bp)) = st.r[RBP].clone() {
                    st.r[RSP] = Some(E::Sp(bp + i64::from(ptr)));
                }
                st.r[RBP] = Some(E::Saved(RBP));
            }
            "call" => {
                let t = o0?;
                // __x86.get_pc_thunk.bx: registrador recebe o endereço seguinte.
                if let Some(n) = self.call_target_name(ins) {
                    if let Some(r) = n.strip_prefix("__x86.get_pc_thunk.") {
                        if let Some(rr) = parse_reg(&format!("e{r}")) {
                            st.r[rr.r] = Some(E::C(ins.next as i64));
                            return None;
                        }
                    }
                }
                let target = match t {
                    Opnd::I(a) => self.classify(*a as u64),
                    other => rd(self, st, other, pend, out),
                };
                let (_, noreturn) = self.do_call(st, out, pend, ins, target, false);
                if noreturn {
                    return Some(Term::Stop(String::new()));
                }
            }
            "jmp" => {
                if let Some(ts) = self.f.switches.iter().find(|(a, _)| *a == ins.addr).map(|(_, t)| t.clone()) {
                    let fl = if matches!(st.fl, Fl::Cmp(_, E::C(_))) { st.fl.clone() } else { self.last_cmp.clone() };
                    let (idx, base) = match &fl {
                        Fl::Cmp(x, E::C(_)) => match x {
                            E::Bin(Bo::Add, y, k) => match **k {
                                E::C(k) => ((**y).clone(), -k),
                                _ => (x.clone(), 0),
                            },
                            _ => (x.clone(), 0),
                        },
                        _ => (E::Raw("/* índice */ ?".into()), 0),
                    };
                    let targets = ts.iter().filter_map(|t| self.block_at.get(t).copied()).collect();
                    self.flush(st, pend, out);
                    return Some(Term::Switch(idx, base, targets));
                }
                match o0 {
                    Some(Opnd::I(t)) => {
                        if let Some(&blk) = self.block_at.get(&(*t as u64)) {
                            self.flush(st, pend, out);
                            return Some(Term::Goto(blk));
                        }
                        let target = self.classify(*t as u64);
                        let (c, _) = self.do_call(st, out, pend, ins, target, true);
                        return Some(Term::Tail(c));
                    }
                    Some(other) => {
                        let target = rd(self, st, other, pend, out);
                        if matches!(target, E::Sym(_)) {
                            let (c, _) = self.do_call(st, out, pend, ins, target, true);
                            return Some(Term::Tail(c));
                        }
                        self.flush(st, pend, out);
                        return Some(Term::Stop(format!("salto indireto: {}", ins.text)));
                    }
                    None => return Some(Term::Stop(String::new())),
                }
            }
            m if m.starts_with("ret") => {
                self.flush(st, pend, out);
                let v = match &st.r[RAX] {
                    Some(E::Saved(_)) | None => None,
                    Some(e) => Some(e.clone()),
                };
                return Some(Term::Ret(v));
            }
            "hlt" | "ud2" | "int3" => {
                self.flush(st, pend, out);
                return Some(Term::Stop(format!("{mn}: o programa para aqui")));
            }
            m if is_jcc(m) => {
                self.flush(st, pend, out);
                let c = self.cond(&st.fl, m);
                let succs = &self.blocks[b].succs;
                return Some(match (succs.first(), succs.get(1)) {
                    (Some(&t), Some(&f)) => Term::Br(c, t, f),
                    (Some(&t), None) => Term::Goto(t),
                    _ => Term::Stop(String::new()),
                });
            }
            m if m.starts_with("set") => {
                let d = o0?;
                let c = self.cond(&st.fl, m);
                self.write(st, d, c, ins, pend, out);
            }
            m if m.starts_with("cmov") => {
                let (Some(d), Some(s)) = (o0, o1) else { return None };
                let c = self.cond(&st.fl, m);
                let old = rd(self, st, d, pend, out);
                let new = rd(self, st, s, pend, out);
                self.write(st, d, E::Tern(bx(c), bx(new), bx(old)), ins, pend, out);
            }
            "bt" | "cmc" | "clc" | "stc" | "cld" | "std" => {
                st.fl = Fl::No;
            }
            _ => self.fresh(st, out, ins),
        }
        None
    }

    // ------------------------------------------------------------- fluxo de dados

    fn merge_entry(&mut self, b: usize, entry_block: usize, init: &State) -> Option<State> {
        let mut incoming: Vec<State> = self.blocks[b].preds.iter().filter_map(|p| self.exits[*p].clone()).collect();
        if b == entry_block {
            incoming.insert(0, init.clone());
        }
        let first = incoming.first()?.clone();
        let mut out = first.clone();
        for r in 0..NREG {
            if self.merged.contains(&(b, r)) {
                let v = self.var(VKey::Merge(b, r));
                out.r[r] = Some(E::V(v));
                continue;
            }
            let vals: Vec<&Option<E>> = incoming.iter().map(|s| &s.r[r]).collect();
            if vals.iter().all(|v| *v == vals[0]) {
                continue;
            }
            if vals.iter().any(|v| v.is_none()) {
                out.r[r] = None;
                continue;
            }
            // Ponteiro de pilha diferente (limpeza cdecl não rastreada): fica o primeiro.
            if (r == RSP || r == RBP) && vals.iter().all(|v| matches!(v, Some(E::Sp(_)))) {
                continue;
            }
            self.merged.insert((b, r));
            let v = self.var(VKey::Merge(b, r));
            out.r[r] = Some(E::V(v));
        }
        if !incoming.iter().all(|s| s.fl == first.fl) {
            out.fl = Fl::No;
        }
        Some(out)
    }

    fn dataflow(&mut self) {
        let Some(&entry) = self.block_at.get(&self.f.start) else { return };
        let order = self.rpo(entry);
        let init = self.initial_state();
        let mut entries: Vec<Option<State>> = vec![None; self.blocks.len()];
        for _ in 0..MAX_ITER {
            let mut changed = false;
            for &b in &order {
                let Some(e) = self.merge_entry(b, entry, &init) else { continue };
                if entries[b].as_ref() == Some(&e) && self.lifted[b].is_some() {
                    continue;
                }
                let l = self.lift(b, e.clone());
                if self.exits[b].as_ref() != Some(&l.exit) {
                    changed = true;
                }
                entries[b] = Some(e);
                self.exits[b] = Some(l.exit.clone());
                self.lifted[b] = Some(l);
            }
            if !changed {
                break;
            }
        }
    }

    /// As cópias de uma aresta acontecem "ao mesmo tempo" (`a = b; b = a` troca).
    /// Ordena para que nenhuma sobrescreva um valor ainda lido; ciclo → temporário.
    fn sequentialize_copies(&mut self) {
        for p in 0..self.blocks.len() {
            for s in self.blocks[p].succs.clone() {
                let mut pending = self.copies(p, s);
                if pending.len() < 2 {
                    self.seq_copies.insert((p, s), pending);
                    continue;
                }
                let mut out = vec![];
                while !pending.is_empty() {
                    let free = (0..pending.len()).find(|&i| {
                        let target = E::V(pending[i].0);
                        !pending.iter().enumerate().any(|(j, (_, e))| j != i && e.any(&|x| *x == target))
                    });
                    match free {
                        Some(i) => out.push(pending.remove(i)),
                        None => {
                            // Ciclo: guarda o valor antigo de um alvo num temporário.
                            let (v, _) = pending[0].clone();
                            let t = self.var(VKey::Temp(self.blocks[s].start, 1000 + v as usize));
                            out.push((t, E::V(v)));
                            for (_, e) in pending.iter_mut() {
                                let old = std::mem::replace(e, E::C(0));
                                *e = old.map(&mut |x| if x == E::V(v) { E::V(t) } else { x });
                            }
                        }
                    }
                }
                self.seq_copies.insert((p, s), out);
            }
        }
    }

    /// Cópias que a aresta `p → s` precisa fazer (variáveis de junção), em paralelo.
    fn copies(&self, p: usize, s: usize) -> Vec<(u32, E)> {
        let Some(exit) = &self.exits[p] else { return vec![] };
        let mut v = vec![];
        for r in 0..NREG {
            if !self.merged.contains(&(s, r)) {
                continue;
            }
            let Some(&id) = self.var_id.get(&VKey::Merge(s, r)) else { continue };
            if let Some(val) = &exit.r[r] {
                if *val != E::V(id) {
                    v.push((id, val.clone()));
                }
            }
        }
        v
    }
}

impl Func {
    /// Cópia leve (sem os caches de linhas) para decodificar só um trecho.
    fn shallow(&self) -> Func {
        Func {
            start: self.start,
            name: self.name.clone(),
            insns: vec![],
            blocks: Default::default(),
            calls: vec![],
            code_refs: vec![],
            switches: vec![],
            disasm: Default::default(),
            pseudo: Default::default(),
            hex: Default::default(),
        }
    }
}

// ===================================================================== estruturação

#[derive(Debug, Clone)]
enum Loop {
    Forever,
    While(E),
    DoWhile(E),
    /// `for (init; cond; passo)`.
    For(Box<Stmt>, E, Box<Stmt>),
}

#[derive(Debug, Clone)]
enum Node {
    S(Stmt),
    If(u64, E, Vec<Node>, Vec<Node>),
    Loop(u64, Loop, Vec<Node>),
    Switch(u64, E, Vec<(Vec<i64>, Vec<Node>)>, Option<Vec<Node>>),
    Ret(u64, Option<E>),
    Break,
    Continue,
    Goto(usize),
    Label(usize),
    Comment(u64, String),
}

struct LoopCtx {
    header: usize,
    follow: Option<usize>,
    body: HashSet<usize>,
}

struct Structurer<'c, 'a> {
    cx: &'c Ctx<'a>,
    emitted: HashSet<usize>,
    gotos: HashSet<usize>,
    ipdom: Vec<Option<usize>>,
    loops: HashMap<usize, HashSet<usize>>,
    /// Último bloco de parada alcançado pela região (para `switch`).
    reached: Option<usize>,
}

fn idoms(n: usize, entry: usize, succs: &[Vec<usize>], preds: &[Vec<usize>]) -> Vec<Option<usize>> {
    // Cooper, Harvey, Kennedy.
    let mut order = vec![];
    let mut seen = vec![false; n];
    let mut stack = vec![(entry, 0usize)];
    seen[entry] = true;
    while let Some((b, i)) = stack.pop() {
        if i < succs[b].len() {
            stack.push((b, i + 1));
            let s = succs[b][i];
            if !seen[s] {
                seen[s] = true;
                stack.push((s, 0));
            }
        } else {
            order.push(b);
        }
    }
    order.reverse();
    let mut rpo_num = vec![usize::MAX; n];
    for (i, &b) in order.iter().enumerate() {
        rpo_num[b] = i;
    }
    let mut idom: Vec<Option<usize>> = vec![None; n];
    idom[entry] = Some(entry);
    let mut changed = true;
    while changed {
        changed = false;
        for &b in order.iter().skip(1) {
            let mut new: Option<usize> = None;
            for &p in &preds[b] {
                if idom[p].is_none() {
                    continue;
                }
                new = Some(match new {
                    None => p,
                    Some(mut a) => {
                        let mut c = p;
                        while a != c {
                            while rpo_num[a] > rpo_num[c] {
                                a = idom[a].unwrap();
                            }
                            while rpo_num[c] > rpo_num[a] {
                                c = idom[c].unwrap();
                            }
                        }
                        a
                    }
                });
            }
            if new.is_some() && idom[b] != new {
                idom[b] = new;
                changed = true;
            }
        }
    }
    idom
}

impl<'c, 'a> Structurer<'c, 'a> {
    fn run(cx: &'c Ctx<'a>, entry: usize) -> Vec<Node> {
        let n = cx.blocks.len();
        let succs: Vec<Vec<usize>> = cx.blocks.iter().map(|b| b.succs.clone()).collect();
        let preds: Vec<Vec<usize>> = cx.blocks.iter().map(|b| b.preds.clone()).collect();
        let dom = idoms(n, entry, &succs, &preds);
        // Pós-dominadores: grafo invertido com uma saída virtual (índice n).
        let mut rs: Vec<Vec<usize>> = preds.clone();
        let mut rp: Vec<Vec<usize>> = succs.clone();
        rs.push(vec![]);
        rp.push(vec![]);
        for b in 0..n {
            if succs[b].is_empty() {
                rs[n].push(b);
                rp[b].push(n);
            }
        }
        let pdom = idoms(n + 1, n, &rs, &rp);
        let ipdom: Vec<Option<usize>> = (0..n).map(|b| pdom[b].filter(|&d| d != n && d != b)).collect();
        // Laços naturais.
        let dominates = |a: usize, mut b: usize| -> bool {
            loop {
                if a == b {
                    return true;
                }
                match dom[b] {
                    Some(d) if d != b => b = d,
                    _ => return false,
                }
            }
        };
        let mut loops: HashMap<usize, HashSet<usize>> = HashMap::new();
        for b in 0..n {
            if dom[b].is_none() {
                continue;
            }
            for &h in &succs[b] {
                if dominates(h, b) {
                    let body = loops.entry(h).or_insert_with(|| HashSet::from([h]));
                    let mut work = vec![b];
                    while let Some(x) = work.pop() {
                        if body.insert(x) {
                            work.extend(preds[x].iter().copied().filter(|p| dom[*p].is_some()));
                        }
                    }
                }
            }
        }
        let mut s = Structurer { cx, emitted: HashSet::new(), gotos: HashSet::new(), ipdom, loops, reached: None };
        let mut out = s.region(entry, None, &HashSet::new(), None);
        // Blocos alcançados só por goto e ainda não impressos.
        loop {
            let pending: Vec<usize> = s.gotos.iter().copied().filter(|g| !s.emitted.contains(g)).collect();
            let Some(&g) = pending.iter().min() else { break };
            let mut more = s.region(g, None, &HashSet::new(), None);
            out.append(&mut more);
        }
        let gotos = s.gotos.clone();
        strip_labels(out, &gotos)
    }

    fn lifted(&self, b: usize) -> &Lifted {
        self.cx.lifted[b].as_ref().expect("bloco levantado")
    }

    /// Bloco pequeno que termina a função: pode ser repetido em vez de virar goto.
    fn dupable(&self, b: usize) -> bool {
        let Some(l) = self.cx.lifted[b].as_ref() else { return false };
        !self.loops.contains_key(&b) && l.stmts.len() <= 2 && matches!(l.term, Term::Ret(_) | Term::Tail(_))
    }

    fn dup(&self, from: Option<usize>, b: usize) -> Vec<Node> {
        let subst: HashMap<u32, E> = from.map(|p| self.cx.copies(p, b)).unwrap_or_default().into_iter().collect();
        let mut f = |e: E| match e {
            E::V(v) => subst.get(&v).cloned().unwrap_or(E::V(v)),
            other => other,
        };
        let l = self.lifted(b);
        let mut out: Vec<Node> = l.stmts.iter().cloned().map(|s| Node::S(s.map_exprs(&mut f))).collect();
        let last = self.cx.ins[*self.cx.blocks[b].ins.last().unwrap()].addr;
        match &l.term {
            Term::Ret(e) => out.push(Node::Ret(last, e.clone().map(|e| e.map(&mut f)))),
            Term::Tail(c) => out.push(Node::Ret(last, Some(E::Call(Box::new(c.clone())).map(&mut f)))),
            _ => {}
        }
        out
    }

    fn emit_copies(&self, from: Option<usize>, to: usize, out: &mut Vec<Node>) {
        let Some(p) = from else { return };
        let copies = self.cx.seq_copies.get(&(p, to)).cloned().unwrap_or_else(|| self.cx.copies(p, to));
        for (v, e) in copies {
            let addr = self.cx.ins[*self.cx.blocks[p].ins.last().unwrap()].addr;
            out.push(Node::S(Stmt { addr, st: St::Set(v, e) }));
        }
    }

    fn follow_of(&self, b: usize, lp: Option<&LoopCtx>) -> Option<usize> {
        let f = self.ipdom[b]?;
        if let Some(l) = lp {
            if !l.body.contains(&f) || f == l.header {
                return None;
            }
        }
        if self.emitted.contains(&f) || self.dupable(f) {
            return None;
        }
        Some(f)
    }

    fn region(&mut self, start: usize, from: Option<usize>, stop: &HashSet<usize>, lp: Option<&LoopCtx>) -> Vec<Node> {
        let mut out = vec![];
        let mut cur = start;
        let mut prev = from;
        let mut at_header = lp.is_some_and(|l| l.header == start) && from.is_none();
        loop {
            if let Some(l) = lp {
                if cur == l.header && !at_header {
                    self.emit_copies(prev, cur, &mut out);
                    out.push(Node::Continue);
                    return out;
                }
                if Some(cur) == l.follow {
                    self.emit_copies(prev, cur, &mut out);
                    out.push(Node::Break);
                    return out;
                }
            }
            at_header = false;
            if stop.contains(&cur) {
                self.emit_copies(prev, cur, &mut out);
                self.reached = Some(cur);
                return out;
            }
            let outside_loop = lp.is_some_and(|l| !l.body.contains(&cur));
            if (self.emitted.contains(&cur) || outside_loop) || (prev.is_some() && self.dupable(cur)) {
                if self.dupable(cur) {
                    out.extend(self.dup(prev, cur));
                } else {
                    self.emit_copies(prev, cur, &mut out);
                    out.push(Node::Goto(cur));
                    self.gotos.insert(cur);
                }
                self.reached = None;
                return out;
            }
            self.emit_copies(prev, cur, &mut out);
            // Início de laço.
            if self.loops.contains_key(&cur) && lp.map_or(true, |l| l.header != cur) {
                let body = self.loops[&cur].clone();
                let follow = self.loop_follow(cur, &body);
                let inner = LoopCtx { header: cur, follow, body };
                let addr = self.cx.blocks[cur].start;
                let nodes = self.region(cur, None, &HashSet::new(), Some(&inner));
                out.push(Node::Loop(addr, Loop::Forever, nodes));
                match follow {
                    Some(f) if !self.emitted.contains(&f) => {
                        prev = None;
                        cur = f;
                        continue;
                    }
                    Some(f) => {
                        out.push(Node::Goto(f));
                        self.gotos.insert(f);
                        return out;
                    }
                    None => {
                        self.reached = None;
                        return out;
                    }
                }
            }
            self.emitted.insert(cur);
            out.push(Node::Label(cur));
            let l = self.lifted(cur).clone();
            out.extend(l.stmts.iter().cloned().map(Node::S));
            let last = self.cx.ins[*self.cx.blocks[cur].ins.last().unwrap()].addr;
            match l.term {
                Term::Goto(t) => {
                    prev = Some(cur);
                    cur = t;
                }
                Term::Ret(e) => {
                    out.push(Node::Ret(last, e));
                    self.reached = None;
                    return out;
                }
                Term::Tail(c) => {
                    out.push(Node::Ret(last, Some(E::Call(Box::new(c)))));
                    self.reached = None;
                    return out;
                }
                Term::Stop(msg) => {
                    if !msg.is_empty() {
                        out.push(Node::Comment(last, msg));
                    }
                    self.reached = None;
                    return out;
                }
                Term::Br(c, t, f) => {
                    // switch: cadeia de comparações ou checagem de limite + tabela.
                    if let Some(sw) = self.chain(cur) {
                        let next = self.emit_switch(cur, last, sw, stop, lp, &mut out);
                        match next {
                            Some(n) => {
                                prev = None;
                                cur = n;
                                continue;
                            }
                            None => return out,
                        }
                    }
                    if let Some(sw) = self.bounded_table(cur, &c, t, f) {
                        let next = self.emit_switch(cur, last, sw, stop, lp, &mut out);
                        match next {
                            Some(n) => {
                                prev = None;
                                cur = n;
                                continue;
                            }
                            None => return out,
                        }
                    }
                    let follow = self.follow_of(cur, lp);
                    let mut st2 = stop.clone();
                    if let Some(f) = follow {
                        st2.insert(f);
                    }
                    let then = self.region(t, Some(cur), &st2, lp);
                    let els = self.region(f, Some(cur), &st2, lp);
                    out.push(Node::If(last, c, then, els));
                    match follow {
                        Some(fl) => {
                            prev = None;
                            cur = fl;
                        }
                        None => {
                            self.reached = None;
                            return out;
                        }
                    }
                }
                Term::Switch(idx, base, targets) => {
                    let mut cases: Vec<(i64, usize, usize)> = vec![];
                    for (i, &t) in targets.iter().enumerate() {
                        cases.push((base + i as i64, cur, t));
                    }
                    let sw = SwitchPlan { idx, cases, default: None, head: cur };
                    match self.emit_switch(cur, last, sw, stop, lp, &mut out) {
                        Some(n) => {
                            prev = None;
                            cur = n;
                        }
                        None => return out,
                    }
                }
            }
        }
    }

    fn loop_follow(&self, h: usize, body: &HashSet<usize>) -> Option<usize> {
        let mut exits: Vec<usize> = vec![];
        for &b in body {
            for &s in &self.cx.blocks[b].succs {
                if !body.contains(&s) && !exits.contains(&s) {
                    exits.push(s);
                }
            }
        }
        exits.sort_unstable();
        // Saída direta do cabeçalho (while) tem preferência.
        if let Some(&s) = self.cx.blocks[h].succs.iter().find(|s| !body.contains(s)) {
            return Some(s);
        }
        let non_dup: Vec<usize> = exits.iter().copied().filter(|e| !self.dupable(*e)).collect();
        non_dup.first().or(exits.first()).copied()
    }

    /// Cadeia `if (x == A) ... else if (x == B) ...` com 3+ valores → switch.
    fn chain(&self, head: usize) -> Option<SwitchPlan> {
        let x = match &self.lifted(head).term {
            Term::Br(E::Cmp(_, a, b), _, _) if matches!(**b, E::C(_)) => (**a).clone(),
            _ => return None,
        };
        let forward = |mut b: usize| -> usize {
            for _ in 0..4 {
                match self.cx.lifted[b].as_ref() {
                    Some(l) if l.stmts.is_empty() && self.cx.copies(b, 0).is_empty() => match l.term {
                        Term::Goto(t) if !self.cx.blocks[t].preds.is_empty() => b = t,
                        _ => break,
                    },
                    _ => break,
                }
            }
            b
        };
        let chainable = |b: usize| -> bool {
            b != head
                && self.cx.blocks[b].preds.len() == 1
                && self.cx.lifted[b].as_ref().is_some_and(|l| {
                    l.stmts.is_empty() && matches!(&l.term, Term::Br(E::Cmp(_, a, k), _, _) if **a == x && matches!(**k, E::C(_)))
                })
        };
        let mut cases: Vec<(i64, usize, usize)> = vec![];
        let mut others: Vec<(usize, usize)> = vec![];
        let mut work = vec![head];
        let mut seen = HashSet::new();
        while let Some(b) = work.pop() {
            if !seen.insert(b) || seen.len() > 64 {
                return None;
            }
            let Term::Br(E::Cmp(op, _, k), t, f) = &self.lifted(b).term else { return None };
            let E::C(k) = **k else { return None };
            match op {
                Co::Eq | Co::Ne => {
                    let (hit, miss) = if *op == Co::Eq { (*t, *f) } else { (*f, *t) };
                    cases.push((k, b, hit));
                    if chainable(miss) {
                        work.push(miss);
                    } else {
                        others.push((b, miss));
                    }
                }
                _ => {
                    if chainable(*t) && chainable(*f) {
                        work.push(*t);
                        work.push(*f);
                    } else if chainable(*t) {
                        work.push(*t);
                        others.push((b, *f));
                    } else if chainable(*f) {
                        work.push(*f);
                        others.push((b, *t));
                    } else {
                        return None;
                    }
                }
            }
        }
        let distinct: HashSet<i64> = cases.iter().map(|c| c.0).collect();
        if distinct.len() < 3 {
            return None;
        }
        let defaults: HashSet<usize> = others.iter().map(|(_, d)| forward(*d)).collect();
        if defaults.len() > 1 {
            return None;
        }
        let default = others.first().map(|&(b, d)| (b, d));
        cases.sort_by_key(|c| c.0);
        cases.dedup_by_key(|c| c.0);
        Some(SwitchPlan { idx: x, cases, default, head })
    }

    /// `if (x > N) default; else switch tabela`.
    fn bounded_table(&self, cur: usize, c: &E, t: usize, f: usize) -> Option<SwitchPlan> {
        let is_table = |b: usize| -> bool {
            self.cx.blocks[b].preds.len() == 1 && self.cx.lifted[b].as_ref().is_some_and(|l| l.stmts.is_empty() && matches!(l.term, Term::Switch(..)))
        };
        let (table, default) = if is_table(f) { (f, t) } else if is_table(t) { (t, f) } else { return None };
        let _ = c;
        let Term::Switch(idx, base, targets) = &self.lifted(table).term else { return None };
        let cases = targets.iter().enumerate().filter(|(_, &tg)| tg != default).map(|(i, &tg)| (base + i as i64, table, tg)).collect();
        Some(SwitchPlan { idx: idx.clone(), cases, default: Some((cur, default)), head: cur })
    }

    fn emit_switch(&mut self, head: usize, addr: u64, sw: SwitchPlan, stop: &HashSet<usize>, lp: Option<&LoopCtx>, out: &mut Vec<Node>) -> Option<usize> {
        let follow = self.follow_of(head, lp);
        // Agrupa valores com o mesmo destino, na ordem do primeiro valor.
        let mut groups: Vec<(Vec<i64>, usize, usize)> = vec![];
        for (v, from, t) in &sw.cases {
            if let Some(g) = groups.iter_mut().find(|g| g.2 == *t) {
                g.0.push(*v);
            } else {
                groups.push((vec![*v], *from, *t));
            }
        }
        let targets: HashSet<usize> = groups.iter().map(|g| g.2).collect();
        let mut arms = vec![];
        for i in 0..groups.len() {
            let (vals, from, t) = groups[i].clone();
            let mut st2 = stop.clone();
            if let Some(f) = follow {
                st2.insert(f);
            }
            st2.extend(targets.iter().copied().filter(|x| *x != t));
            self.reached = None;
            let mut nodes = if Some(t) == follow { vec![] } else { self.region(t, Some(from), &st2, lp) };
            let next_target = groups.get(i + 1).map(|g| g.2);
            if Some(t) == follow || (self.reached.is_some() && self.reached == follow) {
                nodes.push(Node::Break);
            } else if let Some(r) = self.reached {
                if Some(r) != next_target && targets.contains(&r) {
                    nodes.push(Node::Goto(r));
                    self.gotos.insert(r);
                }
            }
            arms.push((vals, nodes));
        }
        let mut after: Vec<Node> = vec![];
        let mut default_nodes = None;
        if let Some((from, d)) = sw.default {
            if Some(d) != follow {
                let mut st2 = stop.clone();
                if let Some(f) = follow {
                    st2.insert(f);
                }
                self.reached = None;
                let mut nodes = self.region(d, Some(from), &st2, lp);
                if follow.is_none() && ends_with_jump(&nodes) {
                    after = nodes; // todos os casos retornam: o "default" fica depois do switch
                } else {
                    if self.reached.is_some() && self.reached == follow {
                        nodes.push(Node::Break);
                    }
                    default_nodes = Some(nodes);
                }
            }
        }
        for (_, n) in arms.iter_mut() {
            if follow.is_none() && n.is_empty() {
                n.push(Node::Break);
            }
        }
        out.push(Node::Switch(addr, sw.idx, arms, default_nodes));
        out.extend(after);
        let f = follow?;
        if self.emitted.contains(&f) {
            return None;
        }
        Some(f)
    }
}

struct SwitchPlan {
    idx: E,
    cases: Vec<(i64, usize, usize)>,
    default: Option<(usize, usize)>,
    #[allow(dead_code)]
    head: usize,
}

fn ends_with_jump(nodes: &[Node]) -> bool {
    match nodes.last() {
        Some(Node::Ret(..)) | Some(Node::Goto(_)) | Some(Node::Break) | Some(Node::Continue) => true,
        Some(Node::If(_, _, t, e)) => ends_with_jump(t) && ends_with_jump(e),
        Some(Node::Comment(..)) => true,
        _ => false,
    }
}

fn strip_labels(nodes: Vec<Node>, keep: &HashSet<usize>) -> Vec<Node> {
    nodes
        .into_iter()
        .filter_map(|n| match n {
            Node::Label(b) if !keep.contains(&b) => None,
            Node::If(a, c, t, e) => Some(Node::If(a, c, strip_labels(t, keep), strip_labels(e, keep))),
            Node::Loop(a, k, b) => Some(Node::Loop(a, k, strip_labels(b, keep))),
            Node::Switch(a, x, arms, d) => Some(Node::Switch(
                a,
                x,
                arms.into_iter().map(|(v, b)| (v, strip_labels(b, keep))).collect(),
                d.map(|d| strip_labels(d, keep)),
            )),
            other => Some(other),
        })
        .collect()
}

// ===================================================================== limpeza

fn node_exprs<'n>(n: &'n Node, out: &mut Vec<&'n E>) {
    match n {
        Node::S(s) => out.extend(s.exprs()),
        Node::If(_, c, t, e) => {
            out.push(c);
            for x in t.iter().chain(e.iter()) {
                node_exprs(x, out);
            }
        }
        Node::Loop(_, k, b) => {
            if let Loop::While(c) | Loop::DoWhile(c) = k {
                out.push(c);
            }
            if let Loop::For(i, c, st) = k {
                out.extend(i.exprs());
                out.push(c);
                out.extend(st.exprs());
            }
            for x in b {
                node_exprs(x, out);
            }
        }
        Node::Switch(_, x, arms, d) => {
            out.push(x);
            for (_, b) in arms {
                for y in b {
                    node_exprs(y, out);
                }
            }
            if let Some(d) = d {
                for y in d {
                    node_exprs(y, out);
                }
            }
        }
        Node::Ret(_, Some(e)) => out.push(e),
        _ => {}
    }
}

/// Expressões avaliadas primeiro por um nó (onde uma chamada pode ser embutida).
fn head_exprs_mut(n: &mut Node) -> Vec<&mut E> {
    match n {
        Node::S(s) => match &mut s.st {
            St::Set(_, e) => vec![e],
            St::Store(a, v, _) => vec![v, a],
            St::Call(c) => {
                let mut v: Vec<&mut E> = c.args.iter_mut().collect();
                v.push(&mut c.target);
                v
            }
            St::Asm(_) => vec![],
        },
        Node::If(_, c, _, _) => vec![c],
        Node::Switch(_, x, _, _) => vec![x],
        Node::Ret(_, Some(e)) => vec![e],
        _ => vec![],
    }
}

fn map_nodes(nodes: Vec<Node>, f: &mut dyn FnMut(E) -> E) -> Vec<Node> {
    nodes
        .into_iter()
        .map(|n| match n {
            Node::S(s) => Node::S(s.map_exprs(f)),
            Node::If(a, c, t, e) => {
                let c = c.map(f);
                Node::If(a, c, map_nodes(t, f), map_nodes(e, f))
            }
            Node::Loop(a, k, b) => {
                let k = match k {
                    Loop::While(c) => Loop::While(c.map(f)),
                    Loop::DoWhile(c) => Loop::DoWhile(c.map(f)),
                    Loop::For(i, c, st) => Loop::For(Box::new(i.map_exprs(f)), c.map(f), Box::new(st.map_exprs(f))),
                    Loop::Forever => Loop::Forever,
                };
                Node::Loop(a, k, map_nodes(b, f))
            }
            Node::Switch(a, x, arms, d) => {
                let x = x.map(f);
                Node::Switch(a, x, arms.into_iter().map(|(v, b)| (v, map_nodes(b, f))).collect(), d.map(|d| map_nodes(d, f)))
            }
            Node::Ret(a, e) => Node::Ret(a, e.map(|e| e.map(f))),
            other => other,
        })
        .collect()
}

fn count_refs(nodes: &[Node]) -> (HashMap<u64, usize>, HashMap<u32, usize>) {
    let mut res: HashMap<u64, usize> = HashMap::new();
    let mut vars: HashMap<u32, usize> = HashMap::new();
    let mut all = vec![];
    for n in nodes {
        node_exprs(n, &mut all);
    }
    for e in all {
        e.walk(&mut |x| match x {
            E::Res(a) => *res.entry(*a).or_default() += 1,
            E::V(v) => *vars.entry(*v).or_default() += 1,
            _ => {}
        });
    }
    (res, vars)
}

/// Variáveis atribuídas (Set) em todo o AST.
fn assigned(nodes: &[Node], out: &mut HashMap<u32, usize>) {
    for n in nodes {
        match n {
            Node::S(Stmt { st: St::Set(v, _), .. }) => *out.entry(*v).or_default() += 1,
            Node::If(_, _, t, e) => {
                assigned(t, out);
                assigned(e, out);
            }
            Node::Loop(_, _, b) => assigned(b, out),
            Node::Switch(_, _, arms, d) => {
                for (_, b) in arms {
                    assigned(b, out);
                }
                if let Some(d) = d {
                    assigned(d, out);
                }
            }
            _ => {}
        }
    }
}

/// Chamadas cujo retorno é usado uma única vez, logo no comando seguinte,
/// entram na expressão: `x = f(); if (x > 0)` → `if (f() > 0)`.
fn inline_calls(nodes: Vec<Node>, counts: &HashMap<u64, usize>, movable: &dyn Fn(&Node, &CallE) -> bool) -> Vec<Node> {
    let mut v: Vec<Node> = nodes
        .into_iter()
        .map(|n| match n {
            Node::If(a, c, t, e) => Node::If(a, c, inline_calls(t, counts, movable), inline_calls(e, counts, movable)),
            Node::Loop(a, k, b) => Node::Loop(a, k, inline_calls(b, counts, movable)),
            Node::Switch(a, x, arms, d) => Node::Switch(
                a,
                x,
                arms.into_iter().map(|(v, b)| (v, inline_calls(b, counts, movable))).collect(),
                d.map(|d| inline_calls(d, counts, movable)),
            ),
            other => other,
        })
        .collect();
    let mut i = 0;
    while i + 1 < v.len() {
        let call = match &v[i] {
            Node::S(Stmt { st: St::Call(c), .. }) if counts.get(&c.addr) == Some(&1) => Some(c.clone()),
            _ => None,
        };
        if let Some(c) = call {
            let mut done = false;
            // A chamada pode "pular" atribuições simples a locais que não a usam.
            let mut j = i + 1;
            while j < v.len() && j < i + 4 {
                let mut refs = vec![];
                node_exprs(&v[j], &mut refs);
                if refs.iter().any(|e| e.any(&|x| *x == E::Res(c.addr))) || !movable(&v[j], &c) {
                    break;
                }
                j += 1;
            }
            // Handle com nome (`hWnd = CreateWindowEx(...)`) não vira argumento de outra chamada.
            if j >= v.len() || (matches!(&v[j], Node::S(Stmt { st: St::Call(_), .. })) && c.name.as_deref().and_then(winapi::lookup).is_some_and(|a| a.ret_name.is_some())) {
                i += 1;
                continue;
            }
            for e in head_exprs_mut(&mut v[j]) {
                if e.any(&|x| *x == E::Res(c.addr)) {
                    let ce = E::Call(Box::new(c.clone()));
                    let taken = std::mem::replace(e, E::C(0));
                    *e = taken.map(&mut |x| if x == E::Res(c.addr) { ce.clone() } else { x });
                    done = true;
                    break;
                }
            }
            if done {
                v.remove(i);
                continue;
            }
        }
        i += 1;
    }
    v
}

fn tidy(nodes: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = vec![];
    for n in nodes {
        match n {
            Node::If(a, c, t, e) => {
                let t = tidy(t);
                let e = tidy(e);
                if t.is_empty() && e.is_empty() {
                    if c.has_side_effects() {
                        out.push(Node::S(Stmt { addr: a, st: St::Set(u32::MAX, c) }));
                    }
                } else if t.is_empty() {
                    out.push(Node::If(a, negate(c), e, vec![]));
                } else if let (Some(Node::Ret(ra, r)), false) = (t.last(), e.is_empty()) {
                    // Mesmo `return` no fim de todos os ramos: sai uma vez, depois do if.
                    let (ra, r) = (*ra, r.clone());
                    let mut node = vec![Node::If(a, c.clone(), t.clone(), e.clone())];
                    if ends_ret(&node, &r) {
                        strip_ret(&mut node);
                        out.extend(tidy(node));
                        out.push(Node::Ret(ra, r));
                    } else if matches!(c, E::Cmp(Co::Ne, ..)) && !matches!(e.as_slice(), [Node::If(..)]) {
                        out.push(Node::If(a, negate(c), e, t));
                    } else {
                        out.push(Node::If(a, c, t, e));
                    }
                } else if !e.is_empty() && matches!(c, E::Cmp(Co::Ne, ..)) && !matches!(e.as_slice(), [Node::If(..)]) {
                    // `if (x != 1) A else B` lê melhor como `if (x == 1) B else A`.
                    out.push(Node::If(a, negate(c), e, t));
                } else {
                    out.push(Node::If(a, c, t, e));
                }
                // Retorno antecipado: `if (c) { return x; } else { resto }` → `if (c) return x; resto`.
                if let Some(Node::If(a, c, t, e)) = out.last().cloned() {
                    if !e.is_empty() && !matches!(e.as_slice(), [Node::If(..)]) {
                        let (tj, ej) = (ends_with_jump(&t), ends_with_jump(&e));
                        if tj && (!ej || t.len() <= e.len()) {
                            out.pop();
                            out.push(Node::If(a, c, t, vec![]));
                            out.extend(e);
                        } else if ej {
                            out.pop();
                            out.push(Node::If(a, negate(c), e, vec![]));
                            out.extend(t);
                        }
                    }
                }
            }
            Node::Loop(a, Loop::Forever, b) => {
                let mut b = tidy(b);
                while matches!(b.last(), Some(Node::Continue)) {
                    b.pop();
                }
                // while (true) { if (c) { A } else break; }  →  while (c) { A }
                if b.len() == 1 {
                    if let Some(Node::If(_, c, t, e)) = b.first() {
                        let (c, mut body) = match (t.as_slice(), e.as_slice()) {
                            (_, [Node::Break]) => (c.clone(), t.clone()),
                            ([Node::Break], _) => (negate(c.clone()), e.clone()),
                            _ => (E::C(0), vec![]),
                        };
                        if c != E::C(0) && !body.iter().any(has_break) {
                            while matches!(body.last(), Some(Node::Continue)) {
                                body.pop();
                            }
                            out.push(Node::Loop(a, Loop::While(c), body));
                            continue;
                        }
                    }
                }
                // while (true) { if (c) break; ... }  →  while (!c) { ... }
                if let Some(Node::If(_, c, t, e)) = b.first() {
                    if e.is_empty() && matches!(t.as_slice(), [Node::Break]) {
                        let c = negate(c.clone());
                        b.remove(0);
                        out.push(Node::Loop(a, Loop::While(c), b));
                        continue;
                    }
                }
                // while (true) { ...; if (c) continue; break; }  →  do { ... } while (c);
                if let Some(Node::If(_, c, t, e)) = b.last() {
                    let c = c.clone();
                    let kind = match (t.as_slice(), e.as_slice()) {
                        ([Node::Continue], [Node::Break]) | ([Node::Continue], []) if !b.iter().rev().skip(1).any(has_break) => Some(c),
                        ([Node::Break], [Node::Continue]) => Some(negate(c)),
                        _ => None,
                    };
                    if let Some(k) = kind {
                        let is_tail_continue = matches!(e.as_slice(), []);
                        if !is_tail_continue || matches!(t.as_slice(), [Node::Continue]) {
                            b.pop();
                            out.push(Node::Loop(a, Loop::DoWhile(k), b));
                            continue;
                        }
                    }
                }
                out.push(Node::Loop(a, Loop::Forever, b));
            }
            Node::Loop(a, k, b) => out.push(Node::Loop(a, k, tidy(b))),
            Node::Switch(a, x, arms, d) => out.push(Node::Switch(a, x, arms.into_iter().map(|(v, b)| (v, tidy(b))).collect(), d.map(tidy))),
            other => out.push(other),
        }
    }
    out
}

/// Todo caminho termina em `return r`?
fn ends_ret(nodes: &[Node], r: &Option<E>) -> bool {
    match nodes.last() {
        Some(Node::Ret(_, x)) => x == r,
        Some(Node::If(_, _, t, e)) => !e.is_empty() && ends_ret(t, r) && ends_ret(e, r),
        _ => false,
    }
}

fn strip_ret(nodes: &mut Vec<Node>) {
    match nodes.last_mut() {
        Some(Node::Ret(..)) => {
            nodes.pop();
        }
        Some(Node::If(_, _, t, e)) => {
            strip_ret(t);
            strip_ret(e);
        }
        _ => {}
    }
}

fn has_continue(n: &Node) -> bool {
    match n {
        Node::Continue => true,
        Node::If(_, _, t, e) => t.iter().any(has_continue) || e.iter().any(has_continue),
        _ => false,
    }
}

/// `i = 0; while (i < n) { ...; i = i + 1; }`  →  `for (i = 0; i < n; i = i + 1) { ... }`.
fn make_for(nodes: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = vec![];
    for n in nodes {
        let n = match n {
            Node::If(a, c, t, e) => Node::If(a, c, make_for(t), make_for(e)),
            Node::Loop(a, k, b) => Node::Loop(a, k, make_for(b)),
            Node::Switch(a, x, arms, d) => Node::Switch(a, x, arms.into_iter().map(|(v, b)| (v, make_for(b))).collect(), d.map(make_for)),
            other => other,
        };
        if let Node::Loop(a, Loop::While(c), body) = &n {
            if let (Some(Node::S(init @ Stmt { st: St::Set(v, _), .. })), Some(Node::S(step @ Stmt { st: St::Set(v2, se), .. }))) = (out.last(), body.last()) {
                let uses = |e: &E| e.any(&|x| *x == E::V(*v));
                if v == v2 && *v != u32::MAX && uses(c) && uses(se) && !body.iter().any(has_continue) {
                    let (init, step) = (init.clone(), step.clone());
                    let mut b = body.clone();
                    b.pop();
                    out.pop();
                    out.push(Node::Loop(*a, Loop::For(Box::new(init), c.clone(), Box::new(step)), b));
                    continue;
                }
            }
        }
        out.push(n);
    }
    out
}

fn has_break(n: &Node) -> bool {
    match n {
        Node::Break => true,
        Node::If(_, _, t, e) => t.iter().any(has_break) || e.iter().any(has_break),
        _ => false,
    }
}

/// Variáveis vivas: usadas em condição, chamada, retorno, memória ou atribuição a
/// variável "real" — ou na atribuição de outra variável viva. Cadeias de cópias
/// que só se alimentam entre si (comum em laços) morrem.
fn liveness(nodes: &[Node], removable: &dyn Fn(u32) -> bool) -> HashMap<u32, usize> {
    fn vars_of(e: &E, out: &mut Vec<u32>) {
        e.walk(&mut |x| {
            if let E::V(v) = x {
                out.push(*v);
            }
        });
    }
    fn walk(nodes: &[Node], removable: &dyn Fn(u32) -> bool, roots: &mut Vec<u32>, deps: &mut HashMap<u32, Vec<u32>>) {
        for n in nodes {
            match n {
                Node::S(Stmt { st: St::Set(v, e), .. }) if *v != u32::MAX && removable(*v) => {
                    let d = deps.entry(*v).or_default();
                    vars_of(e, d);
                    if e.has_side_effects() {
                        vars_of(e, roots);
                    }
                }
                Node::S(s) => {
                    for e in s.exprs() {
                        vars_of(e, roots);
                    }
                }
                Node::If(_, c, t, e) => {
                    vars_of(c, roots);
                    walk(t, removable, roots, deps);
                    walk(e, removable, roots, deps);
                }
                Node::Loop(_, k, b) => {
                    match k {
                        Loop::While(c) | Loop::DoWhile(c) => vars_of(c, roots),
                        Loop::For(i, c, st) => {
                            vars_of(c, roots);
                            walk(&[Node::S((**i).clone()), Node::S((**st).clone())], removable, roots, deps);
                        }
                        Loop::Forever => {}
                    }
                    walk(b, removable, roots, deps);
                }
                Node::Switch(_, x, arms, d) => {
                    vars_of(x, roots);
                    for (_, b) in arms {
                        walk(b, removable, roots, deps);
                    }
                    if let Some(d) = d {
                        walk(d, removable, roots, deps);
                    }
                }
                Node::Ret(_, Some(e)) => vars_of(e, roots),
                _ => {}
            }
        }
    }
    let mut roots = vec![];
    let mut deps: HashMap<u32, Vec<u32>> = HashMap::new();
    walk(nodes, removable, &mut roots, &mut deps);
    let mut live: HashMap<u32, usize> = HashMap::new();
    while let Some(v) = roots.pop() {
        if live.insert(v, 1).is_none() {
            if let Some(d) = deps.get(&v) {
                roots.extend(d.iter().copied());
            }
        }
    }
    live
}

fn remove_dead(nodes: Vec<Node>, reads: &HashMap<u32, usize>, removable: &dyn Fn(u32) -> bool) -> Vec<Node> {
    nodes
        .into_iter()
        .filter_map(|n| match n {
            Node::S(Stmt { st: St::Set(v, ref e), .. }) if removable(v) && !reads.contains_key(&v) && !e.has_side_effects() => None,
            // Resultado nunca lido: fica só a chamada.
            Node::S(Stmt { addr, st: St::Set(v, e) }) if removable(v) && !reads.contains_key(&v) => Some(Node::S(Stmt {
                addr,
                st: match e {
                    E::Call(c) => St::Call(*c),
                    other => St::Set(u32::MAX, other),
                },
            })),
            Node::If(a, c, t, e) => Some(Node::If(a, c, remove_dead(t, reads, removable), remove_dead(e, reads, removable))),
            Node::Loop(a, k, b) => Some(Node::Loop(a, k, remove_dead(b, reads, removable))),
            Node::Switch(a, x, arms, d) => Some(Node::Switch(
                a,
                x,
                arms.into_iter().map(|(v, b)| (v, remove_dead(b, reads, removable))).collect(),
                d.map(|d| remove_dead(d, reads, removable)),
            )),
            other => Some(other),
        })
        .collect()
}

fn cleanup(cx: &mut Ctx, ast: Vec<Node>) -> Vec<Node> {
    // 1. Retornos usados uma vez entram na expressão seguinte.
    let stack_vars: HashSet<u32> = cx.vars.iter().enumerate().filter(|(_, k)| matches!(k, VKey::Stack(_))).map(|(i, _)| i as u32).collect();
    // Uma chamada só "pula" `local = simples` se não lê esse local nem recebe endereço da pilha.
    let movable = move |n: &Node, c: &CallE| match n {
        Node::S(Stmt { st: St::Set(v, e), .. }) => {
            let call = E::Call(Box::new(c.clone()));
            stack_vars.contains(v)
                && !e.any(&|x| matches!(x, E::Call(_) | E::Ld(..) | E::Res(_)))
                && !call.any(&|x| *x == E::V(*v) || matches!(x, E::Sp(_) | E::Ld(..)))
        }
        _ => false,
    };
    let (res, _) = count_refs(&ast);
    let mut ast = inline_calls(ast, &res, &movable);
    // Os demais retornos usados viram variáveis.
    let (res, _) = count_refs(&ast);
    let mut ret_var: HashMap<u64, u32> = HashMap::new();
    for &a in res.keys() {
        let v = cx.var(VKey::Ret(a));
        ret_var.insert(a, v);
    }
    if !ret_var.is_empty() {
        ast = map_nodes(ast, &mut |e| match e {
            E::Res(a) => ret_var.get(&a).map_or(E::Res(a), |v| E::V(*v)),
            other => other,
        });
        ast = assign_results(ast, &ret_var);
    }
    // 2. Cópia de parâmetro para a pilha (gcc -O0): local_X = param_N → usa param_N.
    let mut sets: HashMap<u32, usize> = HashMap::new();
    assigned(&ast, &mut sets);
    let mut addr_taken: HashSet<i64> = HashSet::new();
    {
        let mut all = vec![];
        for n in &ast {
            node_exprs(n, &mut all);
        }
        for e in all {
            e.walk(&mut |x| {
                if let E::Sp(o) = x {
                    addr_taken.insert(*o);
                }
            });
        }
    }
    let mut alias: HashMap<u32, E> = HashMap::new();
    collect_param_copies(&ast, cx, &sets, &addr_taken, &mut alias);
    if !alias.is_empty() {
        ast = drop_sets(ast, &alias.keys().copied().collect());
        ast = map_nodes(ast, &mut |e| match e {
            E::V(v) => alias.get(&v).cloned().unwrap_or(E::V(v)),
            other => other,
        });
    }
    // 3. Estrutura de laços e ifs.
    let mut ast = tidy(ast);
    // Uma segunda passada de embutir (condições de laço recém-criadas).
    let (res, _) = count_refs(&ast);
    ast = inline_calls(ast, &res, &movable);
    // 4. Variáveis de junção/temporárias que não chegam a nenhum uso real.
    let vars = cx.vars.clone();
    let removable = |v: u32| matches!(vars.get(v as usize), Some(VKey::Merge(..)) | Some(VKey::Temp(..)) | Some(VKey::Ret(_)));
    for _ in 0..2 {
        let live = liveness(&ast, &removable);
        ast = remove_dead(ast, &live, &removable);
    }
    // Canário de pilha (`fs:0x28` + `__stack_chk_fail`): ruído do compilador.
    let ast = strip_canary(ast);
    // 5. Zeragem de structs/arrays na pilha vira memset.
    let regions = struct_regions(cx, &ast);
    let ast = merge_struct_zero(ast, cx, &regions);
    let ast = merge_zero_fill(ast, cx);
    make_for(tidy(ast))
}

fn is_canary_load(e: &E) -> bool {
    e.any(&|x| matches!(x, E::Ld(a, _) if matches!(&**a, E::Raw(r) if r == "fs:0x28" || r == "gs:0x14" || r == "fs:0x14")))
}

fn calls_chk_fail(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| matches!(n, Node::S(Stmt { st: St::Call(c), .. }) if c.name.as_deref().is_some_and(|n| winapi::clean_name(n) == "__stack_chk_fail")))
}

fn strip_canary(nodes: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = vec![];
    let mut it = nodes.into_iter().peekable();
    while let Some(n) = it.next() {
        match n {
            Node::S(Stmt { st: St::Set(_, ref e), .. }) if is_canary_load(e) => continue,
            Node::If(a, c, t, e) if is_canary_load(&c) => {
                // Um ramo chama __stack_chk_fail: fica só o outro.
                if calls_chk_fail(&t) {
                    out.extend(strip_canary(e));
                } else if calls_chk_fail(&e) {
                    out.extend(strip_canary(t));
                } else {
                    let rest_fails = it.peek().is_some_and(|n| calls_chk_fail(std::slice::from_ref(n)));
                    if rest_fails && e.is_empty() {
                        it.next();
                        out.extend(strip_canary(t));
                    } else {
                        out.push(Node::If(a, c, strip_canary(t), strip_canary(e)));
                    }
                }
            }
            Node::If(a, c, t, e) => out.push(Node::If(a, c, strip_canary(t), strip_canary(e))),
            Node::Loop(a, k, b) => out.push(Node::Loop(a, k, strip_canary(b))),
            Node::Switch(a, x, arms, d) => out.push(Node::Switch(a, x, arms.into_iter().map(|(v, b)| (v, strip_canary(b))).collect(), d.map(strip_canary))),
            other => out.push(other),
        }
    }
    out
}

/// (deslocamento, tamanho) das structs Win32 na pilha (endereço passado a uma API).
fn struct_regions(cx: &mut Ctx, ast: &[Node]) -> Vec<(i64, i64)> {
    let mut calls = vec![];
    collect_calls(ast, &mut calls);
    let mut out = vec![];
    for c in &calls {
        let Some(api) = c.name.as_deref().and_then(|n| cx.api(n)) else { continue };
        for (i, a) in c.args.iter().enumerate() {
            if let (Some(p), E::Sp(off)) = (api.params.get(i), a) {
                if let Some(def) = winapi::struct_def(&p.ty, cx.ptr == 8) {
                    if !out.iter().any(|(o, _)| o == off) {
                        out.push((*off, def.size));
                    }
                }
            }
        }
    }
    out
}

/// Zeragens espalhadas que cobrem uma struct inteira antes do primeiro uso → um memset.
fn merge_struct_zero(nodes: Vec<Node>, cx: &Ctx, regions: &[(i64, i64)]) -> Vec<Node> {
    let mut nodes: Vec<Node> = nodes
        .into_iter()
        .map(|n| match n {
            Node::If(a, c, t, e) => Node::If(a, c, merge_struct_zero(t, cx, regions), merge_struct_zero(e, cx, regions)),
            Node::Loop(a, k, b) => Node::Loop(a, k, merge_struct_zero(b, cx, regions)),
            other => other,
        })
        .collect();
    for &(off, size) in regions {
        let mut picks: Vec<usize> = vec![];
        let mut covered: Vec<(i64, i64)> = vec![];
        for (i, n) in nodes.iter().enumerate() {
            let mut refs = vec![];
            node_exprs(n, &mut refs);
            if refs.iter().any(|e| e.any(&|x| matches!(x, E::Sp(o) if *o >= off && *o < off + size))) {
                break; // endereço da struct usado: daqui em diante não mexe
            }
            if let Node::S(Stmt { st: St::Set(v, E::C(0)), .. }) = n {
                if let Some(VKey::Stack(o)) = cx.vars.get(*v as usize) {
                    if *o >= off && *o < off + size {
                        let sz = i64::from(cx.var_size.get(v).copied().unwrap_or(cx.ptr));
                        picks.push(i);
                        covered.push((*o, sz));
                    }
                }
            }
        }
        covered.sort_unstable();
        let mut end = off;
        for (o, sz) in &covered {
            if *o > end {
                break;
            }
            end = end.max(o + sz);
        }
        if picks.len() >= 2 && end >= off + size {
            let first = picks[0];
            let addr = match &nodes[first] {
                Node::S(s) => s.addr,
                _ => 0,
            };
            for &i in picks.iter().rev() {
                nodes.remove(i);
            }
            nodes.insert(
                first,
                Node::S(Stmt { addr, st: St::Call(CallE { addr, target: E::Sym("memset".into()), name: Some("memset".into()), args: vec![E::Sp(off), E::C(0), E::C(size)] }) }),
            );
        }
    }
    nodes
}

/// Sequência de `local = 0` em endereços contíguos da pilha → `memset`.
fn merge_zero_fill(nodes: Vec<Node>, cx: &Ctx) -> Vec<Node> {
    let zero_store = |n: &Node| -> Option<(i64, i64, u64)> {
        if let Node::S(Stmt { addr, st: St::Set(v, E::C(0)) }) = n {
            if let Some(VKey::Stack(off)) = cx.vars.get(*v as usize) {
                let size = i64::from(cx.var_size.get(v).copied().unwrap_or(cx.ptr));
                return Some((*off, size, *addr));
            }
        }
        None
    };
    let mut out: Vec<Node> = vec![];
    let mut i = 0;
    let nodes: Vec<Node> = nodes
        .into_iter()
        .map(|n| match n {
            Node::If(a, c, t, e) => Node::If(a, c, merge_zero_fill(t, cx), merge_zero_fill(e, cx)),
            Node::Loop(a, k, b) => Node::Loop(a, k, merge_zero_fill(b, cx)),
            Node::Switch(a, x, arms, d) => Node::Switch(a, x, arms.into_iter().map(|(v, b)| (v, merge_zero_fill(b, cx))).collect(), d.map(|d| merge_zero_fill(d, cx))),
            other => other,
        })
        .collect();
    while i < nodes.len() {
        let mut j = i;
        let mut run: Vec<(i64, i64, u64)> = vec![];
        while j < nodes.len() {
            match zero_store(&nodes[j]) {
                Some(z) => run.push(z),
                None => break,
            }
            j += 1;
        }
        if run.len() >= 2 && run.iter().any(|r| r.1 >= 16) {
            let mut sorted = run.clone();
            sorted.sort_by_key(|r| r.0);
            let contiguous = sorted.windows(2).all(|w| w[0].0 + w[0].1 == w[1].0);
            if contiguous {
                let lo = sorted[0].0;
                let hi = sorted.last().map(|r| r.0 + r.1).unwrap_or(lo);
                let addr = run[0].2;
                out.push(Node::S(Stmt {
                    addr,
                    st: St::Call(CallE { addr, target: E::Sym("memset".into()), name: Some("memset".into()), args: vec![E::Sp(lo), E::C(0), E::C(hi - lo)] }),
                }));
                i = j;
                continue;
            }
        }
        out.push(nodes[i].clone());
        i += 1;
    }
    out
}

fn assign_results(nodes: Vec<Node>, ret_var: &HashMap<u64, u32>) -> Vec<Node> {
    nodes
        .into_iter()
        .map(|n| match n {
            Node::S(Stmt { addr, st: St::Call(c) }) if ret_var.contains_key(&c.addr) => {
                let v = ret_var[&c.addr];
                Node::S(Stmt { addr, st: St::Set(v, E::Call(Box::new(c))) })
            }
            Node::If(a, c, t, e) => Node::If(a, c, assign_results(t, ret_var), assign_results(e, ret_var)),
            Node::Loop(a, k, b) => Node::Loop(a, k, assign_results(b, ret_var)),
            Node::Switch(a, x, arms, d) => {
                Node::Switch(a, x, arms.into_iter().map(|(v, b)| (v, assign_results(b, ret_var))).collect(), d.map(|d| assign_results(d, ret_var)))
            }
            other => other,
        })
        .collect()
}

fn collect_param_copies(nodes: &[Node], cx: &Ctx, sets: &HashMap<u32, usize>, taken: &HashSet<i64>, out: &mut HashMap<u32, E>) {
    for n in nodes {
        if let Node::S(Stmt { st: St::Set(v, E::V(p)), .. }) = n {
            if let (Some(VKey::Stack(off)), Some(VKey::Param(_))) = (cx.vars.get(*v as usize), cx.vars.get(*p as usize)) {
                let near_taken = taken.iter().any(|t| *t <= *off && *off < *t + 0x400);
                if sets.get(v) == Some(&1) && !near_taken && sets.get(p).is_none() {
                    out.insert(*v, E::V(*p));
                }
            }
        }
    }
}

fn drop_sets(nodes: Vec<Node>, vars: &HashSet<u32>) -> Vec<Node> {
    nodes
        .into_iter()
        .filter_map(|n| match n {
            Node::S(Stmt { st: St::Set(v, _), .. }) if vars.contains(&v) => None,
            Node::If(a, c, t, e) => Some(Node::If(a, c, drop_sets(t, vars), drop_sets(e, vars))),
            Node::Loop(a, k, b) => Some(Node::Loop(a, k, drop_sets(b, vars))),
            Node::Switch(a, x, arms, d) => Some(Node::Switch(a, x, arms.into_iter().map(|(v, b)| (v, drop_sets(b, vars))).collect(), d.map(|d| drop_sets(d, vars)))),
            other => Some(other),
        })
        .collect()
}

// ===================================================================== impressão

struct StructInst {
    off: i64,
    def: winapi::StructDef,
}

struct Printer<'p, 'a> {
    cx: &'p mut Ctx<'a>,
    lines: Vec<CodeLine>,
    names: HashMap<u32, String>,
    types: HashMap<u32, String>,
    /// Grupo de constantes associado a uma variável (`uMsg` → `WM`).
    groups: HashMap<u32, String>,
    structs: Vec<StructInst>,
    pe: bool,
    param_names: Vec<String>,
    param_types: Vec<String>,
    void: bool,
}

fn collect_calls(nodes: &[Node], out: &mut Vec<CallE>) {
    let mut all = vec![];
    for n in nodes {
        node_exprs(n, &mut all);
    }
    for n in nodes {
        if let Node::S(Stmt { st: St::Call(c), .. }) = n {
            out.push(c.clone());
        }
    }
    for e in all {
        e.walk(&mut |x| {
            if let E::Call(c) = x {
                out.push((**c).clone());
            }
        });
    }
    fn nested(nodes: &[Node], out: &mut Vec<CallE>) {
        for n in nodes {
            match n {
                Node::If(_, _, t, e) => {
                    collect_calls(t, out);
                    collect_calls(e, out);
                }
                Node::Loop(_, _, b) => collect_calls(b, out),
                Node::Switch(_, _, arms, d) => {
                    for (_, b) in arms {
                        collect_calls(b, out);
                    }
                    if let Some(d) = d {
                        collect_calls(d, out);
                    }
                }
                _ => {}
            }
        }
    }
    nested(nodes, out);
}

fn c_escape(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        match ch {
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\x{:02x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn num(v: i64) -> String {
    let u = v as u64;
    if (0xFFFF_0000..=0xFFFF_FFFF).contains(&u) {
        return (u as u32 as i32).to_string();
    }
    let a = v.unsigned_abs();
    if a < 1000 || (a < 100_000 && a % 10 == 0 && !a.is_power_of_two()) {
        v.to_string()
    } else if v < 0 {
        format!("-0x{a:x}")
    } else {
        format!("0x{v:x}")
    }
}

fn int_type(size: u8, ptr: u8) -> &'static str {
    match size {
        1 => "char",
        2 => "short",
        4 => "int",
        8 if ptr == 8 => "int64_t",
        8 => "int64_t",
        _ => "int",
    }
}

impl<'p, 'a> Printer<'p, 'a> {
    fn var_key(&self, v: u32) -> Option<VKey> {
        self.cx.vars.get(v as usize).copied()
    }

    fn struct_at(&self, off: i64) -> Option<(&StructInst, i64)> {
        self.structs.iter().find(|s| off >= s.off && off < s.off + s.def.size).map(|s| (s, off - s.off))
    }

    fn var_name(&self, v: u32) -> String {
        if let Some(n) = self.names.get(&v) {
            return n.clone();
        }
        match self.var_key(v) {
            Some(VKey::Param(i)) => self.param_names.get(i as usize).cloned().unwrap_or_else(|| format!("param_{}", i + 1)),
            Some(VKey::Stack(off)) => {
                if let Some((s, rel)) = self.struct_at(off) {
                    return match s.def.fields.iter().find(|f| f.off == rel) {
                        Some(f) => format!("{}.{}", s.def.var, f.name),
                        None => format!("{}.field_0x{rel:x}", s.def.var),
                    };
                }
                if off < 0 {
                    format!("local_{:x}", -off)
                } else {
                    format!("stack_{off:x}")
                }
            }
            Some(VKey::In(r)) => format!("in_{}", reg_label(r)),
            _ => format!("var{v}"),
        }
    }

    /// Tamanho do elemento apontado, quando o tipo é conhecido (`char **argv` → 8).
    fn pointee_size(&self, e: &E) -> Option<u8> {
        let E::V(v) = e else { return None };
        let Some(VKey::Param(i)) = self.var_key(*v) else { return None };
        let ty = self.param_types.get(i as usize)?;
        let inner = ty.strip_suffix('*')?;
        Some(if inner.ends_with('*') { self.cx.ptr } else if inner.starts_with("char") { 1 } else if inner.starts_with("int") { 4 } else { return None })
    }

    fn group_of(&self, e: &E) -> Option<String> {
        match e {
            E::V(v) => {
                if let Some(g) = self.groups.get(v) {
                    return Some(g.clone());
                }
                if let Some(VKey::Stack(off)) = self.var_key(*v) {
                    if let Some((s, rel)) = self.struct_at(off) {
                        return s.def.fields.iter().find(|f| f.off == rel).and_then(|f| f.group.map(str::to_string));
                    }
                }
                None
            }
            E::Cast(_, _, x) => self.group_of(x),
            _ => None,
        }
    }

    fn const_named(&self, v: i64, group: Option<&str>, class: Option<&str>) -> String {
        if let Some(g) = group {
            if let Some(n) = winapi::constant(g, v, class) {
                return n;
            }
        }
        num(v)
    }

    fn ty_of_size(&self, size: u8) -> &'static str {
        int_type(size, self.cx.ptr)
    }

    fn e(&mut self, e: &E) -> String {
        self.ex(e, 0, None)
    }

    /// `prec`: precedência do contexto (maior = liga mais forte).
    fn ex(&mut self, e: &E, prec: u8, group: Option<&str>) -> String {
        let wrap = |s: String, mine: u8| if mine < prec { format!("({s})") } else { s };
        match e {
            E::C(v) => {
                let s = self.const_named(*v, group, None);
                if s.contains(' ') && prec > 1 {
                    format!("({s})")
                } else {
                    s
                }
            }
            E::Str(a) => match self.cx.strings.get(a) {
                Some((s, true)) => format!("L\"{}\"", c_escape(s)),
                Some((s, false)) => format!("\"{}\"", c_escape(s)),
                None => format!("&DAT_{a:x}"),
            },
            E::Sym(n) => winapi::clean_name(n),
            E::Imp(n) => format!("&__imp_{}", winapi::clean_name(n)),
            E::Glob(a) => format!("&DAT_{a:x}"),
            E::Sp(off) => {
                if let Some((s, rel)) = self.struct_at(*off) {
                    if rel == 0 {
                        return format!("&{}", s.def.var);
                    }
                }
                let v = self.cx.var(self.cx.stack_key(*off));
                format!("&{}", self.var_name(v))
            }
            E::V(v) => self.var_name(*v),
            E::Res(a) => format!("ret_{a:x}"),
            E::Saved(r) => format!("unaff_{}", reg_label(*r)),
            E::Raw(s) => s.clone(),
            E::Ld(a, size) => {
                let t = self.ty_of_size(*size);
                match &**a {
                    E::Glob(g) => format!("DAT_{g:x}"),
                    E::Raw(r) if r.starts_with("fs:") || r.starts_with("gs:") => format!("*(int64_t *)({r})"),
                    // base + i*tamanho  →  base[i]
                    E::Bin(Bo::Add, b, idx) if matches!(&**idx, E::Bin(Bo::Mul, _, k) if **k == E::C(i64::from(*size))) => {
                        let E::Bin(Bo::Mul, i, _) = &**idx else { unreachable!() };
                        let i = match &**i {
                            E::Cast(_, _, x) => (**x).clone(),
                            x => x.clone(),
                        };
                        let is = self.ex(&i, 0, None);
                        if self.pointee_size(b) == Some(*size) {
                            let bs = self.ex(b, 14, None);
                            return format!("{bs}[{is}]");
                        }
                        let bs = self.ex(b, 13, None);
                        format!("(({t} *){bs})[{is}]")
                    }
                    E::Bin(Bo::Add, b, k) if matches!(**k, E::C(off) if off % i64::from(*size) == 0) && self.pointee_size(b) == Some(*size) => {
                        let E::C(off) = **k else { unreachable!() };
                        let bs = self.ex(b, 14, None);
                        format!("{bs}[{}]", off / i64::from(*size))
                    }
                    other if self.pointee_size(other) == Some(*size) => {
                        let bs = self.ex(other, 14, None);
                        format!("{bs}[0]")
                    }
                    other => {
                        let inner = self.ex(other, 12, None);
                        wrap(format!("*({t} *){inner}"), 13)
                    }
                }
            }
            E::Cast(size, signed, x) => {
                if self.pe && *size == 2 && !signed {
                    let inner = self.ex(x, 0, None);
                    return format!("LOWORD({inner})");
                }
                let t = match (size, signed) {
                    (1, false) => "unsigned char",
                    (1, true) => "char",
                    (2, false) => "unsigned short",
                    (2, true) => "short",
                    (4, false) => "unsigned int",
                    (4, true) => "int",
                    _ => "int64_t",
                };
                let inner = self.ex(x, 13, None);
                wrap(format!("({t}){inner}"), 13)
            }
            E::Un(op, x) => {
                let inner = self.ex(x, 13, None);
                let s = match op {
                    Uo::Neg => format!("-{inner}"),
                    Uo::Not => format!("~{inner}"),
                    Uo::LNot => format!("!{inner}"),
                };
                wrap(s, 13)
            }
            E::Bin(op, a, b) => {
                let (sym, p) = match op {
                    Bo::Mul => ("*", 11),
                    Bo::Div | Bo::UDiv => ("/", 11),
                    Bo::Mod | Bo::UMod => ("%", 11),
                    Bo::Add => ("+", 10),
                    Bo::Shl => ("<<", 9),
                    Bo::Shr | Bo::Sar => (">>", 9),
                    Bo::And => ("&", 6),
                    Bo::Xor => ("^", 5),
                    Bo::Or => ("|", 4),
                };
                // x + -k  →  x - k
                if *op == Bo::Add {
                    if let E::C(k) = **b {
                        if k < 0 {
                            let l = self.ex(a, p, None);
                            return wrap(format!("{l} - {}", num(k.wrapping_neg())), p);
                        }
                    }
                    if let E::Un(Uo::Neg, ref y) = **b {
                        let l = self.ex(a, p, None);
                        let r = self.ex(y, p + 1, None);
                        return wrap(format!("{l} - {r}"), p);
                    }
                }
                let g = if matches!(op, Bo::Or | Bo::And) { group } else { None };
                let l = self.ex(a, p, g);
                let r = self.ex(b, p + 1, g);
                wrap(format!("{l} {sym} {r}"), p)
            }
            E::Cmp(op, a, b) => {
                let ga = self.group_of(a);
                let gb = self.group_of(b);
                let l = self.ex(a, 8, gb.as_deref());
                let r = self.ex(b, 8, ga.as_deref());
                wrap(format!("{l} {} {r}", op.sym()), 7)
            }
            E::Tern(c, a, b) => {
                let c = self.ex(c, 3, None);
                let a = self.ex(a, 3, None);
                let b = self.ex(b, 3, None);
                wrap(format!("{c} ? {a} : {b}"), 2)
            }
            E::Call(c) => self.call(c),
        }
    }

    fn call(&mut self, c: &CallE) -> String {
        let api = c.name.as_deref().and_then(|n| self.cx.api(n));
        let target = match &c.target {
            E::Sym(n) => winapi::clean_name(n),
            other => {
                let t = self.ex(other, 13, None);
                format!("(*{t})")
            }
        };
        if target == "memset" && c.args.len() == 3 {
            if let (E::Sp(off), E::C(n)) = (&c.args[0], &c.args[2]) {
                if let Some((s, 0)) = self.struct_at(*off) {
                    if s.def.size == *n {
                        let (name, ty) = (s.def.var, s.def.name);
                        let _ = ty;
                        let v = self.ex(&c.args[1], 1, None);
                        return format!("memset(&{name}, {v}, sizeof({name}))");
                    }
                }
            }
        }
        // Classe da janela (para estilos BS_/ES_ em CreateWindowEx).
        let class = match (&api, c.args.get(1)) {
            (Some(a), Some(E::Str(s))) if a.name == "CreateWindowEx" => self.cx.strings.get(s).map(|x| x.0.clone()),
            _ => None,
        };
        let mut args = vec![];
        for (i, a) in c.args.iter().enumerate() {
            let p = api.as_ref().and_then(|x| x.params.get(i));
            let s = match (p, a) {
                (Some(p), E::C(0)) if winapi::is_pointer_type(&p.ty) => "NULL".to_string(),
                (Some(p), E::C(v)) if p.group.is_some() => {
                    let g = p.group.as_deref();
                    match g.and_then(|g| winapi::constant(g, *v, class.as_deref())) {
                        Some(n) => n,
                        None if winapi::is_pointer_type(&p.ty) => format!("({}){}", p.ty, num(*v)),
                        None => num(*v),
                    }
                }
                (Some(p), E::C(v)) if winapi::is_pointer_type(&p.ty) && !p.ty.starts_with("LPC") => format!("({}){}", p.ty, num(*v)),
                (Some(p), _) => self.ex(a, 1, p.group.as_deref()),
                (None, _) => self.ex(a, 1, None),
            };
            args.push(s);
        }
        format!("{target}({})", args.join(", "))
    }

    fn push(&mut self, addr: u64, depth: usize, text: String, target: Option<u64>) {
        self.lines.push(CodeLine {
            address: format_address(addr),
            bytes: None,
            text: format!("{}{}", "    ".repeat(depth), text),
            comment: None,
            target: target.map(format_address),
        });
    }

    fn internal_target(&self, s: &Stmt) -> Option<u64> {
        let mut t = None;
        for e in s.exprs() {
            e.walk(&mut |x| {
                if let E::Call(c) = x {
                    if let E::Sym(n) = &c.target {
                        if t.is_none() {
                            t = self.cx.eng.funcs.iter().find(|f| &f.name == n).map(|f| f.start);
                        }
                    }
                }
            });
        }
        if let St::Call(c) = &s.st {
            if let E::Sym(n) = &c.target {
                t = t.or_else(|| self.cx.eng.funcs.iter().find(|f| &f.name == n).map(|f| f.start));
            }
        }
        t
    }

    /// `x = x + 1` → `x++`, `x = x * k` → `x *= k`.
    fn compound(&mut self, v: u32, e: &E) -> Option<String> {
        let E::Bin(op, a, b) = e else { return None };
        if **a != E::V(v) {
            return None;
        }
        let name = self.var_name(v);
        if *op == Bo::Add {
            match **b {
                E::C(1) => return Some(format!("{name}++")),
                E::C(-1) => return Some(format!("{name}--")),
                E::C(k) if k < 0 => return Some(format!("{name} -= {}", num(k.wrapping_neg()))),
                _ => {}
            }
        }
        let sym = match op {
            Bo::Add => "+=",
            Bo::Mul => "*=",
            Bo::And => "&=",
            Bo::Or => "|=",
            Bo::Xor => "^=",
            Bo::Shl => "<<=",
            Bo::Shr | Bo::Sar => ">>=",
            _ => return None,
        };
        let r = self.ex(b, 1, None);
        Some(format!("{name} {sym} {r}"))
    }

    fn stmt_text(&mut self, s: &Stmt) -> String {
        let t = self.stmt_body(s);
        t.trim_end_matches(';').to_string()
    }

    fn stmt(&mut self, s: &Stmt, depth: usize) {
        let text = self.stmt_body(s);
        let target = self.internal_target(s);
        self.push(s.addr, depth, text, target);
    }

    fn stmt_body(&mut self, s: &Stmt) -> String {
        if let St::Set(v, e) = &s.st {
            if *v != u32::MAX {
                if let Some(c) = self.compound(*v, e) {
                    return format!("{c};");
                }
            }
        }
        match &s.st {
            St::Set(v, e) if *v == u32::MAX => format!("{};", self.e(e)),
            St::Set(v, e) => {
                let g = self.group_of(&E::V(*v));
                let rhs = match (e, g.as_deref()) {
                    (E::C(k), Some(g)) => self.const_named(*k, Some(g), None),
                    _ => self.ex(e, 1, g.as_deref()),
                };
                format!("{} = {};", self.var_name(*v), rhs)
            }
            St::Store(a, v, size) => {
                let t = self.ty_of_size(*size);
                let lhs = match a {
                    E::Glob(g) => format!("DAT_{g:x}"),
                    other => {
                        let inner = self.ex(other, 12, None);
                        format!("*({t} *){inner}")
                    }
                };
                let rhs = self.ex(v, 1, None);
                format!("{lhs} = {rhs};")
            }
            St::Call(c) => format!("{};", self.call(c)),
            St::Asm(t) => format!("__asm(\"{t}\");"),
        }
    }

    fn nodes(&mut self, nodes: &[Node], depth: usize, fallback: u64) {
        for n in nodes {
            self.node(n, depth, fallback);
        }
    }

    fn node(&mut self, n: &Node, depth: usize, fallback: u64) {
        match n {
            Node::S(s) => self.stmt(s, depth),
            Node::If(a, c, t, e) => {
                let c = self.e(c);
                self.push(*a, depth, format!("if ({c}) {{"), None);
                self.nodes(t, depth + 1, *a);
                let mut els: &[Node] = e;
                loop {
                    if els.is_empty() {
                        break;
                    }
                    if let [Node::If(a2, c2, t2, e2)] = els {
                        let c2 = self.e(c2);
                        self.push(*a2, depth, format!("}} else if ({c2}) {{"), None);
                        self.nodes(t2, depth + 1, *a2);
                        els = e2;
                        continue;
                    }
                    self.push(*a, depth, "} else {".into(), None);
                    self.nodes(els, depth + 1, *a);
                    break;
                }
                self.push(*a, depth, "}".into(), None);
            }
            Node::Loop(a, k, b) => {
                match k {
                    Loop::Forever => self.push(*a, depth, "while (true) {".into(), None),
                    Loop::While(c) => {
                        let c = self.e(c);
                        self.push(*a, depth, format!("while ({c}) {{"), None);
                    }
                    Loop::DoWhile(_) => self.push(*a, depth, "do {".into(), None),
                    Loop::For(i, c, st) => {
                        let i = self.stmt_text(i);
                        let c = self.e(c);
                        let st = self.stmt_text(st);
                        self.push(*a, depth, format!("for ({i}; {c}; {st}) {{"), None);
                    }
                }
                self.nodes(b, depth + 1, *a);
                match k {
                    Loop::DoWhile(c) => {
                        let c = self.e(c);
                        self.push(*a, depth, format!("}} while ({c});"), None);
                    }
                    _ => self.push(*a, depth, "}".into(), None),
                }
            }
            Node::Switch(a, x, arms, d) => {
                let g = self.group_of(x);
                let xs = self.e(x);
                self.push(*a, depth, format!("switch ({xs}) {{"), None);
                for (vals, body) in arms {
                    for v in vals {
                        let name = self.const_named(*v, g.as_deref(), None);
                        self.push(*a, depth, format!("case {name}:"), None);
                    }
                    self.nodes(body, depth + 1, *a);
                }
                if let Some(d) = d {
                    self.push(*a, depth, "default:".into(), None);
                    self.nodes(d, depth + 1, *a);
                }
                self.push(*a, depth, "}".into(), None);
            }
            Node::Ret(a, e) => {
                let text = match e {
                    Some(e) if !self.void => {
                        let s = self.e(e);
                        format!("return {s};")
                    }
                    Some(E::Call(c)) => format!("{};\n{}return;", self.call(c), "    ".repeat(depth)),
                    _ => "return;".into(),
                };
                for (i, t) in text.split('\n').enumerate() {
                    let t = if i == 0 { t.to_string() } else { t.trim_start().to_string() };
                    self.push(*a, depth, t, None);
                }
            }
            Node::Break => self.push(fallback, depth, "break;".into(), None),
            Node::Continue => self.push(fallback, depth, "continue;".into(), None),
            Node::Goto(b) => {
                let addr = self.cx.blocks[*b].start;
                self.push(fallback, depth, format!("goto LAB_{addr:x};"), Some(addr));
            }
            Node::Label(b) => {
                let addr = self.cx.blocks[*b].start;
                self.push(addr, depth.saturating_sub(1), format!("LAB_{addr:x}:"), None);
            }
            Node::Comment(a, m) => self.push(*a, depth, format!("/* {m} */"), None),
        }
    }
}

fn returns(nodes: &[Node], out: &mut Vec<Option<E>>) {
    for n in nodes {
        match n {
            Node::Ret(_, e) => out.push(e.clone()),
            Node::If(_, _, t, e) => {
                returns(t, out);
                returns(e, out);
            }
            Node::Loop(_, _, b) => returns(b, out),
            Node::Switch(_, _, arms, d) => {
                for (_, b) in arms {
                    returns(b, out);
                }
                if let Some(d) = d {
                    returns(d, out);
                }
            }
            _ => {}
        }
    }
}

fn render(cx: &mut Ctx, ast: Vec<Node>) -> Vec<CodeLine> {
    let ptr64 = cx.ptr == 8;
    let pe = matches!(cx.abi, Abi::Win64) || cx.eng.extern_names.values().any(|n| n.contains(".dll!") || n.contains(".DLL!"));
    let fname = cx.f.name.clone();
    let known = winapi::known_signature(&fname);

    // Structs na pilha: endereço passado para um parâmetro do tipo `STRUCT*`.
    let mut calls = vec![];
    collect_calls(&ast, &mut calls);
    let mut structs: Vec<StructInst> = vec![];
    for c in &calls {
        let Some(api) = c.name.as_deref().and_then(|n| cx.api(n)) else { continue };
        for (i, a) in c.args.iter().enumerate() {
            if let (Some(p), E::Sp(off)) = (api.params.get(i), a) {
                if let Some(def) = winapi::struct_def(&p.ty, ptr64) {
                    if !structs.iter().any(|s| s.off == *off) {
                        structs.push(StructInst { off: *off, def });
                    }
                }
            }
        }
    }
    // Nomes de structs repetidos: wc, wc2...
    let mut used_names: HashMap<String, usize> = HashMap::new();
    for s in structs.iter_mut() {
        let n = used_names.entry(s.def.var.to_string()).or_default();
        *n += 1;
        if *n > 1 {
            let leaked: &'static str = Box::leak(format!("{}{}", s.def.var, n).into_boxed_str());
            s.def.var = leaked;
        }
    }

    // Retorno: void se nenhum caminho devolve valor útil.
    let mut rets = vec![];
    returns(&ast, &mut rets);
    let void = match known {
        Some((ret, _)) => ret.starts_with("void"),
        None => rets.iter().all(|r| match r {
            None => true,
            Some(E::Call(c)) => c.name.as_deref().and_then(|n| cx.api(n)).is_some_and(|a| a.returns_void()),
            Some(_) => false,
        }),
    };

    // Parâmetros usados.
    let (_, reads) = count_refs(&ast);
    let mut max_param: i64 = -1;
    let mut ref_vars: Vec<u32> = reads.keys().copied().collect();
    let mut sets = HashMap::new();
    assigned(&ast, &mut sets);
    ref_vars.extend(sets.keys().copied().filter(|v| *v != u32::MAX));
    // Endereços de pilha citados também contam.
    {
        let mut all = vec![];
        for n in &ast {
            node_exprs(n, &mut all);
        }
        let mut offs = vec![];
        for e in all {
            e.walk(&mut |x| {
                if let E::Sp(o) = x {
                    offs.push(*o);
                }
            });
        }
        for o in offs {
            let k = cx.stack_key(o);
            let v = cx.var(k);
            ref_vars.push(v);
        }
    }
    ref_vars.sort_unstable();
    ref_vars.dedup();
    for &v in &ref_vars {
        if let Some(VKey::Param(i)) = cx.vars.get(v as usize) {
            max_param = max_param.max(i64::from(*i));
        }
    }
    let nparams = match known {
        Some((_, ps)) => ps.len(),
        None => (max_param + 1) as usize,
    };
    let mut param_names = vec![];
    let mut param_types = vec![];
    let mut groups: HashMap<u32, String> = HashMap::new();
    for i in 0..nparams {
        let id = cx.var(VKey::Param(i as u32));
        match known.and_then(|(_, ps)| ps.get(i)) {
            Some((t, n, g)) => {
                param_types.push(t.to_string());
                param_names.push(n.to_string());
                if let Some(g) = g {
                    groups.insert(id, g.to_string());
                }
            }
            None => {
                let size = cx.var_size.get(&id).copied().unwrap_or(cx.ptr);
                param_types.push(int_type(size, cx.ptr).to_string());
                param_names.push(format!("param_{}", i + 1));
            }
        }
    }

    // Nomes e tipos sugeridos pelas APIs: `hMenu = CreateMenu()`.
    let mut names: HashMap<u32, String> = HashMap::new();
    let mut types: HashMap<u32, String> = HashMap::new();
    let mut taken: HashSet<String> = param_names.iter().cloned().collect();
    for s in &structs {
        taken.insert(s.def.var.to_string());
    }
    let mut hints: Vec<(u32, String, String)> = vec![];
    collect_hints(&ast, cx, &mut hints);
    let mut seen_hint: HashSet<u32> = HashSet::new();
    for (v, name, ty) in hints {
        if !seen_hint.insert(v) {
            continue;
        }
        if let Some(VKey::Param(_)) = cx.vars.get(v as usize) {
            continue;
        }
        if let Some(VKey::Stack(off)) = cx.vars.get(v as usize) {
            if structs.iter().any(|s| *off >= s.off && *off < s.off + s.def.size) {
                continue;
            }
        }
        let mut n = name.clone();
        let mut k = 2;
        while taken.contains(&n) {
            n = format!("{name}{k}");
            k += 1;
        }
        taken.insert(n.clone());
        names.insert(v, n);
        types.insert(v, ty);
    }
    // Demais variáveis: var1, var2... na ordem de aparição.
    let mut counter = 1;
    for &v in &ref_vars {
        if names.contains_key(&v) {
            continue;
        }
        if matches!(cx.vars.get(v as usize), Some(VKey::Merge(..)) | Some(VKey::Temp(..)) | Some(VKey::Fresh(_)) | Some(VKey::Ret(_))) {
            let mut n = format!("var{counter}");
            while taken.contains(&n) {
                counter += 1;
                n = format!("var{counter}");
            }
            counter += 1;
            taken.insert(n.clone());
            names.insert(v, n);
        }
    }

    let mut p = Printer {
        cx,
        lines: vec![],
        names,
        types,
        groups,
        structs,
        pe,
        param_names: param_names.clone(),
        param_types: param_types.clone(),
        void,
    };
    let start = p.cx.f.start;
    let abi = match p.cx.abi {
        Abi::Win64 => "x86-64, Windows",
        Abi::SysV64 => "x86-64, System V",
        Abi::X86 => "x86 32 bits",
    };
    p.push(start, 0, format!("// Decompilado ({abi}). Reconstrução a partir do binário: nomes de variáveis locais,"), None);
    p.push(start, 0, "// comentários e macros do código-fonte não existem no executável.".into(), None);
    let all_strings = !rets.is_empty() && rets.iter().all(|r| matches!(r, Some(E::Str(_))));
    let ret_ty = match known {
        Some((r, _)) => r.to_string(),
        None if void => "void".into(),
        None if all_strings => "const char *".into(),
        None => if ptr64 { "int64_t".into() } else { "int".into() },
    };
    let params = if nparams == 0 {
        "void".to_string()
    } else {
        p.param_types.iter().zip(p.param_names.iter()).map(|(t, n)| format!("{t} {n}")).collect::<Vec<_>>().join(", ")
    };
    p.push(start, 0, format!("{ret_ty} {}({params})", p.cx.f.name), None);
    p.push(start, 0, "{".into(), None);

    // Declarações.
    let mut decls: BTreeMap<String, String> = BTreeMap::new();
    for s in &p.structs {
        decls.insert(s.def.var.to_string(), format!("{} {};", s.def.name, s.def.var));
    }
    for &v in &ref_vars {
        let key = p.cx.vars.get(v as usize).copied();
        if matches!(key, Some(VKey::Param(_)) | Some(VKey::In(_)) | None) {
            continue;
        }
        if let Some(VKey::Stack(off)) = key {
            if p.struct_at(off).is_some() {
                continue;
            }
        }
        let name = p.var_name(v);
        let ty = p.types.get(&v).cloned().unwrap_or_else(|| {
            let size = p.cx.var_size.get(&v).copied().unwrap_or(p.cx.ptr);
            match size {
                16 => "char".to_string(),
                s => int_type(s, p.cx.ptr).to_string(),
            }
        });
        let decl = if p.cx.var_size.get(&v) == Some(&16) && !p.types.contains_key(&v) { format!("{ty} {name}[16];") } else { format!("{ty} {name};") };
        decls.insert(name, decl);
    }
    let has_decls = !decls.is_empty();
    for (_, d) in decls {
        p.push(start, 1, d, None);
    }
    if has_decls {
        p.push(start, 0, String::new(), None);
    }
    p.nodes(&ast, 1, start);
    let end = p.cx.ins.last().map_or(start, |i| i.addr);
    p.push(end, 0, "}".into(), None);
    p.lines
}

fn collect_hints(nodes: &[Node], cx: &mut Ctx, out: &mut Vec<(u32, String, String)>) {
    for n in nodes {
        match n {
            Node::S(Stmt { st: St::Set(v, E::Call(c)), .. }) => {
                if let Some(api) = c.name.as_deref().and_then(|x| cx.api(x)) {
                    if let Some(rn) = api.ret_name.clone() {
                        out.push((*v, rn, api.ret.clone()));
                    }
                }
            }
            Node::If(_, _, t, e) => {
                collect_hints(t, cx, out);
                collect_hints(e, cx, out);
            }
            Node::Loop(_, _, b) => collect_hints(b, cx, out),
            Node::Switch(_, _, arms, d) => {
                for (_, b) in arms {
                    collect_hints(b, cx, out);
                }
                if let Some(d) = d {
                    collect_hints(d, cx, out);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::NativeEngine;
    use crate::engine::{parse_address, CodeKind, Engine};

    /// Pseudocódigo de uma função (só o texto, sem indentação).
    fn pseudo(bin: &[u8], name: &str) -> Vec<String> {
        let e = NativeEngine::analyze(bin.to_vec(), &mut |_| {});
        let f = e.list_functions().into_iter().find(|f| f.name == name).unwrap_or_else(|| panic!("função {name} não encontrada"));
        let a = parse_address(&f.address).unwrap();
        e.lines(a, CodeKind::Pseudo).unwrap().iter().map(|l| l.text.trim().to_string()).collect()
    }

    fn has(lines: &[String], want: &str) {
        assert!(lines.iter().any(|l| l == want), "faltou `{want}` em:\n{}", lines.join("\n"));
    }

    // Fonte em tests/fixtures/hello.c (MinGW -O2 -s: sem símbolos).
    const WIN64: &[u8] = include_bytes!("../../../tests/fixtures/hello_win64.exe");
    const WIN32: &[u8] = include_bytes!("../../../tests/fixtures/hello_win32.exe");
    // Fonte em tests/fixtures/t2.c (gcc -O2, Linux).
    const ELF: &[u8] = include_bytes!("../../../tests/fixtures/t2_linux_O2.elf");

    #[test]
    fn wndproc_win64_matches_source() {
        for bin in [WIN64, WIN32] {
            let l = pseudo(bin, "WndProc");
            has(&l, "LRESULT CALLBACK WndProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam)");
            has(&l, "switch (uMsg) {");
            has(&l, "case WM_CREATE:");
            has(&l, "case WM_COMMAND:");
            has(&l, "AppendMenuA(hMenu2, MF_SEPARATOR, 0, NULL);");
            has(&l, r#"CreateWindowExA(0, "BUTTON", "Hello World", WS_CHILD | WS_VISIBLE | BS_PUSHBUTTON, 110, 60, 160, 40, hwnd, (HMENU)1, NULL, NULL);"#);
            has(&l, "if (LOWORD(wParam) == 1) {");
            has(&l, r#"MessageBoxA(hwnd, "Hello, World!", "Hello", MB_OK | MB_ICONINFORMATION);"#);
            has(&l, "} else if (LOWORD(wParam) == 2) {");
            has(&l, "PostQuitMessage(0);");
            has(&l, "return DefWindowProcA(hwnd, uMsg, wParam, lParam);");
        }
    }

    #[test]
    fn winmain_matches_source() {
        for bin in [WIN64, WIN32] {
            let l = pseudo(bin, "WinMain");
            has(&l, "memset(&wc, 0, sizeof(wc));");
            has(&l, "wc.lpfnWndProc = WndProc;");
            has(&l, "wc.hCursor = LoadCursorA(NULL, IDC_ARROW);");
            has(&l, "wc.hbrBackground = (HBRUSH)(COLOR_WINDOW + 1);");
            has(&l, r#"wc.lpszClassName = "HelloMenu";"#);
            has(&l, "RegisterClassA(&wc);");
            has(&l, r#"hWnd = CreateWindowExA(0, "HelloMenu", "Teste - Hello World", WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT, 400, 220, NULL, NULL, hInstance, NULL);"#);
            has(&l, "ShowWindow(hWnd, nShowCmd);");
            has(&l, "while (GetMessageA(&msg, NULL, 0, 0) > 0) {");
            has(&l, "TranslateMessage(&msg);");
            has(&l, "return 0;");
        }
    }

    #[test]
    fn elf_switch_loop_and_strings() {
        let l = pseudo(ELF, "dia");
        has(&l, "switch (param_1) {");
        has(&l, r#"return "sabado";"#);
        has(&l, r#"return "invalido";"#);
        let m = pseudo(ELF, "main");
        has(&m, r#"puts("sem argumento");"#);
        assert!(m.iter().any(|x| x.contains(r#""fib=%d soma=%d\n""#)), "string com \\n");
        assert!(!m.iter().any(|x| x.contains("fs:0x28")), "canário de pilha deveria sumir");
    }

    #[test]
    fn braces_balance_everywhere() {
        for bin in [WIN64, WIN32, ELF] {
            let e = NativeEngine::analyze(bin.to_vec(), &mut |_| {});
            for f in e.list_functions() {
                let a = parse_address(&f.address).unwrap();
                let lines = e.lines(a, CodeKind::Pseudo).unwrap();
                let text: String = lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
                let open = text.matches('{').count() - text.matches("'{'").count();
                let close = text.matches('}').count() - text.matches("'}'").count();
                if text.contains("Decompilado") {
                    assert_eq!(open, close, "chaves desbalanceadas em {}", f.name);
                }
            }
        }
    }
}
