//! Lyrics: soft, centered, no border. When no lyrics are available (the default
//! in this build) the view simply rests on the song's name.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::lyrics;
use crate::ui;

/// Render the lyrics view.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let current = &app.player.state.current;

    let mut lines = Vec::new();

    match current {
        Some(track) => {
            lines.push(
                Line::from(Span::styled(
                    track.title.clone(),
                    Style::new().fg(theme.text).add_modifier(Modifier::BOLD),
                ))
                .centered(),
            );
            lines.push(Line::from(""));

            match lyrics::for_track(track) {
                Some(found) => {
                    for line in found.lines {
                        lines.push(
                            Line::from(Span::styled(line, Style::new().fg(theme.muted))).centered(),
                        );
                    }
                }
                None => {
                    lines.push(
                        Line::from(Span::styled(
                            "the music speaks for itself",
                            Style::new().fg(theme.muted).add_modifier(Modifier::ITALIC),
                        ))
                        .centered(),
                    );
                }
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

    let height = (lines.len() as u16).max(1);
    let center = ui::centered(area, area.width.clamp(1, 70), height);
    frame.render_widget(Paragraph::new(lines).centered(), center);
}
