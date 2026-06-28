//! Now playing: the current track over a soft visualizer band. Also used for
//! focus mode (in the body) and zen mode (fullscreen, full-width visualizer).
//!
//! When the visualizer is switched off (see settings / the `v` key) the screen
//! falls back to the older, perfectly still centered card.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::player::PlayerState;
use crate::theme::Theme;
use crate::ui::{self, visualizer};
use crate::widgets::progress;

/// Render the now-playing view into `area` (also used as the focus-mode body).
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if app.config.visualizer {
        render_with_visualizer(frame, area, app);
    } else {
        render_calm(frame, area, app);
    }
}

/// Render the fullscreen zen variant: a full-width visualizer with the track
/// resting above it.
pub fn render_zen(frame: &mut Frame, area: Rect, app: &App) {
    if !app.config.visualizer {
        render_calm(frame, area, app);
        return;
    }
    let theme = &app.theme;
    let state = &app.player.state;

    let [info_area, progress_area, viz_area] = Layout::vertical([
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Percentage(55),
    ])
    .areas(area);

    let lines = info_lines(theme, state);
    let height = (lines.len() as u16).min(info_area.height.max(1));
    let centered = ui::centered(info_area, info_area.width, height);
    frame.render_widget(Paragraph::new(lines).centered(), centered);

    render_progress(frame, progress_area, theme, state);
    visualizer::render(frame, viz_area, app);
}

/// The default look: the track up top, a progress line, then the visualizer
/// filling whatever vertical space remains.
fn render_with_visualizer(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let state = &app.player.state;

    let lines = info_lines(theme, state);
    let info_height = lines.len() as u16;

    // A breath above the title, the info, a breath, the progress, a breath, then
    // the visualizer claims the rest of the screen.
    let [_top, info_area, _gap, progress_area, _gap2, viz_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(info_height),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);

    frame.render_widget(Paragraph::new(lines).centered(), info_area);
    render_progress(frame, progress_area, theme, state);
    visualizer::render(frame, viz_area, app);
}

/// The still fallback (visualizer off): everything centered in a small card.
fn render_calm(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let state = &app.player.state;

    let mut lines = vec![
        Line::from(Span::styled("☾", Style::new().fg(theme.secondary))).centered(),
        Line::from(""),
    ];
    lines.extend(info_lines(theme, state));

    let box_width = area.width.clamp(1, 64);
    let center = ui::centered(area, box_width, 9);
    let [text_area, _gap, progress_area] = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(center);

    frame.render_widget(Paragraph::new(lines).centered(), text_area);
    render_progress(frame, progress_area, theme, state);
}

/// The centered title / artist / album lines (or a quiet "nothing playing").
fn info_lines(theme: &Theme, state: &PlayerState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
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
    lines
}

/// Draw the progress line, capped to a calm width and centered, but only when a
/// track is actually loaded.
fn render_progress(frame: &mut Frame, area: Rect, theme: &Theme, state: &PlayerState) {
    if !state.has_track() || area.height == 0 {
        return;
    }
    let width = area.width.min(72);
    let line = ui::centered(area, width, 1);
    progress::render(frame, line, theme, state.position, state.duration);
}
