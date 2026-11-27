//! Hellius no terminal.
//!
//! Uso: `hellius-tui [arquivo]`. Sem argumento, abre a tela de abertura.
//! O arquivo é apenas lido; nunca é executado. Não há chamadas de rede.

mod app;
mod tokens;
mod ui;

use std::io::{self, Write};
use std::time::Duration;

use app::{App, Screen};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

const USAGE: &str = "Hellius (terminal) 0.3.0

Uso:
  hellius-tui [arquivo]

Sem arquivo, abre a tela de abertura para digitar o caminho.
Dentro do app, ? mostra os atalhos.

Variáveis:
  NO_COLOR=1   desliga as cores";

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    let first = args.next();
    match first.as_deref() {
        Some("-h") | Some("--help") => {
            println!("{USAGE}");
            return Ok(());
        }
        Some("-V") | Some("--version") => {
            println!("hellius-tui 0.3.0");
            return Ok(());
        }
        _ => {}
    }

    let mut app = App::new();
    if let Some(path) = first {
        app.open(&path);
        if app.screen == Screen::Open {
            app.path_input = path;
        }
    }

    let theme = ui::Theme::from_env();
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app, &theme);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App, theme: &ui::Theme) -> io::Result<()> {
    while !app.quit {
        terminal.draw(|f| ui::draw(f, app, theme))?;

        // Abre depois de desenhar "Analisando…", para o usuário ver o progresso.
        if let Some(path) = app.pending_open.take() {
            app.open(&path);
            continue;
        }
        if let Some(text) = app.pending_copy.take() {
            copy_osc52(&text)?;
        }

        if app.poll_ai() {
            continue;
        }
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.on_key(key);
                }
            }
        }
    }
    Ok(())
}

/// Copia via OSC 52: funciona em terminais que permitem (a maioria dos modernos,
/// às vezes atrás de uma opção). Não depende de servidor gráfico.
fn copy_osc52(text: &str) -> io::Result<()> {
    let mut out = io::stdout();
    write!(out, "\x1b]52;c;{}\x07", base64(text.as_bytes()))?;
    out.flush()
}

fn base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
