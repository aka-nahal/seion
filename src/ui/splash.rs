//! The opening screen: a single calm breath before the interface fades in.

use ratatui::Frame;
use ratatui::layout::{Rect, Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::ui;
use crate::utils;

/// Render the splash, fading in over the first second or so.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    // 0.0 → 1.0 over ~8 ticks (about a second).
    let t = (app.splash_frame as f32 / 8.0).min(1.0);
    let logo = utils::lerp_color(theme.background, theme.accent, t);
    let text = utils::lerp_color(theme.background, theme.muted, t);

    let lines = vec![
        Line::from(Span::styled(
            "静音",
            Style::new().fg(logo).add_modifier(Modifier::BOLD),
        ))
        .centered(),
        Line::from("").centered(),
        Line::from(Span::styled("breathe.", Style::new().fg(text))).centered(),
        Line::from(Span::styled(
            "press enter",
            Style::new().fg(text).add_modifier(Modifier::DIM),
        ))
        .centered(),
    ];

    let center = ui::centered(area, 40, lines.len() as u16);
    frame.render_widget(Paragraph::new(lines).centered(), center);

    // Once settled, let today's quiet line surface near the bottom.
    if app.config.daily_quote && t >= 1.0 && area.height > 6 {
        let (jp, en) = app.quote;
        let quote = vec![
            Line::from(Span::styled(
                jp,
                Style::new().fg(utils::lerp_color(theme.background, theme.muted, 0.6)),
            ))
            .centered(),
            Line::from(Span::styled(
                en,
                Style::new()
                    .fg(utils::lerp_color(theme.background, theme.muted, 0.45))
                    .add_modifier(Modifier::ITALIC),
            ))
            .centered(),
        ];
        let [_, bottom] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(area);
        frame.render_widget(Paragraph::new(quote).centered(), bottom);
    }
}
