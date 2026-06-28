//! The slim header: the name, the current view, and a thin separator.
//!
//! The 静音 logo is centered on its own line so it sits at a fixed position on
//! every screen — a wide (CJK) glyph that shifts column between frames can leave
//! a stale cell behind, so we keep it still and let only the ascii subtitle vary.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::ui;
use crate::utils;

/// Render the header into a 3-row area.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let [title_area, sep_area] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

    let logo = Line::from(Span::styled(
        "静音",
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
    ))
    .centered();

    let subtitle_text = if app.view.title().is_empty() {
        "peaceful music terminal".to_string()
    } else {
        format!("{} · peaceful music terminal", app.view.title())
    };
    let subtitle = Line::from(Span::styled(
        subtitle_text,
        Style::new()
            .fg(utils::lerp_color(theme.background, theme.muted, 0.7))
            .add_modifier(Modifier::ITALIC),
    ))
    .centered();

    frame.render_widget(Paragraph::new(vec![logo, subtitle]).centered(), title_area);

    let separator = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            separator,
            Style::new().fg(ui::border_color(theme)),
        ))),
        sep_area,
    );
}
