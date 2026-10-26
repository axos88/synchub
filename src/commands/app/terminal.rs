//! The terminal: switching to the full screen and back, and reading keys.

use std::io::{self, IsTerminal};

use anyhow::Result;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use ratatui::crossterm::execute;

/// Whether a person is at the terminal to fill in a form.
pub fn interactive() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Switches to the full-screen terminal, with mouse clicks and scrolling reported.
pub fn start() -> Result<DefaultTerminal> {
    let terminal = ratatui::try_init()?;
    execute!(io::stdout(), EnableMouseCapture)?;
    Ok(terminal)
}

pub fn stop() -> Result<()> {
    execute!(io::stdout(), DisableMouseCapture)?;
    ratatui::try_restore()?;
    Ok(())
}

/// The key `event` pressed, if it's a key press.
pub fn pressed(event: &Event) -> Option<KeyEvent> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => Some(*key),
        _ => None,
    }
}

pub fn ctrl_c(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c')
}
