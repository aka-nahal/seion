//! Rendering. The whole interface is immediate-mode: each frame is drawn from
//! current [`App`] state. This module owns the top-level layout and a few small
//! theme helpers; each screen lives in its own submodule, and lists/bars are
//! shared through [`crate::widgets`] so the calm look stays consistent.

pub mod header;
pub mod help;
pub mod home;
pub mod library;
pub mod lyrics;
pub mod now_playing;
pub mod player_bar;
pub mod queue;
pub mod rain;
pub mod search;
pub mod settings;
pub mod splash;
pub mod visualizer;

use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, BorderType, Padding};

use crate::app::{App, Overlay, View};
use crate::models::Track;
use crate::theme::Theme;
use crate::utils;

/// The content column. Uses the full terminal width — each screen still centers
/// its own content, so wide terminals are filled rather than letterboxed.
fn content_area(screen: Rect) -> Rect {
    screen
}

/// Draw the whole interface for one frame.
pub fn render(frame: &mut Frame, app: &mut App) {
    let screen = frame.area();
    // Wash the whole screen in the background colour first.
    frame.render_widget(Block::new().style(Style::new().bg(app.theme.background)), screen);

    if app.view == View::Splash {
        splash::render(frame, screen, app);
        return;
    }

    // Zen mode: nothing but the track and a full-width visualizer, fullscreen.
    if app.zen_mode {
        now_playing::render_zen(frame, screen, app);
        if app.overlay == Some(Overlay::Help) {
            help::render(frame, screen, app);
        }
        return;
    }

    // Everything else lives in a centered column.
    let area = content_area(screen);
    let [header_area, body_area, player_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(6),
    ])
    .areas(area);

    header::render(frame, header_area, app);

    if app.focus_mode {
        // Focus mode: keep the frame, but the body is only the current track.
        now_playing::render(frame, body_area, app);
    } else {
        match app.view {
            View::Home => home::render(frame, body_area, app),
            View::Search => search::render(frame, body_area, app),
            View::Library => library::render_hub(frame, body_area, app),
            View::Liked => library::render_liked(frame, body_area, app),
            View::History => library::render_history(frame, body_area, app),
            View::Downloads => library::render_downloads(frame, body_area, app),
            View::Playlists => library::render_playlists(frame, body_area, app),
            View::Queue => queue::render(frame, body_area, app),
            View::NowPlaying => now_playing::render(frame, body_area, app),
            View::Lyrics => lyrics::render(frame, body_area, app),
            View::Settings => settings::render(frame, body_area, app),
            View::Splash => {}
        }
    }

    player_bar::render(frame, player_area, app);

    if app.overlay == Some(Overlay::Help) {
        help::render(frame, screen, app);
    }
}

/// A soft, rounded panel with generous padding and an optional centered title.
pub fn panel(theme: &Theme, title: &str) -> Block<'static> {
    titled_block(theme, title).padding(Padding::new(3, 3, 1, 1))
}

/// A slim rounded box with horizontal padding only — for single-line fields.
///
/// The full [`panel`] adds vertical padding, which on a short (e.g. 3-row) box
/// leaves zero rows for content and hides it. Use this for the search bar etc.
pub fn slim_panel(theme: &Theme, title: &str) -> Block<'static> {
    titled_block(theme, title).padding(Padding::horizontal(2))
}

/// Shared rounded, soft-bordered, optionally-titled block (no padding set yet).
fn titled_block(theme: &Theme, title: &str) -> Block<'static> {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border_color(theme)))
        .style(Style::new().bg(theme.background).fg(theme.text));
    if title.is_empty() {
        block
    } else {
        block.title(
            Line::from(format!("  {title}  "))
                .centered()
                .style(Style::new().fg(theme.muted)),
        )
    }
}

/// A whisper-soft border colour — partway between background and muted text.
pub fn border_color(theme: &Theme) -> Color {
    utils::lerp_color(theme.background, theme.muted, 0.4)
}

