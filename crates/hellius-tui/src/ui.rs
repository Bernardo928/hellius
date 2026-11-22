//! Desenho da versão de terminal com ratatui. Fundo preto, um acento azul,
//! realce de sintaxe com a mesma paleta da GUI. Respeita NO_COLOR.

use hellius_core::CodeKind;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, Focus, ListTab, Mode, Row, Screen, Tone};
use crate::tokens::{self, Tok};

/// Paleta. Com NO_COLOR, tudo vira estilo padrão e a hierarquia fica por negrito e inversão.
pub struct Theme {
    pub color: bool,
}

impl Theme {
    pub fn from_env() -> Self {
        Self { color: std::env::var_os("NO_COLOR").is_none() }
    }
    fn fg(&self, c: Color) -> Style {
        if self.color { Style::new().fg(c) } else { Style::new() }
    }
    fn bg(&self, c: Color) -> Style {
        if self.color { Style::new().bg(c) } else { Style::new().add_modifier(Modifier::REVERSED) }
    }
    fn text(&self) -> Style { self.fg(Color::Rgb(0xf2, 0xf4, 0xf8)) }
    fn muted(&self) -> Style { self.fg(Color::Rgb(0xa4, 0xac, 0xba)) }
    fn faint(&self) -> Style { self.fg(Color::Rgb(0x6f, 0x77, 0x86)) }
    fn accent(&self) -> Style { self.fg(Color::Rgb(0x8a, 0xb4, 0xff)) }
    fn warn(&self) -> Style { self.fg(Color::Rgb(0xff, 0xd4, 0x79)) }
    fn error(&self) -> Style { self.fg(Color::Rgb(0xff, 0x7b, 0x8a)) }
    fn success(&self) -> Style { self.fg(Color::Rgb(0x7e, 0xe7, 0xa8)) }
    fn border(&self, focused: bool) -> Style {
        if focused { self.accent() } else { self.fg(Color::Rgb(0x2a, 0x2f, 0x3a)) }
    }
    fn tok(&self, t: Tok) -> Style {
        let s = match t {
            Tok::Mnemonic => self.fg(Color::Rgb(0xc7, 0x92, 0xea)).add_modifier(Modifier::BOLD),
            Tok::Reg => self.fg(Color::Rgb(0x82, 0xd8, 0xff)),
            Tok::Num => self.fg(Color::Rgb(0xf5, 0xc9, 0x7a)),
            Tok::Str => self.fg(Color::Rgb(0xa5, 0xe6, 0xa0)),
            Tok::Target => self.fg(Color::Rgb(0xff, 0x9e, 0x7a)).add_modifier(Modifier::UNDERLINED),
            Tok::Kw => self.fg(Color::Rgb(0xff, 0x7e, 0xb6)),
            Tok::Sym | Tok::Plain => self.text(),
        };
        if self.color { s } else { s.remove_modifier(Modifier::UNDERLINED) }
    }
    fn selected_bg(&self) -> Style { self.bg(Color::Rgb(0x16, 0x26, 0x4a)) }
    fn cursor_bg(&self) -> Style { self.bg(Color::Rgb(0x22, 0x2a, 0x3a)) }
    fn match_bg(&self) -> Style { self.bg(Color::Rgb(0x4a, 0x3a, 0x10)) }
}

pub fn draw(f: &mut Frame, app: &mut App, theme: &Theme) {
    let area = f.area();
    f.render_widget(Block::new().style(Style::new().bg(if theme.color { Color::Black } else { Color::Reset })), area);
    match app.screen {
        Screen::Open => draw_open(f, app, theme, area),
        Screen::Workspace => draw_workspace(f, app, theme, area),
    }
    if app.mode == Mode::Help {
        draw_help(f, theme, area);
    }
}

// ---------------- Tela de abertura ----------------

