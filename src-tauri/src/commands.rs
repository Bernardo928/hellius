//! Comandos Tauri tipados. O front nunca recebe o binário inteiro: só metadados,
//! listas e fatias de código.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::analyze::{self, FileSummary};
use crate::engine::{
    format_address, mock::MockEngine, parse_address, CodeKind, CodeLine, CodeSlice, Engine,
    FunctionInfo, ImportInfo, StringInfo,
};
use crate::error::AppError;

/// Máximo de linhas devolvidas numa só fatia.
const MAX_SLICE: usize = 2_000;

pub struct Session {
    pub summary: FileSummary,
    pub engine: Box<dyn Engine>,
}

pub struct AppState {
    pub session: Mutex<Option<Session>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub name: String,
    pub is_demo: bool,
}

#[derive(Serialize, Clone)]
struct Progress {
    phase: String,
    step: u32,
    total: u32,
}

fn with_session<T>(
    state: &AppState,
    f: impl FnOnce(&Session) -> Result<T, AppError>,
) -> Result<T, String> {
    let guard = state.session.lock().map_err(|_| "Estado interno bloqueado.".to_string())?;
    let session = guard.as_ref().ok_or(AppError::NoSession).map_err(|e| e.to_string())?;
    f(session).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_file(
    path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<FileSummary, String> {
    let emitter = app.clone();
    let (summary, engine) = tauri::async_runtime::spawn_blocking(move || {
        let summary = analyze::summarize(&path, &mut |phase, step, total| {
            let _ = emitter.emit(
                "analysis:progress",
                Progress { phase: phase.to_string(), step, total },
            );
        })?;
        let _ = emitter.emit(
            "analysis:progress",
            Progress { phase: "Carregando engine".into(), step: 4, total: 4 },
        );
        let engine: Box<dyn Engine> = Box::new(MockEngine::new());
        Ok::<_, AppError>((summary, engine))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let mut guard = state.session.lock().map_err(|_| "Estado interno bloqueado.".to_string())?;
    *guard = Some(Session { summary: summary.clone(), engine });
    Ok(summary)
}

#[tauri::command]
pub fn engine_info(state: State<'_, AppState>) -> Result<EngineInfo, String> {
    with_session(&state, |s| {
        Ok(EngineInfo { name: s.engine.name().to_string(), is_demo: s.engine.is_demo() })
    })
}

#[tauri::command]
pub fn list_functions(state: State<'_, AppState>) -> Result<Vec<FunctionInfo>, String> {
    with_session(&state, |s| Ok(s.engine.list_functions()))
}

#[tauri::command]
pub fn get_code(
    function_address: String,
    kind: String,
    start_line: usize,
    count: usize,
    state: State<'_, AppState>,
) -> Result<CodeSlice, String> {
    with_session(&state, |s| {
        let addr = parse_address(&function_address)?;
        let k = CodeKind::parse(&kind)?;
        let all = s.engine.lines(addr, k)?;
        let total = all.len();
        let start = start_line.min(total);
        let end = start.saturating_add(count.min(MAX_SLICE)).min(total);
        Ok(CodeSlice {
            function_address: format_address(addr),
            kind: k,
            total_lines: total,
            start_line: start,
            lines: all[start..end].to_vec(),
        })
    })
}

/// Texto completo para copiar. `scope` = "function" ou "program".
#[tauri::command]
pub fn get_full_text(
    scope: String,
    kind: String,
    function_address: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    with_session(&state, |s| {
        let k = CodeKind::parse(&kind)?;
        match scope.as_str() {
            "function" => {
                let addr = parse_address(
                    function_address
                        .as_deref()
                        .ok_or_else(|| AppError::BadArgument("endereço da função ausente".into()))?,
                )?;
                Ok(render_all(s.engine.lines(addr, k)?, k))
            }
            "program" => {
                let mut out = String::new();
                for addr in s.engine.function_addresses() {
                    let name = s.engine.function_name(addr).unwrap_or_default();
                    out.push_str(&format!("; ---- {name} @ {} ----\n", format_address(addr)));
                    out.push_str(&render_all(s.engine.lines(addr, k)?, k));
                    out.push_str("\n\n");
                }
                Ok(out)
            }
            other => Err(AppError::BadArgument(other.to_string())),
        }
    })
}

#[tauri::command]
pub fn list_strings(state: State<'_, AppState>) -> Result<Vec<StringInfo>, String> {
    with_session(&state, |s| Ok(s.engine.list_strings()))
}

#[tauri::command]
pub fn list_imports(state: State<'_, AppState>) -> Result<Vec<ImportInfo>, String> {
    with_session(&state, |s| Ok(s.engine.list_imports()))
}

fn render_all(lines: &[CodeLine], kind: CodeKind) -> String {
    lines.iter().map(|l| render_line(l, kind)).collect::<Vec<_>>().join("\n")
}

/// Uma linha como aparece na tela, para cópia.
pub fn render_line(l: &CodeLine, kind: CodeKind) -> String {
    match kind {
        CodeKind::Pseudo => l.text.clone(),
        CodeKind::Hex => format!("{}  {:<47}  {}", l.address, l.bytes.as_deref().unwrap_or(""), l.text),
        CodeKind::Disasm => {
            let base = format!("{}  {:<14}  {}", l.address, l.bytes.as_deref().unwrap_or(""), l.text);
            match &l.comment {
                Some(c) => format!("{base}  ; {c}"),
                None => base,
            }
        }
    }
}
