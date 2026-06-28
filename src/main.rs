//! 静音 (Seion) — "quiet sound".
//!
//! An ultra-lightweight, distraction-free terminal client for YouTube Music.
//! It aims to feel like a quiet tea house on a rainy evening: keyboard-first,
//! softly coloured, and free of visual noise.
//!
//! ```text
//!         静音
//!    ─────────────────
//!       peaceful music
//! ```
//!
//! Architecture (everything is modular and async):
//!
//! ```text
//!   ui  ←  app (event loop)  →  player  →  mpv (ipc)
//!                │                 │
//!                ├── youtube (yt-dlp search / stream resolution)
//!                ├── database (sqlite: liked, history)
//!                ├── config (toml)  ·  cache (stream urls)
//!                └── theme · widgets · commands · utils
//! ```

mod app;
mod cache;
mod commands;
mod config;
mod database;
mod downloads;
mod lyrics;
mod models;
mod player;
mod preview;
mod theme;
mod ui;
mod utils;
mod widgets;
mod youtube;

use app::App;

/// A single-threaded async runtime is plenty: the work is I/O-bound (a few
/// pipes and subprocesses) and this keeps memory and startup small.
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // Build everything (config, database, player) before we touch the terminal,
    // so any early error prints normally rather than into the alternate screen.
    let (app, events) = App::new().await?;

    // `--preview` renders the screens to text and exits — handy where a real
    // TUI can't run (a pipe, CI, or just a quick look).
    if std::env::args().any(|arg| arg == "--preview") {
        preview::run(app);
        return Ok(());
    }

    // `ratatui::init()` enables raw mode + the alternate screen and installs a
    // panic hook that restores the terminal before the process exits — which
    // still runs under our `panic = "abort"` profile.
    let terminal = ratatui::init();
    let result = app.run(terminal, events).await;
    ratatui::restore();

    result
}
