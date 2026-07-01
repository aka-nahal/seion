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
use unicode_width::UnicodeWidthStr;

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

    // Just the view name now — the old "· peaceful music terminal" tagline
    // repeated on every screen and earned its keep nowhere. The splash carries
    // the mood; here we stay quiet and contextual.
    let subtitle_text = if app.view.title().is_empty() {
        "peaceful music terminal".to_string()
    } else {
        app.view.title().to_string()
    };
    let subtitle = Line::from(Span::styled(
        subtitle_text,
        Style::new()
            .fg(utils::lerp_color(theme.background, theme.muted, 0.7))
            .add_modifier(Modifier::ITALIC),
    ))
    .centered();

    frame.render_widget(Paragraph::new(vec![logo, subtitle]).centered(), title_area);

    frame.render_widget(Paragraph::new(separator_line(app, sep_area.width)), sep_area);
}

/// The thin divider beneath the header. When a track is playing it carries a
/// faint, right-aligned `♪ title` so you always know what's on without leaving
/// the current screen; the rest of the row is the usual soft rule.
fn separator_line(app: &App, width: u16) -> Line<'static> {
    let theme = &app.theme;
    let rule = Style::new().fg(ui::border_color(theme));
    let full = width as usize;

    // Only show the indicator when there's a track and the row is wide enough to
    // carry it without crowding the rule.
    if let Some(track) = &app.player.state.current {
        let title = utils::truncate(&track.title, 30);
        let indicator = format!("♪ {title} ");
        let ind_w = UnicodeWidthStr::width(indicator.as_str());
        if full > ind_w + 8 {
            let dashes = full - ind_w - 1;
            return Line::from(vec![
                Span::styled("─".repeat(dashes), rule),
                Span::styled(" ", rule),
                Span::styled(indicator, Style::new().fg(theme.muted)),
            ]);
        }
    }

    Line::from(Span::styled("─".repeat(full), rule))
}
