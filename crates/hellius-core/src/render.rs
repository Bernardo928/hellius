//! Texto das linhas como aparece na tela, usado para copiar e para o terminal.

use crate::engine::{format_address, CodeKind, CodeLine, Engine};
use crate::error::AppError;

/// Uma linha formatada.
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

/// Todas as linhas de uma função, uma por linha de texto.
pub fn render_function(engine: &dyn Engine, address: u64, kind: CodeKind) -> Result<String, AppError> {
    Ok(engine
        .lines(address, kind)?
        .iter()
        .map(|l| render_line(l, kind))
        .collect::<Vec<_>>()
        .join("\n"))
}

/// O programa inteiro: cada função com um cabeçalho.
pub fn render_program(engine: &dyn Engine, kind: CodeKind) -> Result<String, AppError> {
    let mut out = String::new();
    for addr in engine.function_addresses() {
        let name = engine.function_name(addr).unwrap_or_default();
        out.push_str(&format!("; ---- {name} @ {} ----\n", format_address(addr)));
        out.push_str(&render_function(engine, addr, kind)?);
        out.push_str("\n\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::mock::{MockEngine, SYNTHETIC_ADDRESS, SYNTHETIC_LINES};

    #[test]
    fn full_function_text_has_every_line() {
        let e = MockEngine::new();
        let text = render_function(&e, SYNTHETIC_ADDRESS, CodeKind::Disasm).unwrap();
        assert_eq!(text.lines().count(), SYNTHETIC_LINES);
    }

    #[test]
    fn program_text_has_a_header_per_function() {
        let e = MockEngine::new();
        let text = render_program(&e, CodeKind::Pseudo).unwrap();
        let headers = text.lines().filter(|l| l.starts_with("; ---- ")).count();
        assert_eq!(headers, e.function_addresses().len());
    }
}
