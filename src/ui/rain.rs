//! Subtle idle rain — a few faint drops drifting down the screen.
//!
//! Drawn directly into the frame buffer so it sits behind everything else. It is
//! procedural (a function of the tick counter and column), so there is no state
//! to keep and it costs almost nothing.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::App;
use crate::utils;

/// Cheap integer hash, to scatter the drops across columns deterministically.
fn hash(n: u64) -> u64 {
    let mut x = n.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Draw the rain over `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let theme = &app.theme;
    let drop_color = utils::lerp_color(theme.background, theme.muted, 0.22);
    let trail_color = utils::lerp_color(theme.background, theme.muted, 0.12);
    let height = area.height as u64;
    // Advance every other tick so the rain keeps its slow, calm fall even though
    // the ticker now runs at ~10 Hz (for the visualizer's sake).
    let t = app.tick_count / 2;
    let buf = frame.buffer_mut();

    for col in 0..area.width {
        let seed = hash(col as u64);
        // Only about one column in four carries a drop — keep it sparse.
        if !seed.is_multiple_of(4) {
            continue;
        }
        let speed = 1 + (seed >> 5) % 2;
        let y = area.y + ((t * speed + seed) % height) as u16;

        if let Some(cell) = buf.cell_mut((area.x + col, y)) {
            cell.set_char('│').set_style(Style::new().fg(drop_color));
        }
        if y > area.y
            && let Some(cell) = buf.cell_mut((area.x + col, y - 1))
        {
            cell.set_char('·').set_style(Style::new().fg(trail_color));
        }
    }
}
