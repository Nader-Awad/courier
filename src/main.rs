mod chatdb;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
};

use chatdb::{ConversationSummary, default_db, load_conversations};

struct App {
    chats: Vec<ConversationSummary>,
    list_state: ListState,
    error: Option<String>,
}

impl App {
    fn new() -> Self {
        let (chats, error) = match load_conversations(&default_db()) {
            Ok(c) => (c, None),
            Err(e) => (Vec::new(), Some(e.to_string())),
        };
        let mut list_state = ListState::default();
        if !chats.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            chats,
            list_state,
            error,
        }
    }

    fn next(&mut self) {
        if self.chats.is_empty() {
            return;
        }
        let i = self
            .list_state
            .selected()
            .map_or(0, |i| (i + 1) % self.chats.len());
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.chats.is_empty() {
            return;
        }
        let i = self.list_state.selected().map_or(0, |i| {
            if i == 0 {
                self.chats.len() - 1
            } else {
                i - 1
            }
        });
        self.list_state.select(Some(i));
    }

    fn selected(&self) -> Option<&ConversationSummary> {
        self.list_state.selected().and_then(|i| self.chats.get(i))
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    let mut app = App::new();
    loop {
        terminal.draw(|f| render(f, &mut app))?;
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('j') | KeyCode::Down => app.next(),
                KeyCode::Char('k') | KeyCode::Up => app.previous(),
                _ => {}
            }
        }
    }
}

fn render(frame: &mut Frame, app: &mut App) {
    let [sidebar, thread] =
        Layout::horizontal([Constraint::Length(32), Constraint::Min(0)]).areas(frame.area());

    render_sidebar(frame, app, sidebar);
    render_thread(frame, app, thread);
}

fn render_sidebar(frame: &mut Frame, app: &mut App, area: Rect) {
    if let Some(err) = &app.error {
        let p = Paragraph::new(err.as_str())
            .wrap(Wrap { trim: false })
            .block(Block::default().borders(Borders::ALL).title("courier"));
        frame.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .chats
        .iter()
        .map(|c| ListItem::new(Line::from(c.name.clone())))
        .collect();

    let title = format!("courier · {} chats", app.chats.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▸ ");

    frame.render_stateful_widget(list, area, &mut app.list_state);
}

fn render_thread(frame: &mut Frame, app: &App, area: Rect) {
    let body = match app.selected() {
        Some(c) => format!(
            "{name}\n\n\
             identifier: {ident}\n\
             service:    {svc}\n\
             rowid:      {rowid}\n\n\
             (messages load in milestone 2)\n\n\
             q/Esc to quit   j/k or ↑/↓ to navigate",
            name = c.name,
            ident = c.identifier,
            svc = c.service,
            rowid = c.rowid,
        ),
        None => String::from("No conversation selected.\n\nq/Esc to quit"),
    };
    let p = Paragraph::new(body)
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::ALL).title("Messages"));
    frame.render_widget(p, area);
}