/// The gentle wash behind a selected list row.
pub fn selection_bg(theme: &Theme) -> Color {
    utils::lerp_color(theme.background, theme.selection, 0.45)
}

/// A colour along the theme's gentle spectrum — `secondary` at the foot,
/// through `accent`, up to `highlight` at the crest. Used for the visualizer's
/// vertical gradient so the band is coloured by the active theme. `t` is
/// clamped to `0..=1`.
pub fn level_color(theme: &Theme, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        utils::lerp_color(theme.secondary, theme.accent, t * 2.0)
    } else {
        utils::lerp_color(theme.accent, theme.highlight, (t - 0.5) * 2.0)
    }
}

/// The index of the currently playing track within `list`, if it appears there
/// — so views can mark it with a small ♪.
pub fn now_playing_index(list: &[Track], current: &Option<Track>) -> Option<usize> {
    let current = current.as_ref()?;
    list.iter().position(|t| t.id == current.id)
}

/// Centre a fixed-size rect within `area` (used for overlays and the splash).
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [horizontal] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [centered] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(horizontal);
    centered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Overlay};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Every screen — plus overlay, zen and focus modes — must render without
    /// panicking on a normal-sized terminal.
    #[tokio::test(flavor = "current_thread")]
    async fn all_views_render() {
        let (mut app, _rx) = App::new().await.expect("app builds");
        let mut terminal = Terminal::new(TestBackend::new(90, 32)).unwrap();

        let views = [
            View::Splash,
            View::Home,
            View::Search,
            View::Library,
            View::Liked,
            View::History,
            View::Playlists,
            View::Downloads,
            View::Queue,
            View::NowPlaying,
            View::Lyrics,
            View::Settings,
        ];
        for view in views {
            app.view = view;
            terminal.draw(|f| render(f, &mut app)).unwrap();
        }

        app.overlay = Some(Overlay::Help);
        terminal.draw(|f| render(f, &mut app)).unwrap();
        app.overlay = None;

        app.zen_mode = true;
        terminal.draw(|f| render(f, &mut app)).unwrap();
        app.zen_mode = false;

        app.focus_mode = true;
        terminal.draw(|f| render(f, &mut app)).unwrap();
    }

    /// Layouts must survive an absurdly small terminal (clamped/zero-size rects).
    #[tokio::test(flavor = "current_thread")]
    async fn renders_in_a_tiny_terminal() {
        let (mut app, _rx) = App::new().await.unwrap();
        let mut terminal = Terminal::new(TestBackend::new(6, 3)).unwrap();
        for view in [View::Home, View::Search, View::NowPlaying] {
            app.view = view;
            terminal.draw(|f| render(f, &mut app)).unwrap();
        }
    }

    /// Now-playing must render both with the visualizer and with it off, and zen
    /// mode must render at both a normal and an absurdly small size.
    #[tokio::test(flavor = "current_thread")]
    async fn visualizer_and_zen_render() {
        let (mut app, _rx) = App::new().await.unwrap();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        app.view = View::NowPlaying;
        for on in [true, false] {
            app.config.visualizer = on;
            terminal.draw(|f| render(f, &mut app)).unwrap();
            app.zen_mode = true;
            terminal.draw(|f| render(f, &mut app)).unwrap();
            app.zen_mode = false;
        }
        // Zen on a near-zero terminal must not panic either.
        let mut tiny = Terminal::new(TestBackend::new(5, 4)).unwrap();
        app.zen_mode = true;
        tiny.draw(|f| render(f, &mut app)).unwrap();
    }

    #[test]
    fn level_color_spans_the_theme_spectrum() {
        let t = Theme::kyoto_night();
        assert_eq!(level_color(&t, 0.0), t.secondary);
        assert_eq!(level_color(&t, 0.5), t.accent);
        assert_eq!(level_color(&t, 1.0), t.highlight);
        // Out-of-range input is clamped, not panicked.
        assert_eq!(level_color(&t, -1.0), t.secondary);
        assert_eq!(level_color(&t, 2.0), t.highlight);
    }
}