fn draw_open(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let rows = Layout::new(
        Direction::Vertical,
        [Constraint::Min(3), Constraint::Length(13), Constraint::Length(7), Constraint::Min(1)],
    )
    .split(area);

    // Portal: um anel de pontos com degradê azul→violeta e o nome no centro.
    f.render_widget(Paragraph::new(portal_lines(theme)).alignment(Alignment::Center), rows[1]);

    let box_w = area.width.saturating_sub(4).min(72);
    let input_area = centered(rows[2], box_w, 7);
    let status = match &app.status {
        Some((m, Tone::Error)) => Line::from(Span::styled(m.clone(), theme.error())),
        Some((m, _)) => Line::from(Span::styled(m.clone(), theme.muted())),
        None => Line::from(Span::styled("Enter para analisar · Esc para sair", theme.faint())),
    };
    let lines = vec![
        Line::from(Span::styled("Caminho do arquivo (exe, dll, so, elf, bin…)", theme.muted())),
        Line::from(vec![
            Span::styled("› ", theme.accent()),
            Span::styled(app.path_input.clone(), theme.text()),
            Span::styled("▏", theme.accent()),
        ]),
        Line::default(),
        status,
    ];
    f.render_widget(
        Paragraph::new(lines).block(
            Block::bordered().border_type(BorderType::Rounded).border_style(theme.border(true)).title(" Abrir "),
        ),
        input_area,
    );
}

fn portal_lines(theme: &Theme) -> Vec<Line<'static>> {
    // Células de terminal são ~2x mais altas que largas: o raio horizontal é o dobro.
    let (h, w) = (11i32, 44i32);
    let (cy, cx) = (h / 2, w / 2);
    let mut out = Vec::new();
    for y in 0..h {
        let mut spans = Vec::new();
        for x in 0..w {
            let dx = (x - cx) as f32 / 2.0;
            let dy = (y - cy) as f32;
            let d = (dx * dx + dy * dy).sqrt();
            let ch = if (d - 5.0).abs() < 0.55 {
                '●'
            } else if (d - 3.6).abs() < 0.35 && (x + y) % 3 == 0 {
                '·'
            } else {
                ' '
            };
            let angle = dy.atan2(dx);
            let t = (angle + std::f32::consts::PI) / (2.0 * std::f32::consts::PI);
            let color = Color::Rgb((0x2a as f32 + t * 80.0) as u8, (0x4b as f32 + (1.0 - t) * 120.0) as u8, 0xff);
            spans.push(Span::styled(ch.to_string(), theme.fg(color)));
        }
        if y == cy {
            // Nome no miolo do portal.
            let name = "H E L L I U S";
            let start = (cx - name.chars().count() as i32 / 2) as usize;
            for (i, c) in name.chars().enumerate() {
                spans[start + i] = Span::styled(c.to_string(), theme.text().add_modifier(Modifier::BOLD));
            }
        }
        out.push(Line::from(spans));
    }
    out
}

// ---------------- Área de trabalho ----------------

fn draw_workspace(f: &mut Frame, app: &mut App, theme: &Theme, area: Rect) {
    let rows = Layout::new(
        Direction::Vertical,
        [Constraint::Length(1), Constraint::Min(5), Constraint::Length(1), Constraint::Length(1)],
    )
    .split(area);
    draw_header(f, app, theme, rows[0]);

    let show_inspector = area.width >= 120;
    let cols = if show_inspector {
        Layout::new(Direction::Horizontal, [Constraint::Length(32), Constraint::Min(40), Constraint::Length(34)])
            .split(rows[1])
    } else {
        Layout::new(Direction::Horizontal, [Constraint::Length(30), Constraint::Min(30)]).split(rows[1])
    };
    draw_list(f, app, theme, cols[0]);
    draw_code(f, app, theme, cols[1]);
    if show_inspector {
        draw_inspector(f, app, theme, cols[2]);
    }
    draw_status(f, app, theme, rows[2]);
    draw_keys(f, app, theme, rows[3]);
}

