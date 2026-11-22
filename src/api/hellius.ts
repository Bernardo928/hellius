// Única porta de entrada para o Rust. A UI nunca chama Ghidra/rizin diretamente.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AnalysisProgress,
  CodeKind,
  CodeSlice,
  EngineInfo,
  FileSummary,
  FunctionInfo,
  ImportInfo,
  Scope,
  StringInfo,
} from './types';

export const api = {
  openFile: (path: string) => invoke<FileSummary>('open_file', { path }),
  engineInfo: () => invoke<EngineInfo>('engine_info'),
  listFunctions: () => invoke<FunctionInfo[]>('list_functions'),
  getCode: (functionAddress: string, kind: CodeKind, startLine: number, count: number) =>
    invoke<CodeSlice>('get_code', { functionAddress, kind, startLine, count }),
  getFullText: (scope: Scope, kind: CodeKind, functionAddress?: string) =>
    invoke<string>('get_full_text', { scope, kind, functionAddress: functionAddress ?? null }),
  listStrings: () => invoke<StringInfo[]>('list_strings'),
  listImports: () => invoke<ImportInfo[]>('list_imports'),
};

export function onAnalysisProgress(cb: (p: AnalysisProgress) => void): Promise<UnlistenFn> {
  return listen<AnalysisProgress>('analysis:progress', (e) => cb(e.payload));
}
