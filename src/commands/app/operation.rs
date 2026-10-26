//! Running operations from the app: in the background, with a dialog saying
//! what's being done and what it reports - and bringing up the forms they ask
//! with.

use std::ops::ControlFlow;
use std::sync::mpsc::{self, Receiver};
use std::thread::ScopedJoinHandle;
use std::time::{Duration, Instant};

use anyhow::Result;
use ratatui::Frame;
use ratatui::crossterm::event::Event;
use ratatui::style::Style;

use super::App;
use super::components::dialog::{self, Dialog, DialogBox, Part};
use super::components::status_bar::HotKeys;
use super::pages::main::quits;
use super::state::State;
use super::terminal;
use super::window::Window;
use crate::commands::Command;
use crate::output;

/// How often a dialog showing work going on is drawn again.
const REFRESH: Duration = Duration::from_millis(100);

/// The spinner's frame, `started` that long ago.
fn spinner(started: Instant) -> char {
    const SPINNER: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    SPINNER[(started.elapsed().as_millis() / 100) as usize % SPINNER.len()]
}

impl App {
    /// Logs the command line of `command`, the `synchub` command doing the
    /// same - only shown, never run - then runs `op` and the reload after it
    /// in the background, showing what's being done and what they report
    /// meanwhile - and why they failed, if they did.
    pub fn perform(
        &mut self,
        doing: String,
        command: Option<Command>,
        op: impl FnOnce() -> Result<()> + Send,
    ) -> Result<()> {
        if let Some(command) = &command {
            self.log.add(command.command_line());
        }
        let then_reload = move || {
            let result = op();
            (result, output::sent(), State::load())
        };
        let ((result, reported, state), lines) = self.in_background(&doing, then_reload, false)?;
        let (op_lines, reload_lines) = lines.split_at(reported);
        if let Err(err) = result {
            self.failed(&doing, op_lines, &err)?;
        }
        self.reloaded(&doing, state, reload_lines)
    }

    /// Reloads the state, saying `doing` meanwhile; quitting doesn't wait
    /// for it - nothing is being changed, and the hub may be slow to answer.
    pub fn reload(&mut self, doing: &str) -> Result<()> {
        let (state, lines) = self.in_background(doing, State::load, true)?;
        self.reloaded(doing, state, &lines)
    }

    /// Takes the state reloaded for `doing`; if loading it failed, the state
    /// shown stays, after saying why - the first load failing is the app's
    /// error.
    fn reloaded(&mut self, doing: &str, state: Result<State>, lines: &[String]) -> Result<()> {
        match state {
            Ok(state) => self.loaded(state),
            Err(err) if self.state.is_some() => self.failed(doing, lines, &err)?,
            Err(err) => return Err(err),
        }
        Ok(())
    }

    /// Runs `work` on another thread, with the busy dialog over the page
    /// meanwhile - quittable if `quits`; what it returned, and the lines it
    /// reported.
    fn in_background<T: Send>(
        &self,
        doing: &str,
        work: impl FnOnce() -> T + Send,
        quits: bool,
    ) -> Result<(T, Vec<String>)> {
        let (sink, reported) = mpsc::channel();
        std::thread::scope(|scope| {
            let worker = scope.spawn(move || output::stream(sink, work));
            let busy = Busy {
                doing,
                quits,
                started: Instant::now(),
                worker: &worker,
                reported,
                lines: Vec::new(),
            };
            let lines = busy.popup(self)?;
            let result = worker
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            Ok((result, lines))
        })
    }

    /// Shows why `doing` failed, below what it reported, until it's dismissed.
    fn failed(&self, doing: &str, lines: &[String], err: &anyhow::Error) -> Result<()> {
        let error = format!("{err:#}");
        dialog::alert(doing, lines, &error).popup(self)
    }
}

/// The dialog of work in the background, until it's done; keys pressed
/// meanwhile are dropped - they were meant for the window before. Ends with
/// the lines the work reported.
struct Busy<'a, 'scope, T> {
    doing: &'a str,
    /// Whether q, Esc or Ctrl-C quit the app meanwhile.
    quits: bool,
    started: Instant,
    worker: &'a ScopedJoinHandle<'scope, T>,
    /// Each line as the work reports it.
    reported: Receiver<String>,
    /// The lines reported so far.
    lines: Vec<String>,
}

impl<T> Window for Busy<'_, '_, T> {
    type Output = Vec<String>;

    fn draw(&mut self, frame: &mut Frame) {
        let doing = format!("{} {}", spinner(self.started), self.doing);
        let dialog = DialogBox {
            title: " working ".to_string(),
            color: Style::new().yellow(),
            parts: vec![
                Part::text(doing, Style::new().bold()),
                Part::Lines {
                    lines: &self.lines,
                    style: Style::new().dim(),
                },
            ],
        };
        dialog::draw(frame, dialog);
    }

    fn event(&mut self, event: Event) -> Result<ControlFlow<Vec<String>>> {
        if self.quits && terminal::pressed(&event).is_some_and(|key| quits(&key)) {
            // Not waiting for the work.
            terminal::stop()?;
            std::process::exit(0);
        }
        Ok(ControlFlow::Continue(()))
    }

    fn interval(&self) -> Option<Duration> {
        Some(REFRESH)
    }

    fn finished(&mut self) -> Option<Vec<String>> {
        // Asked first: once it's finished, every line it reported has arrived.
        let finished = self.worker.is_finished();
        self.lines.extend(self.reported.try_iter());
        finished.then(|| std::mem::take(&mut self.lines))
    }
}

impl<T> Dialog for Busy<'_, '_, T> {
    fn hot_keys(&self) -> HotKeys {
        HotKeys::default()
    }
}
