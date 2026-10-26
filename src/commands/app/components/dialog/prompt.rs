//! The dialog asking for one line of text: like JavaScript's `window.prompt`.

use std::ops::ControlFlow;

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::{Dialog, DialogBox, Part, clicked, draw};
use crate::commands::app::components::form::{FieldKind, TextField};
use crate::commands::app::components::status_bar::{HotKey, HotKeys};
use crate::commands::app::terminal;
use crate::commands::app::window::Window;

/// Like JavaScript's `window.prompt`: a box titled `doing`, asking for one
/// line of text - the field `label`led,
/// holding `value` to start with - with OK and Cancel buttons; Tab moves
/// between them, Enter in the field is OK, Esc is Cancel. Ends with what's
/// typed, trimmed, or `None` if cancelled.
pub fn prompt<'a>(
    doing: &'a str,
    label: &str,
    value: &str,
) -> impl Dialog<Output = Option<String>> + 'a {
    Prompt {
        doing,
        field: TextField::new().label(label).value(value).required(),
        focus: Focus::Field,
        chosen: Focus::Ok,
        field_area: Rect::default(),
        buttons: Vec::new(),
    }
}

struct Prompt<'a> {
    doing: &'a str,
    field: TextField,
    focus: Focus,
    /// The button Enter presses from the field: the last one that had the
    /// focus, OK at first.
    chosen: Focus,
    /// Where the field was drawn.
    field_area: Rect,
    /// Where OK and Cancel were drawn.
    buttons: Vec<Rect>,
}

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Field,
    Ok,
    Cancel,
}

impl Focus {
    /// The next one Tab goes to; back the other way, with `back`.
    fn next(self, back: bool) -> Self {
        use Focus::*;
        match (self, back) {
            (Field, false) | (Cancel, true) => Ok,
            (Ok, false) | (Field, true) => Cancel,
            (Cancel, false) | (Ok, true) => Field,
        }
    }
}

impl Prompt<'_> {
    const BUTTONS: [&'static str; 2] = ["[ OK ]", "[ Cancel ]"];

    /// A click on OK or Cancel presses it; on the field, focuses it.
    fn mouse(&mut self, event: &Event) -> Result<ControlFlow<Option<String>>> {
        match clicked(&self.buttons, event) {
            Some(0) => return Ok(self.press(Focus::Ok)),
            Some(_) => return Ok(self.press(Focus::Cancel)),
            None => {}
        }
        if let (Some(_), Event::Mouse(mouse)) = (clicked(&[self.field_area], event), event) {
            self.focus = Focus::Field;
            self.field.mouse(mouse, self.field_area)?;
        }
        Ok(ControlFlow::Continue(()))
    }

    /// What Enter does from the field.
    fn chosen_label(&self) -> &'static str {
        match self.chosen {
            Focus::Cancel => "Cancel",
            _ => "OK",
        }
    }

    /// Moves the focus to `focus` - a button becoming the chosen one.
    fn focus_on(&mut self, focus: Focus) {
        self.focus = focus;
        if focus != Focus::Field {
            self.chosen = focus;
        }
    }

    /// What pressing `button` does: OK ends with what's typed - unless
    /// it's nothing yet - Cancel with nothing.
    fn press(&self, button: Focus) -> ControlFlow<Option<String>> {
        match button {
            Focus::Ok if !self.field.missing() => {
                ControlFlow::Break(Some(self.field.text().trim().to_string()))
            }
            Focus::Cancel => ControlFlow::Break(None),
            _ => ControlFlow::Continue(()),
        }
    }
}

impl Window for Prompt<'_> {
    type Output = Option<String>;

    fn draw(&mut self, frame: &mut Frame) {
        let focus = self.focus != Focus::Field;
        let dialog = DialogBox {
            title: format!(" {} ", self.doing),
            color: Style::new().cyan(),
            parts: vec![
                Part::Field {
                    field: &mut self.field,
                    focused: !focus,
                },
                Part::Gap,
                Part::Buttons {
                    labels: Self::BUTTONS.map(String::from).to_vec(),
                    highlighted: usize::from(self.chosen == Focus::Cancel),
                    focus,
                },
            ],
        };
        let drawn = draw(frame, dialog);
        self.field_area = drawn.field.unwrap_or_default();
        self.buttons = drawn.buttons;
    }

    fn event(&mut self, event: Event) -> Result<ControlFlow<Option<String>>> {
        let Some(key) = terminal::pressed(&event) else {
            return self.mouse(&event);
        };
        if key.code == KeyCode::Esc || terminal::ctrl_c(&key) {
            return Ok(ControlFlow::Break(None));
        }
        match (self.focus, key.code) {
            (_, KeyCode::Tab) => self.focus_on(self.focus.next(false)),
            (_, KeyCode::BackTab) => self.focus_on(self.focus.next(true)),
            (Focus::Field, KeyCode::Enter) => return Ok(self.press(self.chosen)),
            (Focus::Field, KeyCode::Down) => self.focus_on(self.chosen),
            (Focus::Field, _) => {
                self.field.key(key)?;
            }
            (button, KeyCode::Enter | KeyCode::Char(' ')) => return Ok(self.press(button)),
            (Focus::Ok, KeyCode::Left | KeyCode::Right) => self.focus_on(Focus::Cancel),
            (Focus::Cancel, KeyCode::Left | KeyCode::Right) => self.focus_on(Focus::Ok),
            (_, KeyCode::Up) => self.focus_on(Focus::Field),
            _ => {}
        }
        Ok(ControlFlow::Continue(()))
    }
}

