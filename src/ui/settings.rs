//! Settings: a short, clean list. Enter cycles the focused value.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{HighlightSpacing, List, ListItem};

use crate::app::App;
use crate::ui;

/// Render the settings screen.
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;

    let backend = if app.player.is_available() {
        app.config.mpv_path.clone()
    } else {
        format!("{}  (not found)", app.config.mpv_path)
    };

    // Order must match `crate::app::SETTINGS_ITEMS`.
    let values = [
        format!("theme            {}", theme.name),
        format!("idle rain        {}", on_off(app.config.rain_on_idle)),
        format!("daily quote      {}", on_off(app.config.daily_quote)),
        format!("audio backend    {backend}"),
        format!("search results   {}", app.config.search_limit),
    ];

    let items: Vec<ListItem> = values
        .iter()
        .map(|v| ListItem::new(Line::from(Span::styled(v.clone(), Style::new().fg(theme.text)))))
        .collect();

    let list = List::new(items)
        .block(ui::panel(&theme, "settings · enter to change"))
        .highlight_style(
            Style::new()
                .fg(theme.highlight)
                .bg(ui::selection_bg(&theme))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("  ▸ ")
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(list, area, &mut app.settings_state);
}

/// Render a boolean as a calm "on"/"off".
fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}
