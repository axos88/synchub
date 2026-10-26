//! What the commands report. Printed - or, while the app owns the screen,
//! sent to the dialog showing the operation.

use std::sync::Mutex;
use std::sync::mpsc::Sender;

/// Where the lines go instead of being printed, while [`stream`] runs.
static SINK: Mutex<Option<Sink>> = Mutex::new(None);

struct Sink {
    lines: Sender<String>,
    /// How many it sent.
    sent: usize,
}

impl Sink {
    fn send(&mut self, line: &str) {
        // Its receiver gone, nobody sees it; it was reported all the same.
        let _ = self.lines.send(line.into());
        self.sent += 1;
    }
}

// Each takes, optionally first, where the step runs - `Local` or `Hub`, a
// `side::Place` - which starts the line: `step!(Hub, "sharing {name}")`
// reports "hub: sharing ...". Generic code passes the value: `S::PLACE`.

/// Reports a line of progress: printed, or sent on while [`stream`] runs.
#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => { $crate::output::say($crate::output::line!($($arg)*), false) };
}

/// Reports a warning: printed to stderr, or sent on while [`stream`] runs.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { $crate::output::say($crate::output::line!($($arg)*), true) };
}

/// Reports a step of an operation as it starts, e.g. "local: pausing
/// Notes": printed to stderr, or sent on while [`stream`] runs - the app
/// shows them as they come.
#[macro_export]
macro_rules! step {
    ($($arg:tt)*) => { $crate::output::say($crate::output::line!($($arg)*), true) };
}

/// The reported line: the format arguments, after the place if one is given.
#[doc(hidden)]
#[macro_export]
macro_rules! output_line {
    ($fmt:literal $($arg:tt)*) => { format!($fmt $($arg)*) };
    ($place:ident, $($arg:tt)+) => {
        format!("{}: {}", $crate::side::Place::$place, format!($($arg)+))
    };
    ($place:expr, $($arg:tt)+) => { format!("{}: {}", $place, format!($($arg)+)) };
}

#[doc(hidden)]
pub use output_line as line;

pub fn say(text: String, to_stderr: bool) {
    match SINK.lock().unwrap().as_mut() {
        Some(sink) => text.lines().for_each(|line| sink.send(line)),
        None if to_stderr => eprintln!("{text}"),
        None => println!("{text}"),
    }
}

/// Runs `f`, sending each line it reports to `sink` instead of printing it -
/// for another thread to show as they come.
pub fn stream<T>(sink: Sender<String>, f: impl FnOnce() -> T) -> T {
    *SINK.lock().unwrap() = Some(Sink {
        lines: sink,
        sent: 0,
    });
    let result = f();
    *SINK.lock().unwrap() = None;
    result
}

/// How many lines the running [`stream`] has sent so far - where what's
/// reported next starts.
pub fn sent() -> usize {
    SINK.lock().unwrap().as_ref().map_or(0, |sink| sink.sent)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    #[test]
    fn sends_each_line_while_streaming() {
        let (sink, lines) = mpsc::channel();
        stream(sink, || {
            say!("one");
            warn!("two\nthree");
            assert_eq!(
                lines.try_iter().collect::<Vec<_>>(),
                ["one", "two", "three"]
            );
            assert_eq!(sent(), 3);
            step!("four");
        });
        assert_eq!(lines.try_iter().collect::<Vec<_>>(), ["four"]);
    }
}