impl Dialog for Prompt<'_> {
    fn hot_keys(&self) -> HotKeys {
        HotKeys(match self.focus {
            Focus::Field => vec![
                HotKey::new(&["Enter"], self.chosen_label()),
                HotKey::new(&["Esc"], "Cancel"),
                HotKey::new(&["Tab", "↓"], "to the buttons"),
            ],
            _ => vec![
                HotKey::new(&["Enter"], "the highlighted one"),
                HotKey::new(&["←", "→"], "choose"),
                HotKey::new(&["Tab", "↑"], "to the field"),
                HotKey::new(&["Esc"], "Cancel"),
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::KeyEvent;

    use super::*;

    fn press(
        window: &mut impl Window<Output = Option<String>>,
        code: KeyCode,
    ) -> ControlFlow<Option<String>> {
        window.event(Event::Key(KeyEvent::from(code))).unwrap()
    }

    #[test]
    fn a_prompt_ends_with_what_is_typed() {
        let mut prompt = prompt("Rename", "Name", "old");
        "  new".chars().for_each(|c| {
            assert_eq!(
                press(&mut prompt, KeyCode::Char(c)),
                ControlFlow::Continue(())
            )
        });
        assert_eq!(
            press(&mut prompt, KeyCode::Backspace),
            ControlFlow::Continue(())
        );
        assert_eq!(
            press(&mut prompt, KeyCode::Enter),
            ControlFlow::Break(Some("old  ne".to_string()))
        );
    }

    fn prompted(keys: &[KeyCode]) -> ControlFlow<Option<String>> {
        let mut prompt = prompt("Rename", "Name", "old");
        keys.iter()
            .map(|&code| press(&mut prompt, code))
            .find(ControlFlow::is_break)
            .unwrap_or(ControlFlow::Continue(()))
    }

    #[test]
    fn a_prompts_focus_moves_to_its_buttons_and_back() {
        let typed = |text: &str| ControlFlow::Break(Some(text.to_string()));
        assert_eq!(prompted(&[KeyCode::Tab, KeyCode::Enter]), typed("old"));
        assert_eq!(
            prompted(&[KeyCode::Tab, KeyCode::Tab, KeyCode::Enter]),
            ControlFlow::Break(None)
        );
        assert_eq!(
            prompted(&[KeyCode::Down, KeyCode::Right, KeyCode::Char(' ')]),
            ControlFlow::Break(None)
        );
        assert_eq!(
            prompted(&[KeyCode::BackTab, KeyCode::Left, KeyCode::Enter]),
            typed("old")
        );
        // Back in the field, Enter presses the button chosen last.
        assert_eq!(
            prompted(&[KeyCode::Down, KeyCode::Right, KeyCode::Up, KeyCode::Enter]),
            ControlFlow::Break(None)
        );
        // Up goes back to the field, where typing goes in at the cursor.
        assert_eq!(
            prompted(&[
                KeyCode::Down,
                KeyCode::Up,
                KeyCode::Left,
                KeyCode::Char('X'),
                KeyCode::Enter
            ]),
            typed("olXd")
        );
    }

    #[test]
    fn a_prompt_wants_something_typed() {
        let mut prompt = prompt("Rename", "Name", "x");
        assert_eq!(
            press(&mut prompt, KeyCode::Backspace),
            ControlFlow::Continue(())
        );
        assert_eq!(
            press(&mut prompt, KeyCode::Enter),
            ControlFlow::Continue(())
        );
        assert_eq!(press(&mut prompt, KeyCode::Esc), ControlFlow::Break(None));
    }
}
