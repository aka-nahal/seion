//! Search: a single calm input line and the results beneath it.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{HighlightSpacing, List, ListItem, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, PendingLink, SearchKind};
use crate::ui;
use crate::widgets::track_list::{self, Options};

/// Render the search screen.
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;
    let [input_area, results_area] =
        Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).areas(area);

    // --- the input line ---
    // A slim box (horizontal padding only): the full panel's vertical padding
    // would eat the single content row and hide everything you type.
    let block = ui::slim_panel(&theme, "search");
    let inner = block.inner(input_area);
    frame.render_widget(block, input_area);

    let prompt_style = if app.editing {
        Style::new().fg(theme.accent)
    } else {
        Style::new().fg(theme.muted)
    };
    let text = app.search.text();
    let body = if text.is_empty() {
        // Always show the hint when empty (even while editing) so the box never
        // looks blank; the cursor sits at its start.
        Span::styled(
            "type to search…",
            Style::new().fg(theme.muted).add_modifier(Modifier::ITALIC),
        )
    } else {
        Span::styled(text.to_string(), Style::new().fg(theme.text))
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled("› ", prompt_style), body])),
        inner,
    );

    // Place a soft cursor while editing. The prompt "› " is two columns wide;
    // the caret offset is the *display width* of the text before the cursor, so
    // it lands correctly even amid wide CJK glyphs (the app's whole reason for
    // being).
    if app.editing {
        let before: String = app.search.text().chars().take(app.search.cursor()).collect();
        let cursor_x = inner.x + 2 + UnicodeWidthStr::width(before.as_str()) as u16;
        if cursor_x < inner.x + inner.width {
            frame.set_cursor_position((cursor_x, inner.y));
        }
    }

    // --- results ---
    // A pasted YouTube link gets a load prompt instead of live results.
    if let Some(link) = app.pending_link() {
        render_link_hint(frame, results_area, &app.theme, &link);
        return;
    }
    match app.search_kind {
        SearchKind::Tracks => render_track_results(frame, results_area, app),
        SearchKind::Playlists => render_playlist_results(frame, results_area, app),
    }
}

/// The prompt shown when the box holds a YouTube link, ready to load.
fn render_link_hint(frame: &mut Frame, area: Rect, theme: &crate::theme::Theme, link: &PendingLink) {
    let (panel_title, glyph, detected, action) = match link {
        PendingLink::Playlist(_) => (
            "playlist link",
            "≣  ",
            "playlist link detected",
            "press enter to load the whole playlist",
        ),
        PendingLink::Video(_) => (
            "track link",
            "♪  ",
            "track link detected",
            "press enter to play this track",
        ),
    };

    let block = ui::panel(theme, panel_title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines = vec![
        Line::from(vec![
            Span::styled(glyph, Style::new().fg(theme.accent)),
            Span::styled(detected, Style::new().fg(theme.accent)),
        ])
        .centered(),
        Line::from(""),
        Line::from(Span::styled(
            action,
            Style::new().fg(theme.muted).add_modifier(Modifier::ITALIC),
        ))
        .centered(),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

/// The ordinary track results list.
fn render_track_results(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;
    let now = ui::now_playing_index(&app.results, &app.player.state.current);
    let title = if app.searching { "searching…" } else { "results" };
    let opts = Options {
        empty: if app.searching {
            "listening for something…"
        } else {
            "no results yet"
        },
        show_duration: true,
        now_playing: now,
        ..Options::default()
    };
    track_list::render(frame, area, &theme, title, &app.results, &mut app.results_state, &opts);
}

/// Playlist results — entered with `/playlist <query>`. Enter opens (and plays).
fn render_playlist_results(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;
    let title = if app.searching {
        "searching playlists…"
    } else if app.playlists.is_empty() {
        "playlists"
    } else {
        "playlists · enter plays · a queues"
    };
    let block = ui::panel(&theme, title);

    if app.playlists.is_empty() {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let message = if app.searching {
            "looking for playlists…"
        } else {
            "no playlists — try /playlist lofi hip hop"
        };
        frame.render_widget(
            Paragraph::new(message)
                .centered()
                .style(Style::new().fg(theme.muted).add_modifier(Modifier::ITALIC)),
            inner,
        );
        return;
    }

    let muted = Style::new().fg(theme.muted);
    let items: Vec<ListItem> = app
        .playlists
        .iter()
        .map(|playlist| {
            let mut spans = vec![
                Span::styled("≣ ", Style::new().fg(theme.accent)),
                Span::styled(playlist.title.clone(), Style::new().fg(theme.text)),
            ];
            if !playlist.uploader.is_empty() {
                spans.push(Span::styled("   ·   ", muted));
                spans.push(Span::styled(playlist.uploader.clone(), muted));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::new()
                .fg(theme.highlight)
                .bg(ui::selection_bg(&theme))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("  ▸ ")
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(list, area, &mut app.results_state);
}
