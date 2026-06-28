//! Now playing: the current track, centered and unhurried. Also used for focus
//! mode (in the body) and zen mode (fullscreen).

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::ui;
use crate::widgets::progress;

/// Render the now-playing view into `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let state = &app.player.state;

    let mut lines = vec![
        Line::from(Span::styled("☾", Style::new().fg(theme.secondary))).centered(),
        Line::from(""),
    ];

    match &state.current {
        Some(track) => {
            lines.push(
                Line::from(Span::styled(
                    track.title.clone(),
                    Style::new().fg(theme.text).add_modifier(Modifier::BOLD),
                ))
                .centered(),
            );
            if !track.artist.is_empty() {
                lines.push(
                    Line::from(Span::styled(track.artist.clone(), Style::new().fg(theme.muted)))
                        .centered(),
                );
            }
            if let Some(album) = &track.album {
                lines.push(
                    Line::from(Span::styled(
                        album.clone(),
                        Style::new().fg(theme.muted).add_modifier(Modifier::ITALIC),
                    ))
                    .centered(),
                );
            }
        }
        None => {
            lines.push(
                Line::from(Span::styled(
                    "nothing playing",
                    Style::new().fg(theme.muted).add_modifier(Modifier::ITALIC),
                ))
                .centered(),
            );
        }
    }

    let box_width = area.width.clamp(1, 64);
    let center = ui::centered(area, box_width, 9);
    let [text_area, _gap, progress_area] = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(center);

    frame.render_widget(Paragraph::new(lines).centered(), text_area);

    if state.has_track() {
        progress::render(frame, progress_area, theme, state.position, state.duration);
    }
}
