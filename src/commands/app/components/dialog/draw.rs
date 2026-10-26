//! Laying a dialog box out and drawing it.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};

use super::{DialogBox, Drawn, Part};
use crate::commands::app::components::form::FieldKind;

/// Between buttons.
const GAP: &str = "   ";

/// Draws `dialog` in the middle of the screen; the last of its lines that
/// fit, if they don't all.
pub fn draw(frame: &mut Frame, dialog: DialogBox) -> Drawn {
    let area = frame.area();
    let room = usize::from(area.width.saturating_sub(6)).max(1);
    let pieces: Vec<Piece> = dialog
        .parts
        .into_iter()
        .map(|part| Piece::of(part, room))
        .collect();
    // What isn't lines takes its room first.
    let fixed: usize = pieces
        .iter()
        .filter(|piece| !matches!(piece, Piece::Lines(_, true)))
        .map(Piece::height)
        .sum();
    let budget = usize::from(area.height.saturating_sub(4)).saturating_sub(fixed);
    let pieces: Vec<Piece> = pieces
        .into_iter()
        .scan(budget, |left, piece| Some(piece.fitted(left)))
        .collect();
    let widest = pieces
        .iter()
        .map(Piece::width)
        .chain([dialog.title.chars().count()])
        .max()
        .unwrap_or_default();
    let heights: Vec<Constraint> = pieces
        .iter()
        .map(|piece| Constraint::Length(piece.height() as u16))
        .collect();
    let height = pieces.iter().map(Piece::height).sum::<usize>() as u16;
    let inner = boxed(
        frame,
        &dialog.title,
        dialog.color,
        widest.min(room) as u16,
        height,
        area.width,
    );
    let areas = Layout::vertical(heights).split(inner);
    pieces
        .into_iter()
        .zip(areas.iter())
        .fold(Drawn::default(), |mut drawn, (piece, &area)| {
            match piece {
                Piece::Lines(lines, _) => {
                    frame.render_widget(Paragraph::new(lines).style(Style::reset()), area)
                }
                Piece::Field(field, focused) => {
                    field.show(frame, area, focused);
                    drawn.field = Some(area);
                }
                Piece::Buttons(labels, highlighted, focus) => {
                    let line = buttons_line(&labels, highlighted, focus);
                    frame.render_widget(Paragraph::new(line).style(Style::reset()), area);
                    drawn.buttons = button_areas(&labels, area);
                }
            }
            drawn
        })
}

/// A part, laid out: lines - the last of them dropped if need be, when
/// `true` - a field, or buttons.
enum Piece<'a> {
    Lines(Vec<Line<'static>>, bool),
    Field(&'a mut dyn FieldKind, bool),
    Buttons(Vec<String>, usize, bool),
}

impl<'a> Piece<'a> {
    fn of(part: Part<'a>, room: usize) -> Self {
        match part {
            Part::Text { text, style, prose } => {
                // Prose reads better in shorter lines.
                let width = if prose { room.min(60) } else { room };
                Piece::Lines(wrapped(&text, style, width), false)
            }
            Part::Lines { lines, style } => Piece::Lines(
                lines
                    .iter()
                    .flat_map(|line| wrapped(line, style, room))
                    .collect(),
                true,
            ),
            Part::Gap => Piece::Lines(vec![Line::default()], false),
            Part::Field { field, focused } => Piece::Field(field, focused),
            Part::Buttons {
                labels,
                highlighted,
                focus,
            } => Piece::Buttons(labels, highlighted, focus),
        }
    }

    /// Its lines that fit in what's `left` of the room - the last of them.
    fn fitted(self, left: &mut usize) -> Self {
        let Piece::Lines(lines, true) = self else {
            return self;
        };
        let shown = lines.len().min(*left);
        *left -= shown;
        Piece::Lines(lines[lines.len() - shown..].to_vec(), true)
    }

    fn height(&self) -> usize {
        match self {
            Piece::Lines(lines, _) => lines.len(),
            Piece::Field(field, _) => usize::from(field.height()),
            Piece::Buttons(..) => 1,
        }
    }

    fn width(&self) -> usize {
        match self {
            Piece::Lines(lines, _) => lines.iter().map(Line::width).max().unwrap_or_default(),
            Piece::Field(field, _) => field.width(),
            Piece::Buttons(labels, ..) => buttons_width(labels),
        }
    }
}

/// How wide `buttons` are on their line.
fn buttons_width(buttons: &[String]) -> usize {
    buttons.iter().map(String::len).sum::<usize>() + GAP.len() * buttons.len().saturating_sub(1)
}

/// The buttons' line, right-aligned, the button `highlighted` on yellow if
/// it has the `focus`, else on cyan: what Enter does from elsewhere.
fn buttons_line(buttons: &[String], highlighted: usize, focus: bool) -> Line<'static> {
    let style = if focus {
        Style::new().black().on_yellow()
    } else {
        Style::new().black().on_cyan()
    };
    let spans = buttons.iter().enumerate().flat_map(|(i, button)| {
        let gap = (i > 0).then(|| Span::raw(GAP));
        let button = if i == highlighted {
            Span::styled(button.clone(), style)
        } else {
            Span::raw(button.clone())
        };
        gap.into_iter().chain([button])
    });
    Line::from(spans.collect::<Vec<_>>()).right_aligned()
}

