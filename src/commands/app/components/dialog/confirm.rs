//! The dialog asking a yes/no question: like JavaScript's `window.confirm`.

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

/// Like JavaScript's `window.confirm`: the dialog asking `question` about
/// `doing`, with `lines` above it - until it's answered with its buttons, labelled with what they do: `yes` (true) or
/// `no` (false) - `no` has the focus - or y, n, or Esc for `no`.
pub fn confirm<'a>(
    doing: &'a str,
    lines: &'a [String],
    question: &'a str,
    yes: &'a str,
    no: &'a str,
) -> impl Dialog<Output = bool> + 'a {
    Confirm {
        doing,
        lines,
        question,
        yes,
        no,
        // No, unless it's chosen.
        focus: 1,
        buttons: Vec::new(),
    }
}

struct Confirm<'a> {
    doing: &'a str,
    lines: &'a [String],
    question: &'a str,
    yes: &'a str,
    no: &'a str,
    /// The button with the focus: 0 Yes, 1 No.
    focus: usize,
    /// Where they were drawn.
    buttons: Vec<Rect>,
}

impl Window for Confirm<'_> {
    type Output = bool;

    fn draw(&mut self, frame: &mut Frame) {
        let lines = (!self.lines.is_empty()).then_some([
            Part::Lines {
                lines: self.lines,
                style: Style::new(),
            },
            Part::Gap,
        ]);
        let question = Part::Text {
            text: self.question.to_string(),
            style: Style::new().bold(),
            prose: true,
        };
        let buttons = Part::Buttons {
            labels: vec![format!("[ {} ]", self.yes), format!("[ {} ]", self.no)],
            highlighted: self.focus,
            focus: true,
        };
        let dialog = DialogBox {
            // Its title says what it's about.
            title: format!(" {} ", self.doing),
            color: Style::new().cyan(),
            parts: lines
                .into_iter()
                .flatten()
                .chain([question, Part::Gap, buttons])
                .collect(),
        };
        self.buttons = draw(frame, dialog).buttons;
    }

    fn event(&mut self, event: Event) -> Result<ControlFlow<bool>> {
        let Some(key) = terminal::pressed(&event) else {
            return Ok(match clicked(&self.buttons, &event) {
                Some(button) => ControlFlow::Break(button == 0),
                None => ControlFlow::Continue(()),
            });
        };
        Ok(match key.code {
            KeyCode::Char('y' | 'Y') => ControlFlow::Break(true),
            KeyCode::Char('n' | 'N') | KeyCode::Esc => ControlFlow::Break(false),
            _ if terminal::ctrl_c(&key) => ControlFlow::Break(false),
            KeyCode::Enter | KeyCode::Char(' ') => ControlFlow::Break(self.focus == 0),
            KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                self.focus = 1 - self.focus;
                ControlFlow::Continue(())
            }
            _ => ControlFlow::Continue(()),
        })
    }
}

impl Dialog for Confirm<'_> {
    fn hot_keys(&self) -> HotKeys {
        HotKeys(vec![
            HotKey::new(&["y"], self.yes),
            HotKey::new(&["n", "Esc"], self.no),
            HotKey::new(&["←", "→"], "choose"),
            HotKey::new(&["Enter"], "the highlighted one"),
        ])
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyEvent;

    use super::*;

    fn answer(keys: &[KeyCode]) -> ControlFlow<bool> {
        let mut confirm = confirm("Removing it", &[], "Remove it?", "Remove it", "Cancel");
        keys.iter()
            .map(|&code| confirm.event(Event::Key(KeyEvent::from(code))).unwrap())
            .find(ControlFlow::is_break)
            .unwrap_or(ControlFlow::Continue(()))
    }

    #[test]
    fn a_confirm_says_no_unless_yes_is_chosen() {
        assert_eq!(answer(&[KeyCode::Enter]), ControlFlow::Break(false));
        assert_eq!(
            answer(&[KeyCode::Left, KeyCode::Enter]),
            ControlFlow::Break(true)
        );
        assert_eq!(
            answer(&[KeyCode::Tab, KeyCode::Tab, KeyCode::Char(' ')]),
            ControlFlow::Break(false)
        );
        assert_eq!(answer(&[KeyCode::Char('y')]), ControlFlow::Break(true));
        assert_eq!(
            answer(&[KeyCode::Left, KeyCode::Esc]),
            ControlFlow::Break(false)
        );
        assert_eq!(answer(&[KeyCode::Char('x')]), ControlFlow::Continue(()));
    }
}