fn draw_header(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let Some(s) = app.session() else { return };
    let sum = &s.summary;
    let bits = if sum.bits == 0 { String::new() } else { format!(" · {} bits", sum.bits) };
    let left = Line::from(vec![
        Span::styled(" Hellius ", theme.accent().add_modifier(Modifier::BOLD)),
        Span::styled(sum.name.clone(), theme.text().add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {} · {}{}", sum.format, sum.arch, bits), theme.muted()),
        Span::styled(format!("  sha256 {}…", &sum.sha256[..12.min(sum.sha256.len())]), theme.faint()),
    ]);
    f.render_widget(Paragraph::new(left), area);
    if s.engine.is_demo() {
        let badge = " MODO DEMONSTRAÇÃO — engine não conectada ";
        let w = badge.chars().count() as u16;
        if area.width > w + 40 {
            let r = Rect { x: area.x + area.width - w, y: area.y, width: w, height: 1 };
            f.render_widget(Paragraph::new(Span::styled(badge, theme.warn().add_modifier(Modifier::BOLD))), r);
        }
    }
}

fn draw_list(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let focused = app.focus == Focus::List;
    let tabs: Vec<Span> = ListTab::ALL
        .iter()
        .flat_map(|t| {
            let style = if *t == app.tab { theme.accent().add_modifier(Modifier::BOLD) } else { theme.faint() };
            [Span::styled(t.label(), style), Span::raw(" ")]
        })
        .collect();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border(focused))
        .title(Line::from(tabs));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let parts = Layout::new(Direction::Vertical, [Constraint::Length(1), Constraint::Min(1)]).split(inner);
    let filter_line = if app.mode == Mode::Filter {
        Line::from(vec![Span::styled("/ ", theme.accent()), Span::styled(app.filter.clone(), theme.text()), Span::styled("▏", theme.accent())])
    } else if app.filter.is_empty() {
        Line::from(Span::styled("/ filtrar", theme.faint()))
    } else {
        Line::from(vec![Span::styled("/ ", theme.faint()), Span::styled(app.filter.clone(), theme.muted())])
    };
    f.render_widget(Paragraph::new(filter_line), parts[0]);

    let items = app.list_items();
    if items.is_empty() {
        f.render_widget(Paragraph::new(Span::styled("Nada a mostrar.", theme.muted())), parts[1]);
        return;
    }
    let current_name = app.current.and_then(|c| app.session().map(|s| s.functions[c].name.clone()));
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|(title, meta, func)| {
            let is_current = app.tab == ListTab::Functions && func.is_some() && current_name.as_deref() == Some(title.as_str());
            let title_style = if is_current { theme.accent() } else { theme.text() };
            ListItem::new(vec![
                Line::from(Span::styled(title.clone(), title_style)),
                Line::from(Span::styled(meta.clone(), theme.faint())),
            ])
        })
        .collect();
    let mut state = ListState::default().with_selected(Some(app.list_index.min(items.len() - 1)));
    let list = List::new(list_items)
        .highlight_style(if focused { theme.cursor_bg() } else { Style::new() })
        .highlight_symbol(if focused { "▌" } else { " " });
    f.render_stateful_widget(list, parts[1], &mut state);
}

fn draw_code(f: &mut Frame, app: &mut App, theme: &Theme, area: Rect) {
    let focused = app.focus == Focus::Code;
    let kind_tabs = [(CodeKind::Disasm, "1 Disassembly"), (CodeKind::Pseudo, "2 Pseudocódigo"), (CodeKind::Hex, "3 Hex")];
    let mut title: Vec<Span> = kind_tabs
        .iter()
        .flat_map(|(k, label)| {
            let st = if *k == app.kind { theme.accent().add_modifier(Modifier::BOLD) } else { theme.faint() };
            [Span::styled(*label, st), Span::raw("  ")]
        })
        .collect();
    title.push(Span::styled(
        if app.program_scope { "p Programa inteiro" } else { "p Função" },
        theme.muted(),
    ));
    let demo = app.session().is_some_and(|s| s.engine.is_demo());
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border(focused))
        .title(Line::from(title));
    if demo {
        block = block.title_bottom(Line::from(Span::styled(" código de demonstração, não é o seu arquivo ", theme.warn())).right_aligned());
    }
    let inner = block.inner(area);
    f.render_widget(block, area);

    let (search_area, body) = if app.mode == Mode::Search || !app.matches.is_empty() {
        let p = Layout::new(Direction::Vertical, [Constraint::Length(1), Constraint::Min(1)]).split(inner);
        (Some(p[0]), p[1])
    } else {
        (None, inner)
    };
    if let Some(sa) = search_area {
        let info = if app.matches.is_empty() { String::new() } else { format!("  {} de {}", app.match_idx + 1, app.matches.len()) };
        let cursor = if app.mode == Mode::Search { "▏" } else { "" };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("buscar: ", theme.accent()),
                Span::styled(app.search.clone(), theme.text()),
                Span::styled(cursor, theme.accent()),
                Span::styled(info, theme.muted()),
            ])),
            sa,
        );
    }

    if app.rows.is_empty() {
        f.render_widget(Paragraph::new(Span::styled("Selecione uma função na lista (Enter).", theme.muted())), body);
        return;
    }
    let height = body.height as usize;
    app.ensure_visible(height);
    let end = (app.scroll + height).min(app.rows.len());
    let match_set: std::collections::HashSet<usize> = app.matches.iter().copied().collect();
    let show_bytes = app.kind != CodeKind::Pseudo;

    let mut lines = Vec::with_capacity(height);
    for r in app.scroll..end {
        let mut spans: Vec<Span> = Vec::new();
        let row_style = if app.select_all && !matches!(app.rows[r], Row::Blank) {
            theme.selected_bg()
        } else if match_set.contains(&r) {
            theme.match_bg()
        } else if r == app.cursor && focused {
            theme.cursor_bg()
        } else {
            Style::new()
        };
        let marker = if r == app.cursor { "▌" } else { " " };
        spans.push(Span::styled(marker, theme.accent()));
        spans.push(Span::styled(format!("{:>6} ", r + 1), theme.faint()));
        match app.rows[r] {
            Row::Header(func) => {
                let s = app.session().expect("sessão aberta");
                let fi = &s.functions[func];
                spans.push(Span::styled(format!("; ---- {} @ {} ----", fi.name, fi.address), theme.faint().add_modifier(Modifier::ITALIC)));
            }
            Row::Blank => {}
            Row::Line { .. } => {
                if let Some(l) = app.line_at(r) {
                    spans.push(Span::styled(format!("{:<13}", l.address), theme.fg(Color::Rgb(0x6f, 0x7f, 0xa3))));
                    if show_bytes {
                        spans.push(Span::styled(format!("{:<18}", l.bytes.clone().unwrap_or_default()), theme.faint()));
                    }
                    let toks = match app.kind {
                        CodeKind::Disasm => tokens::asm(&l.text),
                        CodeKind::Pseudo => tokens::c(&l.text),
                        CodeKind::Hex => vec![(Tok::Plain, l.text.as_str())],
                    };
                    for (k, t) in toks {
                        spans.push(Span::styled(t.to_string(), theme.tok(k)));
                    }
                    if let Some(c) = &l.comment {
                        spans.push(Span::styled(format!("  ; {c}"), theme.fg(Color::Rgb(0x6f, 0x8a, 0x6f))));
                    }
                }
            }
        }
        lines.push(Line::from(spans).style(row_style));
    }
    f.render_widget(Paragraph::new(lines), body);
}

