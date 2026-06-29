//! A cute, calm "visualizer" — a little lofi cat that listens along, sitting in
//! a softly swaying field of sound.
//!
//! mpv gives us no real spectrum data over its IPC, so instead of faking precise
//! bars we lean into the app's mood. A small ASCII cat sits in the band,
//! breathing gently (a one-row bob) with a slowly swishing tail; soft musical
//! notes drift up from it while a track plays, and a low row of soft blocks
//! sways at its feet like a gentle equalizer. Paused, it dozes and sleepy `z`s
//! rise instead; while a stream is loading it perks up with a curious `?`; with
//! nothing playing it curls up asleep and the field falls almost still.
//!
//! It is drawn straight into the frame buffer (like the rain) for per-cell
//! control of glyph and colour, and is a pure function of the tick counter —
//! no state to keep.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};

use crate::app::App;
use crate::{ui, utils};

/// Horizontal offsets (from the cat's centre) the floaters rise along.
const OFFSETS: [i32; 5] = [0, -3, 3, -5, 5];

/// The soft block glyphs the equalizer baseline rises through, low to high.
const BARS: [char; 8] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇'];

/// The cat's mood, which decides its eyes and what drifts up from it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mood {
    /// A track is playing — happy eyes, music notes, a lively field.
    Playing,
    /// A stream is resolving / loading — curious, a rising `?`.
    Loading,
    /// Loaded but paused — dozing, sleepy `z`s.
    Paused,
    /// Nothing playing — curled up asleep.
    Idle,
}

/// Cheap integer hash, to give each floater a stable, scattered phase.
fn hash(n: u64) -> u64 {
    let mut x = n.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// The three rows of the cat, given its eyes (always seven cells wide so it
/// centres cleanly). Spaces are transparent — the background shows through.
fn cat_rows(eyes: &str) -> [String; 3] {
    [" /\\_/\\ ".to_string(), format!("( {eyes} )"), " > ^ < ".to_string()]
}

/// Draw the visualizer scene across `area`.
pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let theme = &app.theme;
    let state = &app.player.state;
    let tick = app.tick_count;

    let mood = if state.loading {
        Mood::Loading
    } else if state.is_playing() {
        Mood::Playing
    } else if state.has_track() {
        Mood::Paused
    } else {
        Mood::Idle
    };

    // The swaying field of sound at the very foot of the band, drawn first so
    // the cat sits on top of it.
    draw_equalizer(frame, area, theme, mood, tick);

    // Eyes: open and bright while playing (with the occasional slow blink),
    // wide and curious while loading, softly shut while dozing or asleep.
    let blink = mood == Mood::Playing && tick % 28 < 2;
    let eyes = match mood {
        Mood::Playing if blink => "-.-",
        Mood::Playing => "o.o",
        Mood::Loading => "O.O",
        Mood::Paused => "u.u",
        Mood::Idle => "-.-",
    };
    let rows = cat_rows(eyes);

    let cat_w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0) as u16;
    let cat_h = rows.len() as u16;

    // A slow breath: lift the whole cat by one row, alternating — quicker when
    // it's awake and listening, slower when it sleeps.
    let breath = match mood {
        Mood::Playing => 8,
        Mood::Loading => 6,
        _ => 16,
    };
    let bob = ((tick / breath) % 2) as u16;

    // Anchor the cat a row off the floor, then let the breath lift it.
    let cat_x = area.x + area.width.saturating_sub(cat_w) / 2;
    let cat_top = area
        .bottom()
        .saturating_sub(cat_h + 1)
        .saturating_sub(bob)
        .max(area.y);
    let cat_center = cat_x + cat_w / 2;

    // The cat. Its eyes glow (highlight); the rest is the soft accent.
    {
        let buf = frame.buffer_mut();
        for (row, line) in rows.iter().enumerate() {
            let y = cat_top + row as u16;
            for (i, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let glow = row == 1 && !matches!(ch, '(' | ')');
                let color = if glow { theme.highlight } else { theme.accent };
                put(buf, area, cat_x + i as u16, y, ch, color);
            }
        }

        // A tail that swishes gently to the right of the cat's hindquarters.
        // Awake it sweeps through three positions; asleep it barely stirs.
        let swish_div = if matches!(mood, Mood::Playing | Mood::Loading) { 4 } else { 12 };
        let phase = (tick / swish_div) % 4;
        let (dx, tail) = match phase {
            0 => (0u16, '~'),
            1 => (1, '⌒'),
            2 => (2, '~'),
            _ => (1, '⌒'),
        };
        let tail_x = cat_x + cat_w + dx;
        let tail_y = cat_top + cat_h - 1;
        put(buf, area, tail_x, tail_y, tail, theme.secondary);
    }

    // The space above the cat the floaters rise through.
    let span = cat_top.saturating_sub(area.y);
    if span == 0 {
        return;
    }

    let (glyphs, speed_div, count): (&[char], u64, u64) = match mood {
        Mood::Playing => (&['♪', '♫', '♩', '♬'], 2, (area.width / 6).clamp(2, 5) as u64),
        Mood::Loading => (&['·', '?'], 3, 2),
        Mood::Paused => (&['z'], 5, 3),
        Mood::Idle => (&['z'], 7, 1),
    };
    let phase = tick as f32 * 0.1;

    let buf = frame.buffer_mut();
    for stream in 0..count {
        let seed = hash(stream + 1);
        let speed = 1 + seed % 2;
        // How far up this floater has risen, wrapping back to the cat at the top.
        let rise = ((tick / speed_div) * speed + seed % span as u64) % span as u64;
        let y = cat_top - 1 - rise as u16;

        // Notes sway side to side; sleepy `z`s and curious dots drift up.
        let drift = match mood {
            Mood::Playing => (phase * 0.6 + seed as f32).sin().round() as i32,
            _ => rise as i32 / 2,
        };
        let nx = cat_center as i32 + OFFSETS[stream as usize % OFFSETS.len()] + drift;
        if nx < 0 {
            continue;
        }

        let glyph = glyphs[seed as usize % glyphs.len()];

        // Bright and themed near the cat, fading toward the background as it rises.
        let base = match mood {
            Mood::Playing => ui::level_color(theme, (seed % 100) as f32 / 100.0),
            Mood::Loading => theme.highlight,
            _ => utils::lerp_color(theme.background, theme.muted, 0.55),
        };
        let fade = (rise as f32 / span as f32 * 0.85).min(0.85);
        let color = utils::lerp_color(base, theme.background, fade);

        put(buf, area, nx as u16, y, glyph, color);
    }
}

