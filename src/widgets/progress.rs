//! The thin one-line progress bar for the player: `02:17 ━━━━━──────── 05:40`.
//!
//! Drawn by hand (rather than with `LineGauge`) so the filled and unfilled
//! halves can use different glyph weights for a soft, readable line.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;
use crate::ui;
use crate::utils;

/// Render the progress line into a single-row `area`. When `remaining` is set
/// the right-hand label counts down (`-mm:ss`) instead of showing the total.
pub fn render(frame: &mut Frame, area: Rect, theme: &Theme, position: f64, duration: f64, remaining: bool) {
    if area.width == 0 {
        return;
    }

    let position_label = utils::format_position(position);
    let total_label = if duration > 0.0 {
        if remaining {
            let left = (duration - position).max(0.0);
            format!("-{}", utils::format_duration(left as u64))
        } else {
            utils::format_duration(duration as u64)
        }
    } else {
        "--:--".to_string()
    };
    let ratio = if duration > 0.0 {
        (position / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Reserve room for "pos " and " total"; the rest is the bar.
    let labels_width = position_label.chars().count() + total_label.chars().count() + 2;
    let bar_width = (area.width as usize).saturating_sub(labels_width);
    let filled = (ratio * bar_width as f64).round() as usize;
    let filled = filled.min(bar_width);

    let line = Line::from(vec![
        Span::styled(format!("{position_label} "), Style::new().fg(theme.muted)),
        Span::styled("━".repeat(filled), Style::new().fg(theme.accent)),
        Span::styled(
            "─".repeat(bar_width - filled),
            Style::new().fg(ui::border_color(theme)),
        ),
        Span::styled(format!(" {total_label}"), Style::new().fg(theme.muted)),
    ]);

    frame.render_widget(Paragraph::new(line), area);
}
