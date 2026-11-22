//! Estado e regras da versão de terminal. Sem desenho aqui: só lógica testável.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;

use hellius_core::ai;
use hellius_core::render::{render_function, render_line, render_program};
use hellius_core::{
    open_native, parse_address, CodeKind, CodeLine, Engine, FileSummary, FunctionInfo, ImportInfo,
    StringInfo,
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
    /// Pergunta se pode enviar o código para a Anthropic.
    AiConsent,
    /// Janela com a explicação da IA.
    Ai,
}

pub enum AiMsg {
    Delta(String),
    Done(Result<String, String>),
}

#[derive(Default)]
pub struct AiView {
    pub function: String,
    pub text: String,
    pub running: bool,
    pub error: Option<String>,
    pub scroll: u16,
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
    pub ai: AiView,
    ai_rx: Option<Receiver<AiMsg>>,
    ai_cancel: Arc<AtomicBool>,
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
            ai: AiView::default(),
            ai_rx: None,
            ai_cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    fn say(&mut self, msg: impl Into<String>, tone: Tone) {
        self.status = Some((msg.into(), tone));
    }

    // ---------- Abertura ----------

    /// Valida e analisa o arquivo. Em erro, volta à tela de abertura com a mensagem.
    pub fn open(&mut self, path: &str) {
        match open_native(path, &mut |_, _, _| {}) {
            Ok((summary, engine)) => {
                let engine: Box<dyn Engine> = Box::new(engine);
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
                let s = self.session.as_ref().expect("sessão recém-criada");
                let start = s
                    .functions
                    .iter()
                    .position(|f| f.name == "main")
                    .or_else(|| s.functions.iter().position(|f| f.name == "entry"))
                    .unwrap_or(0);
                self.select_function(start, true);
                self.list_index = start;
                let s = self.session.as_ref().expect("sessão recém-criada");
                let msg = if !s.engine.has_code() {
                    "Arquivo aberto. Sem disassembler para este formato/arquitetura: veja Strings e Seções.".to_string()
                } else if s.engine.is_partial() {
                    format!("{} funções (análise parcial: arquivo grande).", s.functions.len())
                } else {
                    format!("{} funções encontradas. Pseudocódigo é tradução simplificada.", s.functions.len())
                };
                self.say(msg, Tone::Info);
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
                Mode::AiConsent => match key.code {
                    KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('y') | KeyCode::Enter => {
                        let mut cfg = ai::load_config();
                        cfg.consent = true;
                        if let Err(e) = ai::save_config(&cfg) {
                            self.say(format!("Não foi possível salvar o consentimento: {e}"), Tone::Error);
                        }
                        self.start_ai();
                    }
                    _ => self.mode = Mode::Normal,
                },
                Mode::Ai => match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => {
                        if self.ai.running {
                            self.ai_cancel.store(true, Ordering::SeqCst);
                        }
                        self.mode = Mode::Normal;
                    }
                    KeyCode::Up | KeyCode::Char('k') => self.ai.scroll = self.ai.scroll.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => self.ai.scroll = self.ai.scroll.saturating_add(1),
                    KeyCode::PageUp => self.ai.scroll = self.ai.scroll.saturating_sub(10),
                    KeyCode::PageDown => self.ai.scroll = self.ai.scroll.saturating_add(10),
                    KeyCode::Char('y') | KeyCode::Char('c') if !self.ai.text.is_empty() => {
                        self.pending_copy = Some(self.ai.text.clone());
                        self.say("Explicação copiada (OSC 52).", Tone::Success);
                    }
                    KeyCode::Char('r') if !self.ai.running => self.request_ai(),
                    _ => {}
                },
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
            KeyCode::Char('e') => {
                self.request_ai();
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

    // ---------- IA ----------

    /// Tecla `e`: confere chave e consentimento, depois explica a função atual.
    pub fn request_ai(&mut self) {
        if self.current.is_none() || self.program_scope {
            self.say("Abra uma função (escopo Função) para explicar.", Tone::Info);
            return;
        }
        if !self.session.as_ref().is_some_and(|s| s.engine.has_code()) {
            self.say("Sem código para explicar neste arquivo.", Tone::Error);
            return;
        }
        let cfg = ai::load_config();
        if cfg.resolve_key().is_none() {
            self.say(ai::AiError::NoKey.to_string(), Tone::Error);
            return;
        }
        if !cfg.consent {
            self.mode = Mode::AiConsent;
            return;
        }
        self.start_ai();
    }

    fn start_ai(&mut self) {
        let cfg = ai::load_config();
        let Some((key, _)) = cfg.resolve_key() else {
            self.say(ai::AiError::NoKey.to_string(), Tone::Error);
            self.mode = Mode::Normal;
            return;
        };
        let Some(s) = self.session.as_ref() else { return };
        let Some(addr) = self.current.and_then(|f| self.function_address(f)) else { return };
        let input = match ai::input_for_function(&s.summary, s.engine.as_ref(), addr) {
            Ok(i) => i,
            Err(e) => {
                self.say(e.to_string(), Tone::Error);
                self.mode = Mode::Normal;
                return;
            }
        };
        self.ai_cancel.store(true, Ordering::SeqCst); // encerra um pedido anterior
        let cancel = Arc::new(AtomicBool::new(false));
        self.ai_cancel = cancel.clone();
        self.ai = AiView { function: input.function_name.clone(), running: true, ..AiView::default() };
        let (tx, rx) = mpsc::channel();
        self.ai_rx = Some(rx);
        let model = cfg.model();
        std::thread::spawn(move || {
            let res = ai::explain(&key, &model, &input, &cancel, &mut |t| {
                let _ = tx.send(AiMsg::Delta(t.to_string()));
            });
            let _ = tx.send(AiMsg::Done(res.map_err(|e| e.to_string())));
        });
        self.mode = Mode::Ai;
    }

    /// Chamado a cada ciclo do laço principal: puxa o texto que chegou.
    pub fn poll_ai(&mut self) -> bool {
        let Some(rx) = self.ai_rx.as_ref() else { return false };
        let mut changed = false;
        while let Ok(msg) = rx.try_recv() {
            changed = true;
            match msg {
                AiMsg::Delta(t) => self.ai.text.push_str(&t),
                AiMsg::Done(r) => {
                    self.ai.running = false;
                    match r {
                        Ok(full) => self.ai.text = full,
                        Err(e) => self.ai.error = Some(e),
                    }
                }
            }
        }
        if !self.ai.running {
            self.ai_rx = None;
        }
        changed
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

    /// O próprio binário de teste: um ELF/PE/Mach-O real em qualquer sistema.
    fn opened() -> App {
        let mut app = App::new();
        app.open(std::env::current_exe().unwrap().to_str().unwrap());
        assert_eq!(app.screen, Screen::Workspace, "falhou ao abrir: {:?}", app.status);
        app
    }

    /// Índice de uma função grande o bastante para os testes.
    fn big_function(app: &App) -> usize {
        let s = app.session().unwrap();
        (0..s.functions.len()).max_by_key(|&i| s.functions[i].size_bytes).unwrap()
    }

    #[test]
    fn open_invalid_path_stays_on_open_screen_with_error() {
        let mut app = App::new();
        app.open("/nao/existe");
        assert_eq!(app.screen, Screen::Open);
        assert!(matches!(app.status, Some((_, Tone::Error))));
    }

    #[test]
    fn open_real_binary_lists_functions_and_code() {
        let app = opened();
        let s = app.session().unwrap();
        assert!(!s.engine.is_demo());
        assert!(s.functions.len() > 50);
        assert!(!app.rows.is_empty());
        assert!(app.line_at(0).is_some_and(|l| l.bytes.is_some()));
    }

    #[test]
    fn select_all_and_copy_return_every_line_of_the_function() {
        let mut app = opened();
        let i = big_function(&app);
        app.select_function(i, true);
        app.on_key(ctrl('a'));
        assert!(app.select_all);
        let n = app.line_count();
        app.on_key(key(KeyCode::Char('y')));
        let text = app.pending_copy.take().unwrap();
        assert_eq!(text.lines().count(), n);
    }

    #[test]
    fn esc_clears_selection() {
        let mut app = opened();
        app.on_key(ctrl('a'));
        app.on_key(key(KeyCode::Esc));
        assert!(!app.select_all);
    }

    #[test]
    fn program_scope_has_a_header_per_function() {
        let mut app = opened();
        app.toggle_scope();
        let headers = app.rows.iter().filter(|r| matches!(r, Row::Header(_))).count();
        assert_eq!(headers, app.session().unwrap().functions.len());
    }

    #[test]
    fn search_finds_and_steps_through_matches() {
        let mut app = opened();
        let i = big_function(&app);
        app.select_function(i, true);
        app.search = app.line_at(0).unwrap().text.split_whitespace().next().unwrap().to_string();
        app.run_search();
        assert!(!app.matches.is_empty());
        app.step_match(true);
        assert!(app.matches.contains(&app.cursor));
    }

    #[test]
    fn goto_by_address_inside_function_moves_cursor() {
        let mut app = opened();
        let i = big_function(&app);
        app.select_function(i, true);
        let target = app.line_at(3).unwrap().address.clone();
        app.select_function(0, true);
        assert!(app.goto(&target));
        assert_eq!(app.current, Some(i));
        assert_eq!(app.line_at(app.cursor).unwrap().address, target);
    }

    #[test]
    fn follow_call_target_and_history() {
        let mut app = opened();
        // Acha uma linha com chamada para outra função conhecida.
        let s = app.session().unwrap();
        let known: std::collections::HashSet<String> = s.functions.iter().map(|f| f.address.clone()).collect();
        let n = s.functions.len();
        let (from, row, to) = (0..n)
            .find_map(|f| {
                let lines = app.lines_of(f);
                lines.iter().position(|l| l.target.as_ref().is_some_and(|t| known.contains(t))).map(|r| {
                    let t = lines[r].target.clone().unwrap();
                    (f, r, t)
                })
            })
            .expect("nenhuma chamada direta encontrada");
        app.select_function(from, true);
        app.cursor = row;
        app.follow_target();
        let cur = &app.session().unwrap().functions[app.current.unwrap()];
        assert_eq!(cur.address, to);
        app.back();
        assert_eq!(app.current, Some(from));
    }

    #[test]
    fn list_filter_narrows_functions() {
        let mut app = opened();
        let name = app.session().unwrap().functions[big_function(&app)].name.clone();
        app.filter = name.clone();
        let items = app.list_items();
        assert!(!items.is_empty());
        assert!(items.iter().all(|(t, m, _)| t.to_lowercase().contains(&name.to_lowercase()) || m.contains(&name)));
    }

    #[test]
    fn ensure_visible_scrolls_with_cursor() {
        let mut app = opened();
        app.rows = (0..1000).map(|idx| Row::Line { func: 0, idx }).collect();
        app.cursor = 500;
        app.ensure_visible(30);
        assert_eq!(app.scroll, 471);
        app.cursor = 10;
        app.ensure_visible(30);
        assert_eq!(app.scroll, 10);
    }

    #[test]
    fn ai_without_key_explains_how_to_configure() {
        std::env::set_var("HELLIUS_CONFIG_DIR", std::env::temp_dir().join(format!("hellius-tui-noai-{}", std::process::id())));
        std::env::remove_var("ANTHROPIC_API_KEY");
        let mut app = opened();
        app.on_key(key(KeyCode::Char('e')));
        assert_eq!(app.mode, Mode::Normal);
        assert!(matches!(&app.status, Some((m, Tone::Error)) if m.contains("chave")));
    }
}
