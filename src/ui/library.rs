//! The library hub and its sub-views (liked, history, downloads, playlists).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{HighlightSpacing, List, ListItem, Paragraph};

use crate::app::{App, LIBRARY_ITEMS};
use crate::ui;
use crate::widgets::track_list::{self, Options};

/// The hub menu: pick a corner of your library.
pub fn render_hub(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;
    let items: Vec<ListItem> = LIBRARY_ITEMS
        .iter()
        .map(|(label, _)| ListItem::new(Line::from(Span::styled(*label, Style::new().fg(theme.text)))))
        .collect();

    let list = List::new(items)
        .block(ui::panel(&theme, "library"))
        .highlight_style(
            Style::new()
                .fg(theme.highlight)
                .bg(ui::selection_bg(&theme))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("  ▸ ")
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(list, area, &mut app.library_state);
}

/// Liked songs.
pub fn render_liked(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;
    let now = ui::now_playing_index(&app.liked, &app.player.state.current);
    let opts = Options {
        empty: "no liked songs yet — press l on a track",
        show_duration: true,
        now_playing: now,
        ..Options::default()
    };
    track_list::render(frame, area, &theme, "liked songs", &app.liked, &mut app.liked_state, &opts);
}

/// Listening history.
pub fn render_history(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;
    let now = ui::now_playing_index(&app.history, &app.player.state.current);
    let opts = Options {
        empty: "no history yet",
        show_duration: true,
        now_playing: now,
        ..Options::default()
    };
    track_list::render(frame, area, &theme, "history", &app.history, &mut app.history_state, &opts);
}

/// Downloaded, offline-ready tracks.
pub fn render_downloads(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;
    let now = ui::now_playing_index(&app.downloads, &app.player.state.current);
    let opts = Options {
        empty: "no downloads yet — press d on a track",
        now_playing: now,
        ..Options::default()
    };
    track_list::render(frame, area, &theme, "downloads", &app.downloads, &mut app.downloads_state, &opts);
}

/// Playlists — search YouTube playlists from here via the `/playlist` command;
/// saved local playlists are still to come.
pub fn render_playlists(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let block = ui::panel(theme, "playlists");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = vec![
        Line::from(Span::styled(
            "search YouTube playlists with  /playlist <query>",
            Style::new().fg(theme.muted),
        ))
        .centered(),
        Line::from(Span::styled(
            "…or paste a playlist URL into search to load it",
            Style::new().fg(theme.muted),
        ))
        .centered(),
        Line::from(""),
        Line::from(Span::styled(
            "saved playlists are coming to this calm corner",
            Style::new().fg(theme.muted).add_modifier(Modifier::ITALIC),
        ))
        .centered(),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}
