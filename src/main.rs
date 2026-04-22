use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout},
    widgets::{Block, Borders, Paragraph},
};

fn main() -> Result<()> {
    color_eyre::install()?;
    let terminal = ratatui::init();
    let result = run(terminal);
    ratatui::restore();
    result
}

fn run(mut terminal: DefaultTerminal) -> Result<()> {
    loop {
        terminal.draw(render)?;
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
        {
            return Ok(());
        }
    }
}

fn render(frame: &mut Frame) {
    let [sidebar, thread] =
        Layout::horizontal([Constraint::Length(32), Constraint::Min(0)]).areas(frame.area());

    frame.render_widget(
        Paragraph::new("(not yet loaded)")
            .block(Block::default().borders(Borders::ALL).title("courier")),
        sidebar,
    );
    frame.render_widget(
        Paragraph::new("Select a conversation\n\nPress q or Esc to quit")
            .block(Block::default().borders(Borders::ALL).title("Messages")),
        thread,
    );
}
