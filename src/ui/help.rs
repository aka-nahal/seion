//! The keybinding overlay — a centered cheatsheet, dismissed by any key.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::commands;
use crate::widgets::popup;

/// Render the help overlay above the current view.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;

    let lines: Vec<Line> = commands::HELP
        .iter()
        .map(|(key, description)| {
            Line::from(vec![
                Span::styled(format!("{key:<10}"), Style::new().fg(theme.accent)),
                Span::styled((*description).to_string(), Style::new().fg(theme.muted)),
            ])
        })
        .collect();

    let height = lines.len() as u16 + 4; // borders + vertical padding
    popup::overlay(frame, area, theme, "keys", lines, 46, height);
}
