//! The one list widget every track view shares — keeping selection, spacing and
//! colour identical across search, library, queue and home.

use ratatui::Frame;
use ratatui::layout::{Margin, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    HighlightSpacing, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState,
};

use crate::models::Track;
use crate::theme::Theme;
use crate::ui;

/// Per-list rendering options.
#[derive(Default, Clone)]
pub struct Options {
    /// Message shown (centered, muted) when the list is empty.
    pub empty: &'static str,
    /// Show each track's duration after the artist.
    pub show_duration: bool,
    /// Index to mark as the one currently playing (a small ♪).
    pub now_playing: Option<usize>,
    /// Rows before this index are dimmed as "already played" (queue history).
    pub played_before: Option<usize>,
    /// Prefix each row with its position number.
    pub numbered: bool,
}

/// Render a list of tracks inside a titled panel, with selection.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    theme: &Theme,
    title: &str,
    tracks: &[Track],
    state: &mut ListState,
    opts: &Options,
) {
    let block = ui::panel(theme, title);

    if tracks.is_empty() {
        let inner = block.inner(area);
        frame.render_widget(block, area);
        // A small, still motif rests above the message so an empty corner feels
        // intentional and calm rather than simply blank.
        let lines = vec![
            Line::from(Span::styled(
                "❀",
                Style::new().fg(theme.secondary).add_modifier(Modifier::DIM),
            ))
            .centered(),
            Line::from(""),
            Line::from(Span::styled(
                opts.empty,
                Style::new().fg(theme.muted).add_modifier(Modifier::ITALIC),
            ))
            .centered(),
        ];
        let center = ui::centered(inner, inner.width, lines.len() as u16);
        frame.render_widget(Paragraph::new(lines).centered(), center);
        return;
    }

    // A quiet position indicator in the title: "history  ·  12/100".
    let titled = match state.selected() {
        Some(i) => format!("{title}  ·  {}/{}", i + 1, tracks.len()),
        None => format!("{title}  ·  {}", tracks.len()),
    };
    let block = ui::panel(theme, &titled);
    let inner = block.inner(area);

    let items: Vec<ListItem> = tracks
        .iter()
        .enumerate()
        .map(|(i, track)| ListItem::new(build_line(theme, i, track, opts)))
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::new()
                .fg(theme.highlight)
                .bg(ui::selection_bg(theme))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("  ▸ ")
        .highlight_spacing(HighlightSpacing::Always);

    frame.render_stateful_widget(list, area, state);

    // A faint scrollbar, drawn only when the list overflows its visible rows. It
    // rides the right border (using the same soft glyph) so it reads as part of
    // the frame rather than an added element.
    if tracks.len() > inner.height as usize {
        let mut scroll_state = ScrollbarState::new(tracks.len())
            .position(state.selected().unwrap_or_else(|| state.offset()));
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("█")
            .track_style(Style::new().fg(ui::border_color(theme)))
            .thumb_style(Style::new().fg(theme.muted));
        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin { vertical: 1, horizontal: 0 }),
            &mut scroll_state,
        );
    }
}

/// Build the single styled line for one track.
fn build_line<'a>(theme: &Theme, index: usize, track: &'a Track, opts: &Options) -> Line<'a> {
    let played = opts.played_before.map(|b| index < b).unwrap_or(false);
    let is_now = opts.now_playing == Some(index);

    let title_color = if played { theme.muted } else { theme.text };
    let muted = Style::new().fg(theme.muted);

    let mut spans: Vec<Span> = Vec::new();

    if opts.numbered {
        spans.push(Span::styled(format!("{:>2}  ", index + 1), muted));
    }
    if is_now {
        spans.push(Span::styled("♪ ", Style::new().fg(theme.accent)));
    }

    spans.push(Span::styled(
        track.title.clone(),
        Style::new().fg(title_color),
    ));

    if !track.artist.is_empty() {
        spans.push(Span::styled("   ·   ", muted));
        spans.push(Span::styled(track.artist.clone(), muted));
    }
    if opts.show_duration {
        spans.push(Span::styled(format!("   {}", track.duration_str()), muted));
    }

    Line::from(spans)
}
