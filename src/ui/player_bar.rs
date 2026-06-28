//! The always-present player bar at the foot of the screen.

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Padding, Paragraph};

use crate::app::App;
use crate::ui;
use crate::widgets::progress;

/// Render the player bar into its (6-row) area.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let state = &app.player.state;

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(ui::border_color(theme)))
        .style(Style::new().bg(theme.background).fg(theme.text))
        .padding(Padding::horizontal(2));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 3 {
        return; // too small to draw the bar meaningfully
    }

    let [track_row, progress_row, controls_row] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .areas(inner);

    // --- the track line ---
    let (glyph, glyph_color) = if state.loading {
        ("…", theme.muted)
    } else if !state.has_track() {
        ("♪", theme.muted)
    } else if state.paused {
        ("‖", theme.secondary)
    } else {
        ("▶", theme.accent)
    };

    let track_line = match &state.current {
        Some(track) => {
            let mut spans = vec![
                Span::styled(format!("{glyph}  "), Style::new().fg(glyph_color)),
                Span::styled(
                    track.title.clone(),
                    Style::new().fg(theme.text).add_modifier(Modifier::BOLD),
                ),
            ];
            if !track.artist.is_empty() {
                spans.push(Span::styled("   ·   ", Style::new().fg(theme.muted)));
                spans.push(Span::styled(track.artist.clone(), Style::new().fg(theme.muted)));
            }
            if state.loading {
                spans.push(Span::styled("   loading…", Style::new().fg(theme.muted)));
            }
            Line::from(spans)
        }
        None => Line::from(vec![
            Span::styled(format!("{glyph}  "), Style::new().fg(glyph_color)),
            Span::styled(
                "silence — press / to search",
                Style::new()
                    .fg(theme.muted)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]),
    };
    frame.render_widget(Paragraph::new(track_line), track_row);

    // --- the progress line ---
    progress::render(frame, progress_row, theme, state.position, state.duration);

    // --- controls + transient status ---
    let [controls_area, status_area] =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
            .areas(controls_row);
    frame.render_widget(Paragraph::new(controls_line(app)), controls_area);

    if let Some(status) = &app.status {
        let color = if status.error {
            theme.error
        } else {
            theme.secondary
        };
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(status.text.clone(), Style::new().fg(color))))
                .right_aligned(),
            status_area,
        );
    }
}

/// The left-hand controls summary: volume, repeat, shuffle, like, queue length.
fn controls_line(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let muted = Style::new().fg(theme.muted);
    let accent = Style::new().fg(theme.accent);
    let separator = || Span::styled("   ", muted);

    let mut spans = vec![
        Span::styled(format!("vol {}", app.player.state.volume), muted),
        separator(),
        Span::styled(
            app.player.repeat.glyph().to_string(),
            if app.player.repeat.is_on() { accent } else { muted },
        ),
        separator(),
        Span::styled(
            "shuffle",
            if app.player.shuffle { accent } else { muted },
        ),
    ];

    if let Some(track) = &app.player.state.current {
        // Check the in-memory liked list rather than hitting SQLite every frame.
        let liked = app.liked.iter().any(|t| t.id == track.id);
        spans.push(separator());
        spans.push(Span::styled(
            if liked { "♥" } else { "♡" },
            if liked { accent } else { muted },
        ));
    }

    spans.push(separator());
    spans.push(Span::styled(
        format!("queue {}", app.player.queue.len()),
        muted,
    ));

    Line::from(spans)
}