fn draw_inspector(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme.border(false))
        .title(Span::styled(" Função ", theme.muted()));
    let Some(s) = app.session() else { return };
    let Some(fi) = app.current.and_then(|c| s.functions.get(c)) else {
        f.render_widget(Paragraph::new("Nenhuma função selecionada.").block(block), area);
        return;
    };
    let name_of = |addr: &str| s.functions.iter().find(|x| x.address == addr).map(|x| x.name.clone()).unwrap_or_default();
    let mut lines = vec![
        Line::from(Span::styled(fi.name.clone(), theme.text().add_modifier(Modifier::BOLD))),
        Line::default(),
        Line::from(vec![Span::styled("Endereço  ", theme.faint()), Span::styled(fi.address.clone(), theme.text())]),
        Line::from(vec![Span::styled("Tamanho   ", theme.faint()), Span::styled(format!("{} B", fi.size_bytes), theme.text())]),
        Line::from(vec![Span::styled("Blocos    ", theme.faint()), Span::styled(fi.block_count.to_string(), theme.text())]),
        Line::default(),
        Line::from(Span::styled("Referências de entrada", theme.muted())),
    ];
    let xref = |lines: &mut Vec<Line>, addrs: &[String]| {
        if addrs.is_empty() {
            lines.push(Line::from(Span::styled("  nenhuma", theme.faint())));
        }
        for a in addrs {
            lines.push(Line::from(vec![Span::styled(format!("  {a} "), theme.accent()), Span::styled(name_of(a), theme.faint())]));
        }
    };
    xref(&mut lines, &fi.xrefs_in);
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("Referências de saída", theme.muted())));
    xref(&mut lines, &fi.xrefs_out);
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("Use : ou Ctrl+G com o endereço para ir até ele.", theme.faint())));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }).block(block), area);
}

