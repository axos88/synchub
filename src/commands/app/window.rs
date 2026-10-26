//! A window: what has the terminal - the main page, a form, the directory
//! browser, the main page with a dialog over it - and the one loop that runs
//! it: draw, wait for an event, hand it on. A window bringing up another runs
//! that one in its event handler, and gets its result back.

use std::cell::RefCell;
use std::ops::ControlFlow;
use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{self, Event};
use ratatui::{DefaultTerminal, Frame};

use super::terminal;

pub trait Window {
    /// What it ends with.
    type Output;

    fn draw(&mut self, frame: &mut Frame);

    /// An event while it's shown; `Break` with the result when that ends it.
    fn event(&mut self, event: Event) -> Result<ControlFlow<Self::Output>>;

    /// For a window showing work in the background: how long to wait for an
    /// event before drawing it again. `None` waits for one.
    fn interval(&self) -> Option<Duration> {
        None
    }

    /// The result, once it's done without an event - the work it shows
    /// finished; asked before every draw.
    fn finished(&mut self) -> Option<Self::Output> {
        None
    }

    /// When it's shown, before it's drawn.
    fn opened(&mut self) -> Result<()> {
        Ok(())
    }

    /// The same window, ending with `f` of what it ends with.
    fn map<U, F>(self, f: F) -> Map<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Output) -> U,
    {
        Map { window: self, f }
    }

    /// For a window ending with an `Option`, e.g. `None` if cancelled: the
    /// same window, ending with `f` of what's in it.
    fn map_some<T, U, F>(self, mut f: F) -> impl Window<Output = Option<U>>
    where
        Self: Window<Output = Option<T>> + Sized,
        F: FnMut(T) -> U,
    {
        self.map(move |output| output.map(&mut f))
    }

    /// For a window ending with a `Result`: the same window, ending with `f`
    /// of its value.
    #[allow(dead_code, reason = "no window ends with a Result yet")]
    fn map_ok<T, E, U, F>(self, mut f: F) -> impl Window<Output = Result<U, E>>
    where
        Self: Window<Output = Result<T, E>> + Sized,
        F: FnMut(T) -> U,
    {
        self.map(move |output| output.map(&mut f))
    }
}

/// A window with its result mapped: [`Window::map`].
pub struct Map<W, F> {
    window: W,
    f: F,
}

impl<W: Window, U, F: FnMut(W::Output) -> U> Window for Map<W, F> {
    type Output = U;

    fn draw(&mut self, frame: &mut Frame) {
        self.window.draw(frame);
    }

    fn event(&mut self, event: Event) -> Result<ControlFlow<U>> {
        Ok(self.window.event(event)?.map_break(&mut self.f))
    }

    fn interval(&self) -> Option<Duration> {
        self.window.interval()
    }

    fn finished(&mut self) -> Option<U> {
        self.window.finished().map(&mut self.f)
    }

    fn opened(&mut self) -> Result<()> {
        self.window.opened()
    }
}

thread_local! {
    /// The full screen, while windows are shown: one for all of them, so a
    /// window shown over another is drawn knowing what's on the screen.
    static TERMINAL: RefCell<Option<DefaultTerminal>> = const { RefCell::new(None) };
}

/// Showing a window: every [`Window`] can, the same way.
pub trait RunnableWindow: Window {
    /// Shows it until it's done, on the full screen (see [`full_screen`]).
    fn run(self) -> Result<Self::Output>;
}

impl<W: Window> RunnableWindow for W {
    fn run(mut self) -> Result<W::Output> {
        full_screen(|| {
            self.opened()?;
            loop {
                if let Some(output) = self.finished() {
                    return Ok(output);
                }
                draw(&mut self)?;
                let ready = match self.interval() {
                    Some(wait) => event::poll(wait)?,
                    None => true,
                };
                if ready && let ControlFlow::Break(output) = self.event(event::read()?)? {
                    return Ok(output);
                }
            }
        })
    }
}

/// Runs `f` on the full screen: switches to it, and back after, unless it's
/// on already - windows shown one after another stay on it this way.
pub fn full_screen<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    let switched = TERMINAL.with_borrow_mut(|screen| -> Result<bool> {
        if screen.is_some() {
            return Ok(false);
        }
        *screen = Some(terminal::start()?);
        Ok(true)
    })?;
    let result = f();
    if switched {
        TERMINAL.set(None);
        terminal::stop()?;
    }
    result
}

fn draw(window: &mut impl Window) -> Result<()> {
    TERMINAL.with_borrow_mut(|screen| {
        let screen = screen
            .as_mut()
            .expect("running a window switches to the full screen");
        screen.draw(|frame| window.draw(frame))?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent};

    use super::*;

    /// Ends with the number of events it got, at the second; or, `done`,
    /// without any.
    struct Counter {
        events: u32,
        done: bool,
    }

    impl Window for Counter {
        type Output = u32;

        fn draw(&mut self, _: &mut Frame) {}

        fn event(&mut self, _: Event) -> Result<ControlFlow<u32>> {
            self.events += 1;
            Ok(match self.events {
                2 => ControlFlow::Break(self.events),
                _ => ControlFlow::Continue(()),
            })
        }

        fn finished(&mut self) -> Option<u32> {
            self.done.then_some(0)
        }
    }

    fn key() -> Event {
        Event::Key(KeyEvent::from(KeyCode::Enter))
    }

    #[test]
    fn map_changes_what_a_window_ends_with() {
        let mut window = Counter {
            events: 0,
            done: false,
        }
        .map(|n| format!("{n} events"));
        assert_eq!(window.event(key()).unwrap(), ControlFlow::Continue(()));
        assert_eq!(
            window.event(key()).unwrap(),
            ControlFlow::Break("2 events".to_string())
        );
    }

    #[test]
    fn map_some_and_map_ok_change_whats_inside() {
        let mut some = Counter {
            events: 0,
            done: true,
        }
        .map(Some)
        .map_some(|n| n + 1);
        assert_eq!(some.finished(), Some(Some(1)));
        let mut none = Counter {
            events: 0,
            done: true,
        }
        .map(|_| None::<u32>)
        .map_some(|n| n + 1);
        assert_eq!(none.finished(), Some(None));
        let mut ok = Counter {
            events: 0,
            done: true,
        }
        .map(Ok::<u32, String>)
        .map_ok(|n| n + 1);
        assert_eq!(ok.finished(), Some(Ok(1)));
    }

    #[test]
    fn map_changes_what_a_window_finishes_with() {
        let mut window = Counter {
            events: 0,
            done: true,
        }
        .map(|n| n + 1);
        assert_eq!(window.finished(), Some(1));
    }
}
