//! The dialog saying an operation failed: like JavaScript's `window.alert`.

use std::ops::ControlFlow;

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::{Dialog, DialogBox, Part, clicked, draw};
use crate::commands::app::components::status_bar::{HotKey, HotKeys};
use crate::commands::app::terminal;
use crate::commands::app::window::Window;

/// Like JavaScript's `window.alert`: the dialog saying `doing` failed - with
/// the `lines` it reported, and the `error` - until its OK button is
/// pressed.
pub fn alert<'a>(
    doing: &'a str,
    lines: &'a [String],
    error: &'a str,
) -> impl Dialog<Output = ()> + 'a {
    Alert {
        doing,
        lines,
        error,
        buttons: Vec::new(),
    }
}

struct Alert<'a> {
    doing: &'a str,
    /// What was reported before it failed.
    lines: &'a [String],
    error: &'a str,
    /// Where its button was drawn.
    buttons: Vec<Rect>,
}

impl Window for Alert<'_> {
    type Output = ();

    fn draw(&mut self, frame: &mut Frame) {
        let dialog = DialogBox {
            title: " failed ".to_string(),
            color: Style::new().red(),
            parts: vec![
                Part::text(self.doing, Style::new().bold()),
                Part::Lines {
                    lines: self.lines,
                    style: Style::new().dim(),
                },
                Part::Gap,
                Part::text(self.error, Style::new().red().bold()),
                Part::Gap,
                Part::Buttons {
                    labels: vec!["[ OK ]".to_string()],
                    highlighted: 0,
                    focus: true,
                },
            ],
        };
        self.buttons = draw(frame, dialog).buttons;
    }

    fn event(&mut self, event: Event) -> Result<ControlFlow<()>> {
        let pressed = match terminal::pressed(&event) {
            Some(key) => {
                matches!(
                    key.code,
                    KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ' | 'o' | 'O')
                ) || terminal::ctrl_c(&key)
            }
            None => clicked(&self.buttons, &event).is_some(),
        };
        Ok(if pressed {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        })
    }
}

impl Dialog for Alert<'_> {
    fn hot_keys(&self) -> HotKeys {
        HotKeys(vec![HotKey::new(&["Enter", "Esc"], "OK")])
    }
}
