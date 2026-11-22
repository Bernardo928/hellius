//! Contrato de análise. A UI fala só com estes tipos via comandos Tauri.
//! Implementações: `NativeEngine` (análise real, capstone) e `MockEngine` (dados fixos para testes).
//! Futuras: `GhidraEngine`, `RizinEngine` (decompilação de verdade).
//! Veja `docs/ENGINE.md`.

pub mod mock;
pub mod native;

use serde::Serialize;

use crate::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodeKind {
    Disasm,
    Pseudo,
    Hex,
}

impl CodeKind {
    pub fn parse(s: &str) -> Result<Self, AppError> {
        match s {
            "disasm" => Ok(Self::Disasm),
            "pseudo" => Ok(Self::Pseudo),
            "hex" => Ok(Self::Hex),
            other => Err(AppError::BadArgument(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeLine {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Endereço de destino de `call`/`jmp`, quando houver.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FunctionInfo {
    pub address: String,
    pub name: String,
    pub size_bytes: u64,
    pub block_count: u32,
    pub xrefs_in: Vec<String>,
    pub xrefs_out: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeSlice {
    pub function_address: String,
    pub kind: CodeKind,
    pub total_lines: usize,
    pub start_line: usize,
    pub lines: Vec<CodeLine>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StringInfo {
    pub address: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportInfo {
    pub library: String,
    pub name: String,
}

pub trait Engine: Send + Sync {
    /// Nome exibido na barra de status, por exemplo "Demonstração (MockEngine)".
    fn name(&self) -> &'static str;
    /// Se o engine é real. `false` faz a UI mostrar o aviso de demonstração.
    fn is_demo(&self) -> bool;
    fn list_functions(&self) -> Vec<FunctionInfo>;
    /// Todas as linhas de uma função para o tipo pedido.
    fn lines(&self, function_address: u64, kind: CodeKind) -> Result<&[CodeLine], AppError>;
    fn list_strings(&self) -> Vec<StringInfo>;
    fn list_imports(&self) -> Vec<ImportInfo>;
    /// Endereço inicial de cada função (para o escopo "programa inteiro").
    fn function_addresses(&self) -> Vec<u64>;
    fn function_name(&self, address: u64) -> Option<String>;
    /// A análise parou num limite (arquivo muito grande).
    fn is_partial(&self) -> bool {
        false
    }
    /// Há disassembler para esta arquitetura.
    fn has_code(&self) -> bool {
        true
    }
}

/// Converte "0x140001000" ou "140001000" em número.
pub fn parse_address(s: &str) -> Result<u64, AppError> {
    let t = s.trim();
    let digits = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    u64::from_str_radix(digits, 16).map_err(|_| AppError::BadAddress(s.to_string()))
}

pub fn format_address(a: u64) -> String {
    format!("0x{a:x}")
}
