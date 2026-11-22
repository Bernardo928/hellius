//! App desktop do Hellius (Tauri). A análise fica no crate `hellius-core`,
//! compartilhado com a versão de terminal.

mod commands;

use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

pub use commands::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            session: Mutex::new(None),
            opening: AtomicBool::new(false),
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
