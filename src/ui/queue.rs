//! The queue: what's playing, what's next, and what's behind us.

use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::App;
use crate::widgets::track_list::{self, Options};

/// Render the queue.
pub fn render(frame: &mut Frame, area: Rect, app: &mut App) {
    let theme = app.theme;
    let current = app.player.queue_pos;
    let opts = Options {
        empty: "the queue is empty",
        show_duration: true,
        numbered: true,
        now_playing: current,
        // Everything before the current track has already been heard — fade it.
        played_before: current,
    };
    track_list::render(
        frame,
        area,
        &theme,
        "queue",
        &app.player.queue,
        &mut app.queue_state,
        &opts,
    );
}
