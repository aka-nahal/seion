//! Settings: a short, clean list. Enter cycles the focused value. The theme row
//! carries a small strip of live colour swatches so cycling shows the palette.
//!
//! Unlike the other list screens this is drawn by hand rather than with `List`,
//! because `List`'s highlight style would repaint the swatch foregrounds and the
//! palette preview would collapse to a single colour on the selected row.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::App;
use crate::theme::Theme;
use crate::ui;

/// Render the settings screen.
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;

    let backend = if app.player.is_available() {
        app.config.mpv_path.clone()
    } else {
        format!("{}  (not found)", app.config.mpv_path)
    };

    // The toggle reads on/off, with a gentle nudge when no application id is set.
    let discord = if app.config.discord_client_id.trim().is_empty() {
        format!("{}  (set client id)", on_off(app.config.discord_presence))
    } else {
        on_off(app.config.discord_presence)
    };

    // Order must match `crate::app::SETTINGS_ITEMS`.
    let rows: [(&str, String); 7] = [
        ("theme", theme.name.to_string()),
        ("idle rain", on_off(app.config.rain_on_idle)),
        ("visualizer", on_off(app.config.visualizer)),
        ("discord presence", discord),
        ("daily quote", on_off(app.config.daily_quote)),
        ("audio backend", backend),
        ("search results", app.config.search_limit.to_string()),
    ];

    let block = ui::panel(&theme, "settings · enter to change");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let selected = app.settings_state.selected().unwrap_or(0);

    for (i, (label, value)) in rows.iter().enumerate() {
        let y = inner.y + i as u16;
        if y >= inner.bottom() {
            break;
        }
        let row = Rect { x: inner.x, y, width: inner.width, height: 1 };
        let focused = i == selected;

        // The selection wash sits behind the whole row; the text patches only
        // foregrounds on top of it, so the swatch colours come through intact.
        if focused {
            frame.render_widget(
                Block::new().style(Style::new().bg(ui::selection_bg(&theme))),
                row,
            );
        }

        let prefix = if focused { "  ▸ " } else { "    " };
        let text_style = if focused {
            Style::new().fg(theme.highlight).add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(theme.text)
        };

        let mut spans = vec![
            Span::styled(prefix, Style::new().fg(theme.accent)),
            Span::styled(format!("{label:<16}{value}"), text_style),
        ];
        if i == 0 {
            spans.extend(swatches(&theme));
        }

        frame.render_widget(Paragraph::new(Line::from(spans)), row);
    }
}

/// A small strip of the theme's colours, for the live palette preview.
fn swatches(theme: &Theme) -> Vec<Span<'static>> {
    [
        theme.accent,
        theme.secondary,
        theme.highlight,
        theme.muted,
        theme.success,
    ]
    .into_iter()
    .map(|c| Span::styled("  ██", Style::new().fg(c)))
    .collect()
}

/// Render a boolean as a calm "on"/"off".
fn on_off(value: bool) -> String {
    if value { "on" } else { "off" }.to_string()
}