/// Draw the soft equalizer baseline: a row of block glyphs along the foot of the
/// band, each rising and falling on a gentle wave. It is livelier while a track
/// plays and almost flat when nothing is. Coloured by the theme spectrum and
/// faded toward the background so it never shouts.
fn draw_equalizer(frame: &mut Frame, area: Rect, theme: &crate::theme::Theme, mood: Mood, tick: u64) {
    if area.height < 2 {
        return;
    }
    let base_y = area.bottom().saturating_sub(1);

    // How fast the wave travels, and how tall it gets, by mood.
    let (speed, amplitude) = match mood {
        Mood::Playing => (0.28_f32, 1.0_f32),
        Mood::Loading => (0.20, 0.6),
        Mood::Paused => (0.10, 0.35),
        Mood::Idle => (0.05, 0.18),
    };
    let t = tick as f32;

    let buf = frame.buffer_mut();
    for x in area.x..area.right() {
        // Two summed sines give a softly irregular, organic sway.
        let fx = x as f32;
        let wave = ((fx * 0.45 + t * speed).sin() + (fx * 0.21 - t * speed * 0.6).sin()) * 0.25
            + 0.5;
        let level = (wave * amplitude).clamp(0.0, 1.0);

        let idx = (level * (BARS.len() - 1) as f32).round() as usize;
        let glyph = BARS[idx.min(BARS.len() - 1)];
        if glyph == ' ' {
            continue;
        }

        // Tall bars glow warmer; the whole field is softened toward the backdrop.
        let base = ui::level_color(theme, level);
        let color = utils::lerp_color(base, theme.background, 0.45);
        put(buf, area, x, base_y, glyph, color);
    }
}

/// Set one cell, but only if it falls inside `area` (so we never bleed into the
/// track info or the progress line above us).
fn put(buf: &mut Buffer, area: Rect, x: u16, y: u16, ch: char, color: Color) {
    if x < area.x || x >= area.right() || y < area.y || y >= area.bottom() {
        return;
    }
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_char(ch).set_style(Style::new().fg(color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cat_rows_are_uniform_width() {
        let rows = cat_rows("o.o");
        let widths: Vec<usize> = rows.iter().map(|r| r.chars().count()).collect();
        assert_eq!(widths, vec![7, 7, 7]);
        assert!(rows[1].contains("o.o")); // the eyes land in the face row
    }

    #[test]
    fn eyes_swap_with_mood() {
        assert!(cat_rows("u.u")[1].contains("u.u"));
        assert!(cat_rows("-.-")[1].contains("-.-"));
        assert!(cat_rows("O.O")[1].contains("O.O"));
    }
}
