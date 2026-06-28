//! Headless preview (`seion --preview`).
//!
//! Renders representative screens through the real [`crate::ui::render`] path
//! into an in-memory backend and prints them as text. Useful where a full TUI
//! can't run (a pipe, CI, a quick look) — there is no colour or animation here,
//! but the layout, glyphs and typography are exactly what the terminal shows.

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use crate::app::{App, View};
use crate::models::Track;

/// Render a handful of screens, populated with sample tracks, to stdout.
pub fn run(mut app: App) {
    // A few calm sample tracks so the screens aren't empty.
    let samples = vec![
        Track::new("a", "夜に駆ける", "YOASOBI", Some(261)),
        Track::new("b", "rainy cafe jazz", "nujabes radio", Some(342)),
        Track::new("c", "nujabes playlist", "hydeout productions", None),
        Track::new("d", "lamp", "ランプ", Some(247)),
        Track::new("e", "ichiko aoba", "windswept adan", Some(198)),
    ];

    app.results = samples.clone();
    app.results_state.select(Some(1));
    app.history = samples.clone();
    app.history_state.select(Some(0));
    for c in "lofi jazz".chars() {
        app.search.insert(c);
    }

    // Pretend something calm is playing, partway through.
    app.player.queue = samples.clone();
    app.player.queue_pos = Some(1);
    app.player.state.current = Some(samples[1].clone());
    app.player.state.position = 137.0;
    app.player.state.duration = 342.0;

    let screens: &[(&str, View, u16)] = &[
        ("splash", View::Splash, 8),
        ("search", View::Search, 0),
        ("home", View::Home, 0),
        ("now playing", View::NowPlaying, 0),
        ("queue", View::Queue, 0),
        ("settings", View::Settings, 0),
    ];

    println!("静音 · seion — static preview (no colour / animation; the live app has both)\n");
    for (name, view, splash_frame) in screens {
        app.view = *view;
        app.editing = false;
        app.splash_frame = *splash_frame;

        // A fresh backend per screen, so each renders from a blank buffer
        // exactly as a real terminal shows it.
        let mut terminal = Terminal::new(TestBackend::new(112, 26)).expect("test backend");
        terminal
            .draw(|frame| crate::ui::render(frame, &mut app))
            .expect("render");

        println!("┌─ {name} {}", "─".repeat(70usize.saturating_sub(name.len())));
        print!("{}", dump(&terminal));
        println!("└{}\n", "─".repeat(76));
    }
}

/// Convert the rendered buffer into plain text, one row per line.
fn dump(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let area = buffer.area;
    let mut out = String::new();
    for y in 0..area.height {
        out.push_str("│ ");
        for x in 0..area.width {
            let symbol = buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" ");
            out.push_str(if symbol.is_empty() { " " } else { symbol });
        }
        out.push('\n');
    }
    out
}
