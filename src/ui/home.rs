//! Home: a quiet greeting, today's line, and what you were last listening to.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::ui::{self, rain};
use crate::widgets::track_list::{self, Options};

/// Render the home screen.
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;
    let [greeting_area, list_area] =
        Layout::vertical([Constraint::Length(4), Constraint::Min(0)]).areas(area);

    // A little rain drifts behind the greeting when nothing is playing.
    if app.config.rain_on_idle && !app.player.state.is_playing() {
        rain::render(frame, greeting_area, app);
    }

    let (jp, en) = app.quote;
    let mut greeting = vec![
        Line::from(Span::styled("welcome back", Style::new().fg(theme.text))).centered(),
    ];
    if app.config.daily_quote {
        greeting.push(
            Line::from(Span::styled(
                format!("{jp}   —   {en}"),
                Style::new().fg(theme.muted).add_modifier(Modifier::ITALIC),
            ))
            .centered(),
        );
    } else {
        greeting.push(
            Line::from(Span::styled(
                "press / to search for something calm",
                Style::new().fg(theme.muted),
            ))
            .centered(),
        );
    }
    frame.render_widget(Paragraph::new(greeting).centered(), greeting_area);

    let now = ui::now_playing_index(&app.history, &app.player.state.current);
    let opts = Options {
        empty: "nothing yet — press / to search",
        show_duration: true,
        now_playing: now,
        ..Options::default()
    };
    track_list::render(
        frame,
        list_area,
        &theme,
        "recently played",
        &app.history,
        &mut app.history_state,
        &opts,
    );
}
