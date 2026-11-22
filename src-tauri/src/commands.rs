//! Comandos Tauri tipados. O front nunca recebe o binário inteiro: só metadados,
//! listas e fatias de código.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use hellius_core::render::{render_function, render_program};
use hellius_core::{
    format_address, open_native, parse_address, AppError, CodeKind, CodeSlice, Engine, FileSummary,
    FunctionInfo, ImportInfo, StringInfo,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

/// Máximo de linhas devolvidas numa só fatia.
const MAX_SLICE: usize = 2_000;

pub struct Session {
    pub summary: FileSummary,
    pub engine: Box<dyn Engine>,
}

pub struct AppState {
    pub session: Mutex<Option<Session>>,
    /// Impede duas análises ao mesmo tempo sobrescrevendo a sessão uma da outra.
    pub opening: AtomicBool,
    /// Pedido de cancelamento da explicação em andamento.
    pub ai_cancel: std::sync::Arc<AtomicBool>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub name: String,
    pub is_demo: bool,
    /// A análise parou num limite de tamanho.
    pub partial: bool,
    /// Existe disassembler para a arquitetura do arquivo.
    pub has_code: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunctionTotal {
    pub address: String,
    pub total_lines: usize,
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

/// Libera a trava de abertura mesmo em caso de erro.
struct OpeningGuard<'a>(&'a AtomicBool);
impl Drop for OpeningGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[tauri::command]
pub async fn open_file(
    path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<FileSummary, String> {
    if state.opening.swap(true, Ordering::SeqCst) {
        return Err("Já existe uma análise em andamento. Aguarde terminar.".into());
    }
    let _guard = OpeningGuard(&state.opening);

    let emitter = app.clone();
    let (summary, engine) = tauri::async_runtime::spawn_blocking(move || {
        let (summary, engine) = open_native(&path, &mut |phase, step, total| {
            let _ = emitter.emit(
                "analysis:progress",
                Progress { phase: phase.to_string(), step, total },
            );
        })?;
        let engine: Box<dyn Engine> = Box::new(engine);
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
        Ok(EngineInfo {
            name: s.engine.name().to_string(),
            is_demo: s.engine.is_demo(),
            partial: s.engine.is_partial(),
            has_code: s.engine.has_code(),
        })
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
                render_function(s.engine.as_ref(), addr, k)
            }
            "program" => render_program(s.engine.as_ref(), k),
            other => Err(AppError::BadArgument(other.to_string())),
        }
    })
}

/// Total de linhas de cada função (para montar o escopo "programa inteiro" numa chamada só).
#[tauri::command]
pub fn get_totals(kind: String, state: State<'_, AppState>) -> Result<Vec<FunctionTotal>, String> {
    with_session(&state, |s| {
        let k = CodeKind::parse(&kind)?;
        s.engine
            .function_addresses()
            .into_iter()
            .map(|a| {
                Ok(FunctionTotal { address: format_address(a), total_lines: s.engine.lines(a, k)?.len() })
            })
            .collect()
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

// ------------------------------------------------------------------ IA

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AiStatus {
    /// Há chave disponível (salva no app ou em ANTHROPIC_API_KEY).
    pub configured: bool,
    /// "app" ou "env".
    pub source: Option<hellius_core::ai::KeySource>,
    pub model: String,
    pub consent: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AiDelta {
    request_id: u32,
    text: String,
}

fn ai_status_now() -> AiStatus {
    let cfg = hellius_core::ai::load_config();
    let key = cfg.resolve_key();
    AiStatus { configured: key.is_some(), source: key.map(|(_, s)| s), model: cfg.model(), consent: cfg.consent }
}

/// Estado da IA. A chave nunca volta para a interface, só se ela existe.
#[tauri::command]
pub fn ai_status() -> AiStatus {
    ai_status_now()
}

#[tauri::command]
pub fn ai_save_settings(
    api_key: Option<String>,
    clear_key: bool,
    model: Option<String>,
    consent: Option<bool>,
) -> Result<AiStatus, String> {
    let mut cfg = hellius_core::ai::load_config();
    if clear_key {
        cfg.api_key = None;
    }
    if let Some(k) = api_key.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
        cfg.api_key = Some(k);
    }
    if let Some(m) = model {
        cfg.model = Some(m.trim().to_string()).filter(|m| !m.is_empty());
    }
    if let Some(c) = consent {
        cfg.consent = c;
    }
    hellius_core::ai::save_config(&cfg).map_err(|e| format!("Não foi possível salvar os ajustes: {e}"))?;
    Ok(ai_status_now())
}

/// Explica uma função. O texto chega por eventos `ai:delta`; o retorno é o texto completo.
#[tauri::command]
pub async fn ai_explain(
    function_address: String,
    request_id: u32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let cfg = hellius_core::ai::load_config();
    let (key, _) = cfg.resolve_key().ok_or_else(|| hellius_core::ai::AiError::NoKey.to_string())?;
    if !cfg.consent {
        return Err("É preciso autorizar o envio do código para a Anthropic antes de usar a IA.".into());
    }
    // Monta o pedido com a sessão travada e solta a trava antes da rede.
    let input = with_session(&state, |s| {
        let addr = parse_address(&function_address)?;
        hellius_core::ai::input_for_function(&s.summary, s.engine.as_ref(), addr)
    })?;
    state.ai_cancel.store(false, Ordering::SeqCst);
    let model = cfg.model();
    let cancel = state.inner().ai_cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        hellius_core::ai::explain(&key, &model, &input, &cancel, &mut |t| {
            let _ = app.emit("ai:delta", AiDelta { request_id, text: t.to_string() });
        })
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ai_cancel(state: State<'_, AppState>) {
    state.ai_cancel.store(true, Ordering::SeqCst);
}
