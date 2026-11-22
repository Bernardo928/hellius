// Única porta de entrada para o Rust. A UI nunca chama Ghidra/rizin diretamente.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AiStatus,
  AnalysisProgress,
  CodeKind,
  CodeSlice,
  EngineInfo,
  FileSummary,
  FunctionInfo,
  FunctionTotal,
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
  getTotals: (kind: CodeKind) => invoke<FunctionTotal[]>('get_totals', { kind }),
  listStrings: () => invoke<StringInfo[]>('list_strings'),
  listImports: () => invoke<ImportInfo[]>('list_imports'),
  aiStatus: () => invoke<AiStatus>('ai_status'),
  aiSaveSettings: (p: { apiKey?: string; clearKey?: boolean; model?: string; consent?: boolean }) =>
    invoke<AiStatus>('ai_save_settings', {
      apiKey: p.apiKey ?? null,
      clearKey: p.clearKey ?? false,
      model: p.model ?? null,
      consent: p.consent ?? null,
    }),
  aiExplain: (functionAddress: string, requestId: number) =>
    invoke<string>('ai_explain', { functionAddress, requestId }),
  aiCancel: () => invoke<null>('ai_cancel'),
};

export function onAiDelta(cb: (d: { requestId: number; text: string }) => void): Promise<UnlistenFn> {
  return listen<{ requestId: number; text: string }>('ai:delta', (e) => cb(e.payload));
}

export function onAnalysisProgress(cb: (p: AnalysisProgress) => void): Promise<UnlistenFn> {
  return listen<AnalysisProgress>('analysis:progress', (e) => cb(e.payload));
}
