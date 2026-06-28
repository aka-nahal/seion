//! The one list widget every track view shares — keeping selection, spacing and
//! colour identical across search, library, queue and home.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{HighlightSpacing, List, ListItem, ListState, Paragraph};

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
        let message = Paragraph::new(opts.empty)
            .centered()
            .style(Style::new().fg(theme.muted).add_modifier(Modifier::ITALIC));
        frame.render_widget(message, inner);
        return;
    }

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
