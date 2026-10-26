//! Gestures - so far a click and a double click - made of the mouse's
//! presses, where they land: the terminal reports only presses. What's at
//! that place is for the component to say: it [`map`](Gesture::map)s the
//! gesture to it.

use std::time::{Duration, Instant};

use ratatui::layout::Position;

/// The longest a double click's second click can come after the first.
const DOUBLE_CLICK: Duration = Duration::from_millis(500);

/// What presses made, at a place on the screen - or, mapped, at what's
/// there, e.g. a list's item.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesture<At = Position> {
    Click(At),
    /// A second click close to the first, quickly.
    DoubleClick(At),
}

impl<At> Gesture<At> {
    /// Where it was made.
    pub fn at(self) -> At {
        match self {
            Gesture::Click(at) | Gesture::DoubleClick(at) => at,
        }
    }

    /// The same gesture, at `f` of where it was made.
    pub fn map<T>(self, f: impl FnOnce(At) -> T) -> Gesture<T> {
        match self {
            Gesture::Click(at) => Gesture::Click(f(at)),
            Gesture::DoubleClick(at) => Gesture::DoubleClick(f(at)),
        }
    }
}

impl<T> Gesture<Option<T>> {
    /// The gesture, if it's at something - e.g. mapped to the item where it
    /// was made, if there's one.
    pub fn transpose(self) -> Option<Gesture<T>> {
        Some(match self {
            Gesture::Click(at) => Gesture::Click(at?),
            Gesture::DoubleClick(at) => Gesture::DoubleClick(at?),
        })
    }
}

/// Makes [`Gesture`]s of presses, as they come.
#[derive(Default)]
pub struct GestureDetector {
    /// Where, and when.
    last_click: Option<(Position, Instant)>,
}

impl GestureDetector {
    /// The next press, at `position`: the gesture it makes with the ones
    /// before it - if it completes one.
    pub fn add(&mut self, position: Position) -> Option<Gesture> {
        self.add_at(position, Instant::now())
    }

    fn add_at(&mut self, position: Position, now: Instant) -> Option<Gesture> {
        let double = self.last_click.is_some_and(|(at, when)| {
            close(at, position) && now.duration_since(when) < DOUBLE_CLICK
        });
        self.last_click = (!double).then_some((position, now));
        Some(if double {
            Gesture::DoubleClick(position)
        } else {
            Gesture::Click(position)
        })
    }
}

/// Whether two presses are close enough to be one place: on the same row, a
/// column apart at most. A cell is narrow but tall - a wobbling hand moves a
/// column, rarely a row - and a row away is often another item.
fn close(a: Position, b: Position) -> bool {
    a.y == b.y && a.x.abs_diff(b.x) <= 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: u16, y: u16) -> Position {
        Position::new(x, y)
    }

    #[test]
    fn a_second_press_close_by_soon_is_a_double_click() {
        let start = Instant::now();
        let soon = start + Duration::from_millis(200);
        let mut gestures = GestureDetector::default();
        assert_eq!(
            gestures.add_at(at(5, 3), start),
            Some(Gesture::Click(at(5, 3)))
        );
        assert_eq!(
            gestures.add_at(at(6, 3), soon),
            Some(Gesture::DoubleClick(at(6, 3)))
        );
        // A third starts over.
        assert_eq!(
            gestures.add_at(at(6, 3), soon),
            Some(Gesture::Click(at(6, 3)))
        );
    }

    #[test]
    fn far_or_late_presses_are_clicks() {
        let start = Instant::now();
        let soon = start + Duration::from_millis(200);
        let late = start + DOUBLE_CLICK;
        let second = |position, when| {
            let mut gestures = GestureDetector::default();
            gestures.add_at(at(5, 3), start);
            gestures.add_at(position, when)
        };
        assert_eq!(second(at(5, 4), soon), Some(Gesture::Click(at(5, 4))));
        assert_eq!(second(at(7, 3), soon), Some(Gesture::Click(at(7, 3))));
        assert_eq!(second(at(5, 3), late), Some(Gesture::Click(at(5, 3))));
    }

    #[test]
    fn a_gesture_keeps_its_kind_when_mapped() {
        assert_eq!(
            Gesture::DoubleClick(at(0, 4)).map(|position| position.y),
            Gesture::DoubleClick(4)
        );
        assert_eq!(
            Gesture::Click(at(3, 4)).map(|position| position.x + 1),
            Gesture::Click(4)
        );
    }

    #[test]
    fn a_gesture_at_nothing_transposes_to_none() {
        assert_eq!(
            Gesture::DoubleClick(Some(2)).transpose(),
            Some(Gesture::DoubleClick(2))
        );
        assert_eq!(Gesture::Click(None::<usize>).transpose(), None);
    }
}
