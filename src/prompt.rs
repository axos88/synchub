//! Yes/no questions on the terminal.

use std::io::{self, BufRead, IsTerminal, Write};

/// Asks `question`; `None` when stdin isn't a terminal, so nobody could answer.
pub fn confirm(question: &str) -> io::Result<Option<bool>> {
    if !io::stdin().is_terminal() {
        return Ok(None);
    }
    print!("{question} [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(Some(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes")))
}
