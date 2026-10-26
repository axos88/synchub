//! What the keys and the mouse do on a form: the keys for the form as a
//! whole; the rest goes to the fields.

use std::ops::ControlFlow;

use anyhow::Result;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Position;

use super::field::EventResponse;
use super::{Answers, Field, Form};
use crate::commands::app::terminal::ctrl_c;

impl Form {
    /// Handles a key; `Break` when the form is done: with its answers when
    /// submitted, `None` when cancelled.
    pub(super) fn key(&mut self, key: KeyEvent) -> Result<ControlFlow<Option<Answers>>> {
        match key.code {
            _ if key.code == KeyCode::Esc || ctrl_c(&key) => return Ok(ControlFlow::Break(None)),
            KeyCode::Enter => match next_missing(&self.fields, self.focus) {
                Some(next) => self.focus = next,
                None => {
                    let fields = std::mem::take(&mut self.fields);
                    return Ok(ControlFlow::Break(Some(Answers(fields))));
                }
            },
            KeyCode::Tab => self.move_focus(1, true),
            KeyCode::BackTab => self.move_focus(-1, true),
            _ => {
                let ignored = matches!(self.fields[self.focus].key(key)?, EventResponse::Ignored);
                // Up and Down past the field's end move the focus.
                match key.code {
                    KeyCode::Up if ignored => self.move_focus(-1, false),
                    KeyCode::Down if ignored => self.move_focus(1, false),
                    _ => {}
                }
            }
        }
        Ok(ControlFlow::Continue(()))
    }

    /// Hands a mouse event to the field it's on; one that takes it gets the focus.
    pub(super) fn mouse(&mut self, mouse: &MouseEvent) -> Result<()> {
        let at = Position::new(mouse.column, mouse.row);
        let Some(i) = self.layout.iter().position(|area| area.contains(at)) else {
            return Ok(());
        };
        if !matches!(
            self.fields[i].mouse(mouse, self.layout[i])?,
            EventResponse::Ignored
        ) {
            self.focus = i;
        }
        Ok(())
    }

    /// Moves the focus `by` fields; past the first or the last, around to
    /// the other end if `wrap`, or else it stays there.
    fn move_focus(&mut self, by: isize, wrap: bool) {
        let count = self.fields.len() as isize;
        let to = self.focus as isize + by;
        self.focus = if wrap {
            to.rem_euclid(count)
        } else {
            to.clamp(0, count - 1)
        } as usize;
    }
}

/// The first empty required field after `focus`, wrapping around.
fn next_missing(fields: &[Field], focus: usize) -> Option<usize> {
    (1..=fields.len())
        .map(|step| (focus + step) % fields.len())
        .find(|&i| fields[i].missing())
}

#[cfg(test)]
mod tests {
    use super::super::{Check, ChecksField, DirField, TextField};
    use super::*;

    fn text(value: &str) -> TextField {
        TextField::new().label("t").value(value)
    }

    #[test]
    fn enter_moves_to_the_next_empty_required_field() {
        let fields: Vec<Field> = vec![
            text("").required().into(),
            text("").into(),
            text("").required().into(),
            text("set").required().into(),
        ];
        assert_eq!(next_missing(&fields, 0), Some(2));
        assert_eq!(next_missing(&fields, 2), Some(0));
        assert_eq!(next_missing(&fields, 3), Some(0));
    }

    #[test]
    fn moving_onto_an_empty_directory_field_doesnt_open_it() {
        let mut form = Form::new("f");
        form.add(text(""));
        form.add(DirField::new().label("d"));
        assert!(
            form.key(KeyEvent::from(KeyCode::Tab))
                .unwrap()
                .is_continue()
        );
        assert_eq!(form.focus, 1);
        // Up isn't the directory field's: it goes back.
        assert!(form.key(KeyEvent::from(KeyCode::Up)).unwrap().is_continue());
        assert_eq!(form.focus, 0);
    }

    #[test]
    fn an_empty_required_directory_keeps_the_form_from_being_submitted() {
        let mut form = Form::new("f");
        form.add(DirField::new().label("d").required());
        form.add(text(""));
        form.focus = 1;
        assert!(
            form.key(KeyEvent::from(KeyCode::Enter))
                .unwrap()
                .is_continue()
        );
        assert_eq!(form.focus, 0);
    }

    #[test]
    fn enter_submits_a_complete_form_its_answers_read_by_handle() {
        let mut form = Form::new("f");
        let first = form.add(text("first"));
        let dir = form.add(DirField::new().label("d").value("~/x"));
        let ControlFlow::Break(Some(answers)) = form.key(KeyEvent::from(KeyCode::Enter)).unwrap()
        else {
            panic!("not submitted");
        };
        assert_eq!(answers[first].text(), "first");
        assert_eq!(answers[dir].path(), "~/x");
    }

    #[test]
    fn a_checks_fields_answer_is_the_keys_ticked() {
        let mut form = Form::new("f");
        let check = |key: i32, ticked: bool| Check {
            key,
            label: key.to_string(),
            ticked,
        };
        let checks = form.add(ChecksField::new().label("c").items(vec![
            check(1, false),
            check(2, false),
            check(3, true),
        ]));
        [KeyCode::Down, KeyCode::Char(' ')]
            .into_iter()
            .for_each(|code| assert!(form.key(KeyEvent::from(code)).unwrap().is_continue()));
        let ControlFlow::Break(Some(answers)) = form.key(KeyEvent::from(KeyCode::Enter)).unwrap()
        else {
            panic!("not submitted");
        };
        assert_eq!(answers[checks].ticked().collect::<Vec<_>>(), [&2, &3]);
    }

    #[test]
    fn esc_cancels() {
        let mut form = Form::new("f");
        form.add(text("typed"));
        assert!(matches!(
            form.key(KeyEvent::from(KeyCode::Esc)).unwrap(),
            ControlFlow::Break(None)
        ));
    }
}
