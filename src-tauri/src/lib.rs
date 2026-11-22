//! Núcleo do Hellius: validação e leitura de metadados de arquivos,
//! motor de análise (trait `Engine`) e comandos Tauri tipados.
//!
//! Regra de segurança: o arquivo analisado é apenas lido. Nunca é executado,
//! carregado como biblioteca ou aberto pelo sistema.

mod analyze;
mod commands;
pub mod engine;
mod error;

use std::sync::Mutex;

pub use commands::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            session: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::open_file,
            commands::list_functions,
            commands::get_code,
            commands::get_full_text,
            commands::list_strings,
            commands::list_imports,
            commands::engine_info,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Hellius");
}
