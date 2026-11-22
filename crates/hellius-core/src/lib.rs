//! Núcleo compartilhado entre a GUI (Tauri) e a versão de terminal.
//!
//! Regra de segurança: o arquivo analisado é apenas lido. Nunca é executado,
//! carregado como biblioteca ou aberto pelo sistema.

pub mod ai;
pub mod analyze;
pub mod engine;
pub mod error;
pub mod render;

pub use analyze::{open_native, summarize, summary_from_bytes, validate_path, FileSummary, Section, MAX_FILE_BYTES};
pub use engine::native::NativeEngine;
pub use engine::mock::MockEngine;
pub use engine::{format_address, parse_address, CodeKind, CodeLine, CodeSlice, Engine, FunctionInfo, ImportInfo, StringInfo};
pub use error::AppError;
