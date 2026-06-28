//! Search: a single calm input line and the results beneath it.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

use crate::app::App;
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
    track_list::render(
        frame,
        results_area,
        &theme,
        title,
        &app.results,
        &mut app.results_state,
        &opts,
    );
}
