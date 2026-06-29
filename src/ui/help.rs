//! The keybinding overlay — a calm, grouped cheatsheet in two columns,
//! dismissed by any key.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::commands::{self, HelpSection};
use crate::theme::Theme;
use crate::widgets::popup;

/// Render the help overlay above the current view.
///
/// The bindings live in `commands::HELP_SECTIONS`; here they are split into two
/// balanced columns so the cheatsheet stays short and readable even on smaller
/// terminals.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;

    // Two columns, hand-balanced so neither runs much taller than the other.
    let sections = commands::HELP_SECTIONS;
    let (left, right) = split_columns(sections);

    let left_lines = column_lines(theme, left);
    let right_lines = column_lines(theme, right);

    let rows = left_lines.len().max(right_lines.len()) as u16;
    let height = rows + 4; // borders + vertical padding
    let width = 60;

    let inner = popup::panel_box(frame, area, theme, "keys", width, height);

    let [left_area, right_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(inner);

    frame.render_widget(Paragraph::new(left_lines), left_area);
    frame.render_widget(Paragraph::new(right_lines), right_area);
}

/// Split the sections into two columns, keeping each column's total height
/// roughly even by greedily placing each section on the shorter column.
fn split_columns(sections: &'static [HelpSection]) -> (Vec<&'static HelpSection>, Vec<&'static HelpSection>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let (mut left_h, mut right_h) = (0usize, 0usize);
    for section in sections {
        // +1 for the heading, +1 spacer between sections.
        let cost = section.keys.len() + 2;
        if left_h <= right_h {
            left.push(section);
            left_h += cost;
        } else {
            right.push(section);
            right_h += cost;
        }
    }
    (left, right)
}

/// Render one column: each section as a soft heading followed by its rows,
/// separated by a single blank line.
fn column_lines(theme: &Theme, sections: Vec<&HelpSection>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            section.title.to_string(),
            Style::new()
                .fg(theme.secondary)
                .add_modifier(Modifier::BOLD),
        )));
        for (key, description) in section.keys {
            lines.push(Line::from(vec![
                Span::styled(format!("  {key:<11}"), Style::new().fg(theme.accent)),
                Span::styled((*description).to_string(), Style::new().fg(theme.muted)),
            ]));
        }
    }
    lines
}