fn draw_status(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let addr = app.line_at(app.cursor).map(|l| l.address.clone()).unwrap_or_else(|| "—".into());
    let sel = if app.select_all { format!("{} linhas selecionadas", app.line_count()) } else { "seleção —".into() };
    let mut spans = vec![
        Span::styled(format!(" endereço {addr}"), theme.muted()),
        Span::styled(format!("   linha {}", app.cursor + 1), theme.muted()),
        Span::styled(format!("   {sel}"), theme.muted()),
    ];
    if app.mode == Mode::Goto {
        spans.push(Span::styled("   ir para: ", theme.accent()));
        spans.push(Span::styled(app.goto_input.clone(), theme.text()));
        spans.push(Span::styled("▏", theme.accent()));
    } else if let Some((m, tone)) = &app.status {
        let st = match tone {
            Tone::Error => theme.error(),
            Tone::Success => theme.success(),
            Tone::Info => theme.text(),
        };
        spans.push(Span::styled(format!("   {m}"), st));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_keys(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let hint = match app.mode {
        Mode::Filter | Mode::Search | Mode::Goto => "Enter confirma · Esc cancela",
        _ => "Tab foco · Enter abrir/seguir · / buscar · : ir para · a selecionar tudo · y copiar · 1/2/3 tipo · p escopo · [ ] histórico · ? ajuda · q sair",
    };
    f.render_widget(Paragraph::new(Span::styled(format!(" {hint}"), theme.faint())), area);
}

fn draw_help(f: &mut Frame, theme: &Theme, area: Rect) {
    let w = 64.min(area.width.saturating_sub(4));
    let h = 22.min(area.height.saturating_sub(2));
    let r = centered(area, w, h);
    let rows: &[(&str, &str)] = &[
        ("Tab", "alternar foco entre lista e código"),
        ("↑ ↓  j k  PgUp PgDn", "mover"),
        ("← →  h l", "trocar aba da lista"),
        ("Enter", "abrir função / seguir call e jmp"),
        ("/", "filtrar lista ou buscar no código"),
        ("n  N", "próxima / anterior ocorrência"),
        (":  Ctrl+G", "ir para endereço ou nome"),
        ("a  Ctrl+A", "selecionar tudo do escopo"),
        ("y  c", "copiar o escopo completo (OSC 52)"),
        ("1 2 3", "disassembly / pseudocódigo / hex"),
        ("p", "alternar função / programa inteiro"),
        ("[  ]  Alt+← Alt+→", "voltar / avançar"),
        ("o", "abrir outro arquivo"),
        ("Esc", "desfazer seleção e busca"),
        ("q", "sair"),
    ];
    let mut lines: Vec<Line> = rows
        .iter()
        .map(|(k, v)| Line::from(vec![Span::styled(format!("{k:<22}"), theme.accent()), Span::styled(*v, theme.text())]))
        .collect();
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("O arquivo nunca é executado. Sem rede.", theme.faint())));
    f.render_widget(Clear, r);
    f.render_widget(
        Paragraph::new(lines).block(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(theme.accent())
                .title(" Atalhos ")
                .style(Style::new().bg(if theme.color { Color::Black } else { Color::Reset })),
        ),
        r,
    );
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect { x: area.x + (area.width - w) / 2, y: area.y + (area.height - h) / 2, width: w, height: h }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn buffer_text(t: &Terminal<TestBackend>) -> String {
        let buf = t.backend().buffer();
        let mut s = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn open_screen_shows_portal_and_input() {
        let mut app = App::new();
        let mut t = Terminal::new(TestBackend::new(100, 32)).unwrap();
        t.draw(|f| draw(f, &mut app, &Theme { color: true })).unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("H E L L I U S"));
        assert!(text.contains("Caminho do arquivo"));
    }

    #[test]
    fn workspace_shows_demo_warning_and_code() {
        let dir = std::env::temp_dir().join("hellius-tui-ui");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("x.bin");
        std::fs::write(&p, b"abc").unwrap();
        let mut app = App::new();
        app.open(p.to_str().unwrap());
        let mut t = Terminal::new(TestBackend::new(140, 30)).unwrap();
        t.draw(|f| draw(f, &mut app, &Theme { color: false })).unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("MODO DEMONSTRAÇÃO"));
        assert!(text.contains("sub rsp"));
        assert!(text.contains("Referências de entrada"));
    }

    #[test]
    fn narrow_terminal_does_not_panic() {
        let dir = std::env::temp_dir().join("hellius-tui-ui2");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("y.bin");
        std::fs::write(&p, b"abc").unwrap();
        let mut app = App::new();
        app.open(p.to_str().unwrap());
        app.mode = Mode::Help;
        let mut t = Terminal::new(TestBackend::new(40, 12)).unwrap();
        t.draw(|f| draw(f, &mut app, &Theme { color: true })).unwrap();
    }
}
