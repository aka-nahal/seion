//! A calm audio visualizer — a soft band of bars that breathe with playback.
//!
//! mpv gives us no real spectrum data over its IPC, so the motion here is
//! *procedural*: a sum of slow sine waves, seeded per column and gently
//! windowed, so it reads as music without ever being loud. While paused the band
//! holds a low breath; with nothing playing it sinks to a near-still line. It is
//! drawn straight into the frame buffer (like the rain) for per-cell control of
//! glyph and colour, and is a pure function of the tick counter — no state.

use std::f32::consts::PI;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::App;
use crate::ui;

/// Eighth-block glyphs: index `n` fills `n`/8 of a cell from the bottom up.
const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Cheap integer hash, to give each column its own stable phase offset.
fn hash(n: u64) -> u64 {
    let mut x = n.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Draw the visualizer band across `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let theme = &app.theme;
    let state = &app.player.state;

    // How "loud" the band is: full while playing, a held breath when paused,
    // and nearly still when there is nothing to play — the screen never feels
    // dead, but it never shouts either.
    let amp = if state.is_playing() {
        1.0
    } else if state.has_track() {
        0.18
    } else {
        0.08
    };

    // A smooth, unitless time. The ticker runs at ~10 Hz (see `crate::app`), so
    // each tick is about a tenth of a second; this keeps the motion gentle.
    let t = app.tick_count as f32 * 0.1;

    let width = area.width;
    let height = area.height;
    let height_f = height as f32;
    let buf = frame.buffer_mut();

    // One-column bars with a one-column gap between them: airier and calmer than
    // a solid wall, and it still spans the whole width.
    for col in (0..width).step_by(2) {
        let frac = if width > 1 {
            col as f32 / (width - 1) as f32
        } else {
            0.5
        };

        // A soft hump across the band so its edges taper rather than cut off.
        let window = 0.45 + 0.55 * (PI * frac).sin();
        // Per-column phase so neighbours never move in lockstep.
        let phase = (hash(col as u64) % 1000) as f32 / 1000.0 * (PI * 2.0);

        // A few layered waves of different speeds and wavelengths read as music.
        let wave = 0.5
            + 0.26 * (t * 1.6 + frac * 9.0 + phase).sin()
            + 0.16 * (t * 2.7 - frac * 15.0 + phase * 1.7).sin()
            + 0.10 * (t * 0.9 + frac * 4.0).sin();

        let level = (wave * window * amp).clamp(0.0, 1.0);

        // Total fill for this column, measured in eighths of a cell.
        let eighths = (level * height_f * 8.0).round() as u32;
        let full = (eighths / 8) as u16;
        let remainder = (eighths % 8) as usize;

        for cell_from_bottom in 0..height {
            let glyph = if cell_from_bottom < full {
                '█'
            } else if cell_from_bottom == full && remainder > 0 {
                BLOCKS[remainder]
            } else {
                break; // nothing rises above the crest of this bar
            };

            // Vertical gradient: secondary at the foot, highlight at the crest.
            let ratio = cell_from_bottom as f32 / height_f;
            let color = ui::level_color(theme, ratio);

            let y = area.y + height - 1 - cell_from_bottom;
            if let Some(cell) = buf.cell_mut((area.x + col, y)) {
                cell.set_char(glyph).set_style(Style::new().fg(color));
            }
        }
    }
}