/// Where `buttons` are, drawn as [`buttons_line`] on `line`.
fn button_areas(buttons: &[String], line: Rect) -> Vec<Rect> {
    let start = line.x + line.width.saturating_sub(buttons_width(buttons) as u16);
    buttons
        .iter()
        .scan(start, |x, button| {
            let area = Rect::new(*x, line.y, button.len() as u16, 1);
            *x += (button.len() + GAP.len()) as u16;
            Some(area)
        })
        .collect()
}

/// Draws the dialog's box, `title`d, in the middle of the screen - room for
/// `width` (at most `room`) by `height` inside it; returns that room.
fn boxed(frame: &mut Frame, title: &str, color: Style, width: u16, height: u16, room: u16) -> Rect {
    let area = frame.area();
    let width = (width.min(room) + 6).min(area.width);
    let height = (height + 4).min(area.height);
    let dialog = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, dialog);
    frame.render_widget(Block::bordered().title(title).style(color), dialog);
    dialog.inner(Margin::new(3, 2))
}

/// `text` in lines of `room` characters, broken between words - and a word
/// longer than that, e.g. a path, anywhere.
fn wrapped(text: &str, style: Style, room: usize) -> Vec<Line<'static>> {
    let lines = text
        .split(' ')
        .fold(vec![String::new()], |mut lines, word| {
            let last = lines.last().map_or(0, |line| line.chars().count());
            if last > 0 && last + 1 + word.chars().count() > room {
                lines.push(String::new());
            }
            let line = lines.last_mut().expect("one line at least");
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
            lines
        });
    lines
        .iter()
        .flat_map(|line| {
            let chars: Vec<char> = line.chars().collect();
            if chars.is_empty() {
                vec![String::new()]
            } else {
                chars
                    .chunks(room)
                    .map(|chunk| chunk.iter().collect())
                    .collect()
            }
        })
        .map(|line| Line::styled(line, style))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_wraps_between_words_and_a_long_word_anywhere() {
        let text = |lines: Vec<Line>| -> Vec<String> {
            lines.iter().map(|line| line.to_string()).collect()
        };
        assert_eq!(
            text(wrapped("Remove it from the hub?", Style::new(), 10)),
            ["Remove it", "from the", "hub?"]
        );
        assert_eq!(
            text(wrapped("in /a/long/path", Style::new(), 6)),
            ["in", "/a/lon", "g/path"]
        );
        assert_eq!(text(wrapped("", Style::new(), 6)), [""]);
    }
}
