// Espelho dos tipos Rust (src-tauri/src/engine/mod.rs e analyze.rs). Mantenha em sincronia.

export type Arch = 'x86' | 'x86_64' | 'arm' | 'aarch64' | 'mips' | 'unknown';
export type Format = 'PE' | 'ELF' | 'MachO' | 'Raw';
/** 0 significa desconhecido (formato Raw). */
export type Bits = 0 | 16 | 32 | 64;
export type CodeKind = 'disasm' | 'pseudo' | 'hex';
export type Scope = 'function' | 'program';

export interface Section {
  name: string;
  address: string;
  size: number;
  flags: string;
}

export interface FileSummary {
  path: string;
  name: string;
  sizeBytes: number;
  sha256: string;
  format: Format;
  arch: Arch;
  bits: Bits;
  entryPoint: string;
  sections: Section[];
}

export interface FunctionInfo {
  address: string;
  name: string;
  sizeBytes: number;
  blockCount: number;
  xrefsIn: string[];
  xrefsOut: string[];
}

export interface CodeLine {
  address: string;
  bytes?: string;
  text: string;
  comment?: string;
  target?: string;
}

export interface CodeSlice {
  functionAddress: string;
  kind: CodeKind;
  totalLines: number;
  startLine: number;
  lines: CodeLine[];
}

export interface StringInfo {
  address: string;
  value: string;
}

export interface ImportInfo {
  library: string;
  name: string;
}

export interface EngineInfo {
  name: string;
  isDemo: boolean;
  /** A análise parou num limite de tamanho. */
  partial: boolean;
  /** Existe disassembler para a arquitetura do arquivo. */
  hasCode: boolean;
}

export interface FunctionTotal {
  address: string;
  totalLines: number;
}

export interface AnalysisProgress {
  phase: string;
  step: number;
  total: number;
}

export interface AiStatus {
  configured: boolean;
  /** Onde está a chave: no app ou na variável de ambiente ANTHROPIC_API_KEY. */
  source: 'app' | 'env' | null;
  model: string;
  consent: boolean;
}
