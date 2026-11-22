//! Estado e regras da versão de terminal. Sem desenho aqui: só lógica testável.

use hellius_core::render::{render_function, render_line, render_program};
use hellius_core::{
    parse_address, summarize, CodeKind, CodeLine, Engine, FileSummary, FunctionInfo, ImportInfo,
    MockEngine, StringInfo,
};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Open,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Code,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    /// Digitando o filtro da lista.
    Filter,
    /// Digitando a busca no código.
    Search,
    /// Digitando endereço ou nome (Ctrl+G ou `:`).
    Goto,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListTab {
    Functions,
    Strings,
    Imports,
    Sections,
}

impl ListTab {
    pub const ALL: [ListTab; 4] = [ListTab::Functions, ListTab::Strings, ListTab::Imports, ListTab::Sections];
    pub fn label(self) -> &'static str {
        match self {
            ListTab::Functions => "Funções",
            ListTab::Strings => "Strings",
            ListTab::Imports => "Imports",
            ListTab::Sections => "Seções",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// Cabeçalho de função no escopo "programa inteiro" (índice da função).
    Header(usize),
    /// Linha `idx` da função `func`.
    Line { func: usize, idx: usize },
    Blank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Info,
    Success,
    Error,
}

pub struct Session {
    pub summary: FileSummary,
    pub engine: Box<dyn Engine>,
    pub functions: Vec<FunctionInfo>,
    pub strings: Vec<StringInfo>,
    pub imports: Vec<ImportInfo>,
}

pub struct App {
    pub screen: Screen,
    pub session: Option<Session>,
    pub path_input: String,
    /// Caminho a abrir no próximo ciclo (depois de desenhar "Analisando…").
    pub pending_open: Option<String>,
    pub focus: Focus,
    pub mode: Mode,
    pub tab: ListTab,
    pub filter: String,
    pub list_index: usize,
    pub current: Option<usize>,
    pub kind: CodeKind,
    pub program_scope: bool,
    pub rows: Vec<Row>,
    pub cursor: usize,
    pub scroll: usize,
    pub select_all: bool,
    pub search: String,
    pub matches: Vec<usize>,
    pub match_idx: usize,
    pub goto_input: String,
    pub history: Vec<usize>,
    pub history_idx: usize,
    pub status: Option<(String, Tone)>,
    pub quit: bool,
    /// Texto que o laço principal deve mandar para a área de transferência (OSC 52).
    pub pending_copy: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Open,
            session: None,
            path_input: String::new(),
            pending_open: None,
            focus: Focus::List,
            mode: Mode::Normal,
            tab: ListTab::Functions,
            filter: String::new(),
            list_index: 0,
            current: None,
            kind: CodeKind::Disasm,
            program_scope: false,
            rows: Vec::new(),
            cursor: 0,
            scroll: 0,
            select_all: false,
            search: String::new(),
            matches: Vec::new(),
            match_idx: 0,
            goto_input: String::new(),
            history: Vec::new(),
            history_idx: 0,
            status: None,
            quit: false,
            pending_copy: None,
        }
    }

    fn say(&mut self, msg: impl Into<String>, tone: Tone) {
        self.status = Some((msg.into(), tone));
    }

    // ---------- Abertura ----------

    /// Valida e analisa o arquivo. Em erro, volta à tela de abertura com a mensagem.
    pub fn open(&mut self, path: &str) {
        match summarize(path, &mut |_, _, _| {}) {
            Ok(summary) => {
                let engine: Box<dyn Engine> = Box::new(MockEngine::new());
                let functions = engine.list_functions();
                let strings = engine.list_strings();
                let imports = engine.list_imports();
                self.session = Some(Session { summary, engine, functions, strings, imports });
                self.screen = Screen::Workspace;
                self.focus = Focus::List;
                self.mode = Mode::Normal;
                self.tab = ListTab::Functions;
                self.filter.clear();
                self.list_index = 0;
                self.program_scope = false;
                self.kind = CodeKind::Disasm;
                self.history.clear();
                self.history_idx = 0;
                self.current = None;
                self.select_function(0, true);
                self.say("Arquivo aberto. Modo demonstração: o código não é do seu arquivo.", Tone::Info);
            }
            Err(e) => {
                self.screen = Screen::Open;
                self.say(format!("Não foi possível abrir: {e}"), Tone::Error);
            }
        }
    }

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    pub fn function_address(&self, func: usize) -> Option<u64> {
        let s = self.session.as_ref()?;
        parse_address(&s.functions.get(func)?.address).ok()
    }

    /// Linhas de uma função para o tipo atual.
    pub fn lines_of(&self, func: usize) -> &[CodeLine] {
        let Some(s) = self.session.as_ref() else { return &[] };
        let Some(addr) = self.function_address(func) else { return &[] };
        s.engine.lines(addr, self.kind).unwrap_or(&[])
    }

    pub fn line_at(&self, row: usize) -> Option<&CodeLine> {
        match self.rows.get(row)? {
            Row::Line { func, idx } => self.lines_of(*func).get(*idx),
            _ => None,
        }
    }

    // ---------- Escopo e linhas ----------

    pub fn rebuild_rows(&mut self) {
        let mut rows = Vec::new();
        let n_funcs = self.session.as_ref().map_or(0, |s| s.functions.len());
        if self.program_scope {
            for f in 0..n_funcs {
                rows.push(Row::Header(f));
                let n = self.lines_of(f).len();
                rows.extend((0..n).map(|idx| Row::Line { func: f, idx }));
                rows.push(Row::Blank);
            }
        } else if let Some(f) = self.current {
            let n = self.lines_of(f).len();
            rows.extend((0..n).map(|idx| Row::Line { func: f, idx }));
        }
        self.rows = rows;
        self.cursor = 0;
        self.scroll = 0;
        self.select_all = false;
        self.matches.clear();
        self.match_idx = 0;
    }

    pub fn line_count(&self) -> usize {
        self.rows.iter().filter(|r| matches!(r, Row::Line { .. })).count()
    }

    pub fn select_function(&mut self, func: usize, record: bool) {
        let n = self.session.as_ref().map_or(0, |s| s.functions.len());
        if func >= n {
            return;
        }
        if record {
            self.history.truncate(self.history_idx + usize::from(!self.history.is_empty()));
            if self.history.last() != Some(&func) {
                self.history.push(func);
            }
            self.history_idx = self.history.len() - 1;
        }
        self.current = Some(func);
        self.program_scope = false;
        self.rebuild_rows();
    }

    pub fn back(&mut self) {
        if self.history_idx > 0 {
            self.history_idx -= 1;
            let f = self.history[self.history_idx];
            self.select_function(f, false);
        }
    }

    pub fn forward(&mut self) {
        if self.history_idx + 1 < self.history.len() {
            self.history_idx += 1;
            let f = self.history[self.history_idx];
            self.select_function(f, false);
        }
    }

    pub fn set_kind(&mut self, kind: CodeKind) {
        if self.kind != kind {
            self.kind = kind;
            self.rebuild_rows();
        }
    }

    pub fn toggle_scope(&mut self) {
        self.program_scope = !self.program_scope;
        self.rebuild_rows();
    }

    // ---------- Lista ----------

    /// Itens visíveis da aba atual, já filtrados: (título, detalhe, índice da função se houver).
    pub fn list_items(&self) -> Vec<(String, String, Option<usize>)> {
        let Some(s) = self.session.as_ref() else { return vec![] };
        let q = self.filter.trim().to_lowercase();
        let keep = |hay: &str| q.is_empty() || hay.to_lowercase().contains(&q);
        match self.tab {
            ListTab::Functions => s
                .functions
                .iter()
                .enumerate()
                .filter(|(_, f)| keep(&format!("{} {}", f.name, f.address)))
                .map(|(i, f)| (f.name.clone(), format!("{} · {} B", f.address, f.size_bytes), Some(i)))
                .collect(),
            ListTab::Strings => s
                .strings
                .iter()
                .filter(|x| keep(&format!("{} {}", x.value, x.address)))
                .map(|x| (x.value.clone(), x.address.clone(), None))
                .collect(),
            ListTab::Imports => s
                .imports
                .iter()
                .filter(|x| keep(&format!("{} {}", x.name, x.library)))
                .map(|x| (x.name.clone(), x.library.clone(), None))
                .collect(),
            ListTab::Sections => s
                .summary
                .sections
                .iter()
                .filter(|x| keep(&format!("{} {}", x.name, x.address)))
                .map(|x| {
                    let name = if x.name.is_empty() { "(sem nome)".to_string() } else { x.name.clone() };
                    (name, format!("{} · {} B · {}", x.address, x.size, x.flags), None)
                })
                .collect(),
        }
    }

    // ---------- Seleção e cópia ----------

    /// Texto completo do escopo atual (o que "Copiar" entrega).
    pub fn scope_text(&self) -> Option<String> {
        let s = self.session.as_ref()?;
        if self.program_scope {
            render_program(s.engine.as_ref(), self.kind).ok()
        } else {
            let addr = self.function_address(self.current?)?;
            render_function(s.engine.as_ref(), addr, self.kind).ok()
        }
    }

    pub fn copy_scope(&mut self) {
        match self.scope_text() {
            Some(text) => {
                let n = text.lines().count();
                self.pending_copy = Some(text);
                self.say(
                    format!("Copiado: {n} linhas (OSC 52; o terminal precisa permitir)"),
                    Tone::Success,
                );
            }
            None => self.say("Nada para copiar.", Tone::Error),
        }
    }

    // ---------- Busca ----------

    pub fn row_text(&self, row: usize) -> String {
        match self.rows.get(row) {
            Some(Row::Line { .. }) => self.line_at(row).map(|l| render_line(l, self.kind)).unwrap_or_default(),
            Some(Row::Header(f)) => self
                .session
                .as_ref()
                .and_then(|s| s.functions.get(*f))
                .map(|f| format!("; ---- {} @ {} ----", f.name, f.address))
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    pub fn run_search(&mut self) {
        let q = self.search.trim().to_lowercase();
        self.matches = if q.is_empty() {
            vec![]
        } else {
            (0..self.rows.len()).filter(|&i| self.row_text(i).to_lowercase().contains(&q)).collect()
        };
        self.match_idx = 0;
        match self.matches.first() {
            Some(&r) => {
                self.cursor = r;
                self.say(format!("{} ocorrência(s)", self.matches.len()), Tone::Info);
            }
            None if !q.is_empty() => self.say("Nenhuma ocorrência", Tone::Info),
            None => {}
        }
    }

    pub fn step_match(&mut self, forward: bool) {
        if self.matches.is_empty() {
            return;
        }
        let n = self.matches.len();
        self.match_idx = if forward { (self.match_idx + 1) % n } else { (self.match_idx + n - 1) % n };
        self.cursor = self.matches[self.match_idx];
        self.say(format!("{} de {}", self.match_idx + 1, n), Tone::Info);
    }

    // ---------- Navegação ----------

    /// Vai para um endereço (início de função ou dentro dela) ou para uma função pelo nome.
    pub fn goto(&mut self, input: &str) -> bool {
        let text = input.trim();
        if text.is_empty() {
            return false;
        }
        let Some(s) = self.session.as_ref() else { return false };
        let addrs: Vec<(u64, u64)> = s
            .functions
            .iter()
            .map(|f| (parse_address(&f.address).unwrap_or(u64::MAX), f.size_bytes))
            .collect();
        if let Ok(target) = parse_address(text) {
            if let Some(i) = addrs.iter().position(|(a, _)| *a == target) {
                self.select_function(i, true);
                return true;
            }
            if let Some(i) = addrs.iter().position(|(a, size)| target > *a && target < a.saturating_add(*size)) {
                self.select_function(i, true);
                if let Some(row) = (0..self.rows.len())
                    .find(|&r| self.line_at(r).is_some_and(|l| parse_address(&l.address).ok() == Some(target)))
                {
                    self.cursor = row;
                }
                return true;
            }
        }
        let lower = text.to_lowercase();
        if let Some(i) = s.functions.iter().position(|f| f.name.to_lowercase().contains(&lower)) {
            self.select_function(i, true);
            return true;
        }
        self.say(format!("Nada encontrado para \"{text}\""), Tone::Error);
        false
    }

    /// Segue o alvo de call/jmp da linha sob o cursor.
    pub fn follow_target(&mut self) {
        let target = self.line_at(self.cursor).and_then(|l| l.target.clone());
        match target {
            Some(t) => {
                self.goto(&t);
            }
            None => self.say("Esta linha não tem destino de salto ou chamada.", Tone::Info),
        }
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let max = self.rows.len() as isize - 1;
        self.cursor = (self.cursor as isize + delta).clamp(0, max) as usize;
    }

    fn move_list(&mut self, delta: isize) {
        let n = self.list_items().len();
        if n == 0 {
            self.list_index = 0;
            return;
        }
        self.list_index = (self.list_index as isize + delta).clamp(0, n as isize - 1) as usize;
    }

    /// Ajusta a rolagem para o cursor ficar visível numa janela de `height` linhas.
    pub fn ensure_visible(&mut self, height: usize) {
        if height == 0 {
            return;
        }
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + height {
            self.scroll = self.cursor + 1 - height;
        }
    }

    // ---------- Teclado ----------

    pub fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        if ctrl && key.code == KeyCode::Char('c') && self.mode == Mode::Normal && self.screen == Screen::Open {
            self.quit = true;
            return;
        }

        match self.screen {
            Screen::Open => self.on_key_open(key),
            Screen::Workspace => match self.mode {
                Mode::Help => {
                    if matches!(key.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Enter) {
                        self.mode = Mode::Normal;
                    }
                }
                Mode::Filter => self.on_text_input(key, InputTarget::Filter),
                Mode::Search => self.on_text_input(key, InputTarget::Search),
                Mode::Goto => self.on_text_input(key, InputTarget::Goto),
                Mode::Normal => self.on_key_normal(key, ctrl, alt),
            },
        }
    }

    fn on_key_open(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if self.session.is_some() {
                    self.screen = Screen::Workspace;
                } else {
                    self.quit = true;
                }
            }
            KeyCode::Enter => {
                let p = self.path_input.trim().to_string();
                if p.is_empty() {
                    self.say("Digite o caminho de um arquivo.", Tone::Error);
                } else {
                    self.pending_open = Some(p);
                    self.say("Analisando…", Tone::Info);
                }
            }
            KeyCode::Backspace => {
                self.path_input.pop();
            }
            KeyCode::Char(c) => self.path_input.push(c),
            _ => {}
        }
    }

    fn on_text_input(&mut self, key: KeyEvent, target: InputTarget) {
        let buf = match target {
            InputTarget::Filter => &mut self.filter,
            InputTarget::Search => &mut self.search,
            InputTarget::Goto => &mut self.goto_input,
        };
        match key.code {
            KeyCode::Esc => {
                if target != InputTarget::Filter {
                    buf.clear();
                }
                if target == InputTarget::Search {
                    self.matches.clear();
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                match target {
                    InputTarget::Filter => self.list_index = 0,
                    InputTarget::Search => self.run_search(),
                    InputTarget::Goto => {
                        let q = std::mem::take(&mut self.goto_input);
                        if self.goto(&q) {
                            self.focus = Focus::Code;
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                buf.pop();
                if target == InputTarget::Filter {
                    self.list_index = 0;
                }
            }
            KeyCode::Char(c) => {
                buf.push(c);
                if target == InputTarget::Filter {
                    self.list_index = 0;
                }
            }
            _ => {}
        }
    }

    fn on_key_normal(&mut self, key: KeyEvent, ctrl: bool, alt: bool) {
        // Atalhos válidos em qualquer foco.
        match key.code {
            KeyCode::Char('q') if !ctrl => {
                self.quit = true;
                return;
            }
            KeyCode::Char('?') => {
                self.mode = Mode::Help;
                return;
            }
            KeyCode::Char('g') if ctrl => {
                self.mode = Mode::Goto;
                return;
            }
            KeyCode::Char(':') => {
                self.mode = Mode::Goto;
                return;
            }
            KeyCode::Char('o') if !ctrl => {
                self.screen = Screen::Open;
                self.path_input.clear();
                return;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::List { Focus::Code } else { Focus::List };
                return;
            }
            KeyCode::Left if alt => {
                self.back();
                return;
            }
            KeyCode::Right if alt => {
                self.forward();
                return;
            }
            KeyCode::Char('[') => {
                self.back();
                return;
            }
            KeyCode::Char(']') => {
                self.forward();
                return;
            }
            KeyCode::Char('1') => {
                self.set_kind(CodeKind::Disasm);
                return;
            }
            KeyCode::Char('2') => {
                self.set_kind(CodeKind::Pseudo);
                return;
            }
            KeyCode::Char('3') => {
                self.set_kind(CodeKind::Hex);
                return;
            }
            KeyCode::Char('p') => {
                self.toggle_scope();
                return;
            }
            KeyCode::Char('a') if ctrl => {
                self.select_all = true;
                self.focus = Focus::Code;
                self.say(format!("{} linhas selecionadas", self.line_count()), Tone::Info);
                return;
            }
            KeyCode::Char('a') => {
                self.select_all = !self.select_all;
                if self.select_all {
                    self.say(format!("{} linhas selecionadas", self.line_count()), Tone::Info);
                }
                return;
            }
            KeyCode::Char('y') | KeyCode::Char('c') => {
                self.copy_scope();
                return;
            }
            KeyCode::Esc => {
                self.select_all = false;
                self.matches.clear();
                self.status = None;
                return;
            }
            _ => {}
        }

        match self.focus {
            Focus::List => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_list(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_list(1),
                KeyCode::PageUp => self.move_list(-10),
                KeyCode::PageDown => self.move_list(10),
                KeyCode::Home => self.list_index = 0,
                KeyCode::End => self.move_list(isize::MAX / 2),
                KeyCode::Left | KeyCode::Char('h') => {
                    let i = ListTab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
                    self.tab = ListTab::ALL[(i + ListTab::ALL.len() - 1) % ListTab::ALL.len()];
                    self.list_index = 0;
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    let i = ListTab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
                    self.tab = ListTab::ALL[(i + 1) % ListTab::ALL.len()];
                    self.list_index = 0;
                }
                KeyCode::Char('/') => self.mode = Mode::Filter,
                KeyCode::Enter => {
                    let item = self.list_items().get(self.list_index).and_then(|(_, _, f)| *f);
                    if let Some(f) = item {
                        self.select_function(f, true);
                        self.focus = Focus::Code;
                    }
                }
                _ => {}
            },
            Focus::Code => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1),
                KeyCode::PageUp => self.move_cursor(-20),
                KeyCode::PageDown => self.move_cursor(20),
                KeyCode::Home => self.cursor = 0,
                KeyCode::End => self.cursor = self.rows.len().saturating_sub(1),
                KeyCode::Char('/') => self.mode = Mode::Search,
                KeyCode::Char('n') => self.step_match(true),
                KeyCode::Char('N') => self.step_match(false),
                KeyCode::Enter => self.follow_target(),
                _ => {}
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputTarget {
    Filter,
    Search,
    Goto,
}

#[cfg(test)]
mod tests {
    use super::*;
    use hellius_core::engine::mock::SYNTHETIC_LINES;
    use ratatui::crossterm::event::{KeyEvent, KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
    }
    fn ctrl(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn opened() -> App {
        // Arquivo próprio por teste: os testes rodam em paralelo.
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("hellius-tui-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("amostra.bin");
        std::fs::write(&f, b"conteudo de teste").unwrap();
        let mut app = App::new();
        app.open(f.to_str().unwrap());
        app
    }

    fn synthetic_index(app: &App) -> usize {
        app.session().unwrap().functions.iter().position(|f| f.name == "funcao_sintetica_20k").unwrap()
    }

    #[test]
    fn open_invalid_path_stays_on_open_screen_with_error() {
        let mut app = App::new();
        app.open("/nao/existe");
        assert_eq!(app.screen, Screen::Open);
        assert!(matches!(app.status, Some((_, Tone::Error))));
    }

    #[test]
    fn open_selects_first_function_and_shows_demo_notice() {
        let app = opened();
        assert_eq!(app.screen, Screen::Workspace);
        assert_eq!(app.current, Some(0));
        assert!(!app.rows.is_empty());
        assert!(app.session().unwrap().engine.is_demo());
    }

    #[test]
    fn select_all_and_copy_return_all_20k_lines() {
        let mut app = opened();
        let i = synthetic_index(&app);
        app.select_function(i, true);
        app.on_key(ctrl('a'));
        assert!(app.select_all);
        assert_eq!(app.line_count(), SYNTHETIC_LINES);
        app.on_key(key(KeyCode::Char('y')));
        let text = app.pending_copy.take().unwrap();
        assert_eq!(text.lines().count(), SYNTHETIC_LINES);
    }

    #[test]
    fn esc_clears_selection() {
        let mut app = opened();
        app.on_key(ctrl('a'));
        app.on_key(key(KeyCode::Esc));
        assert!(!app.select_all);
    }

    #[test]
    fn program_scope_has_headers_and_all_lines() {
        let mut app = opened();
        app.toggle_scope();
        let headers = app.rows.iter().filter(|r| matches!(r, Row::Header(_))).count();
        assert_eq!(headers, app.session().unwrap().functions.len());
        assert!(app.line_count() > SYNTHETIC_LINES);
    }

    #[test]
    fn search_finds_and_steps_through_matches() {
        let mut app = opened();
        app.search = "call".into();
        app.run_search();
        assert!(!app.matches.is_empty());
        let first = app.cursor;
        app.step_match(true);
        assert!(app.matches.len() == 1 || app.cursor != first);
    }

    #[test]
    fn goto_by_address_inside_function_moves_cursor() {
        let mut app = opened();
        assert!(app.goto("0x140001004"));
        assert_eq!(app.session().unwrap().functions[app.current.unwrap()].name, "entry");
        assert_eq!(app.line_at(app.cursor).unwrap().address, "0x140001004");
    }

    #[test]
    fn follow_call_target_and_history() {
        let mut app = opened();
        // Linha 1 de "entry" é "call 0x140001040" (main).
        app.cursor = 1;
        app.follow_target();
        assert_eq!(app.session().unwrap().functions[app.current.unwrap()].name, "main");
        app.back();
        assert_eq!(app.session().unwrap().functions[app.current.unwrap()].name, "entry");
        app.forward();
        assert_eq!(app.session().unwrap().functions[app.current.unwrap()].name, "main");
    }

    #[test]
    fn list_filter_narrows_functions() {
        let mut app = opened();
        app.filter = "hash".into();
        let items = app.list_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].0, "compute_hash");
    }

    #[test]
    fn ensure_visible_scrolls_with_cursor() {
        let mut app = opened();
        let i = synthetic_index(&app);
        app.select_function(i, true);
        app.cursor = 500;
        app.ensure_visible(30);
        assert_eq!(app.scroll, 471);
        app.cursor = 10;
        app.ensure_visible(30);
        assert_eq!(app.scroll, 10);
    }
}
