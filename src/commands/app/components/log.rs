//! The log: the command lines doing what the latest actions did.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph};

use crate::commands::app::state::Log;

/// Its height for `log`, border included.
pub fn height(log: &Log) -> u16 {
    log.latest().len().max(1) as u16 + 2
}

pub fn draw(frame: &mut Frame, area: Rect, log: &Log) {
    let lines: Vec<Line> = log
        .latest()
        .iter()
        .map(|command| Line::from(format!("$ {command}").cyan().bold()))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Log ").dim()),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_grows_with_the_commands_up_to_the_ones_shown() {
        let mut log = Log::default();
        assert_eq!(height(&log), 3);
        (0..5).for_each(|i| log.add(format!("synchub get {i}")));
        assert_eq!(height(&log), 5);
        assert_eq!(
            log.latest().first().map(String::as_str),
            Some("synchub get 2")
        );
        log.max_lines = 1;
        assert_eq!(height(&log), 3);
    }
}
