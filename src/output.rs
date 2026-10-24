//! What the commands report, printed as it happens.

// Each takes, optionally first, where the step runs - `Local` or `Hub`, a
// `side::Place` - which starts the line: `step!(Hub, "sharing {name}")`
// reports "hub: sharing ...". Generic code passes the value: `S::PLACE`.

/// Reports a line of progress.
#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => { $crate::output::say($crate::output::line!($($arg)*), false) };
}

/// Reports a warning, on stderr.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { $crate::output::say($crate::output::line!($($arg)*), true) };
}

/// Reports a step of an operation as it starts, e.g. "local: pausing
/// Notes", on stderr.
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
    if to_stderr {
        eprintln!("{text}");
    } else {
        println!("{text}");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_place_starts_the_line() {
        assert_eq!(output_line!("plain {}", 1), "plain 1");
        assert_eq!(
            output_line!(Hub, "sharing {}", "Notes"),
            "hub: sharing Notes"
        );
        assert_eq!(
            output_line!(crate::side::Place::Local, "pausing"),
            "local: pausing"
        );
    }
}
