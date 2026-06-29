//! The application runtime: state, the event loop, and event handling.
//!
//! Everything flows through a single unbounded channel of [`AppEvent`]s, fed by
//! four sources — a blocking input thread, a gentle ticker, the mpv reader, and
//! spawned search/resolve tasks. The loop draws, then waits for the next event,
//! then mutates state. Because every sender is unbounded, nothing in here ever
//! awaits to enqueue, which keeps handling synchronous and simple.
//!
//! Action handling (key → effect) lives in the sibling [`dispatch`] module.

mod dispatch;

use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::widgets::ListState;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::config::Config;
use crate::database::Database;
use crate::discord::Discord;
use crate::models::Track;
use crate::player::{Player, PlayerEvent};
use crate::theme::Theme;
use crate::widgets::input::Input;
use crate::{commands, ui, utils, youtube};

/// The animation tick period. ~10 Hz keeps the visualizer and idle rain smooth
/// while staying light. The windows below are expressed in ticks derived from
/// it, so they hold their wall-clock length if the rate ever changes.
const TICK_MS: u64 = 100;
/// Ticks per second, derived from [`TICK_MS`].
const TICKS_PER_SEC: u64 = 1000 / TICK_MS;
/// How long a status line lingers before it fades (~4 seconds).
const STATUS_TTL_TICKS: u64 = TICKS_PER_SEC * 4;
/// How long after the last keystroke a debounced search fires (~0.5 seconds).
const SEARCH_DEBOUNCE_TICKS: u64 = TICKS_PER_SEC / 2;

/// The screens of Seion. `Splash` is the opening breath; the rest are reachable
/// by keyboard. Lyrics is opened from Now Playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Splash,
    Home,
    Search,
    Library,
    Liked,
    History,
    Playlists,
    Downloads,
    Queue,
    NowPlaying,
    Lyrics,
    Settings,
}

impl View {
    /// A quiet, lowercase title for the header.
    pub fn title(self) -> &'static str {
        match self {
            View::Splash => "",
            View::Home => "home",
            View::Search => "search",
            View::Library => "library",
            View::Liked => "liked songs",
            View::History => "history",
            View::Playlists => "playlists",
            View::Downloads => "downloads",
            View::Queue => "queue",
            View::NowPlaying => "now playing",
            View::Lyrics => "lyrics",
            View::Settings => "settings",
        }
    }
}

/// Overlays float above the current view and capture all input until dismissed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    /// The keybinding cheatsheet.
    Help,
}

/// A transient one-line message shown in the player bar, auto-clearing.
pub struct Status {
    /// The message text.
    pub text: String,
    /// Whether it is an error (rendered in the clay colour).
    pub error: bool,
    /// The tick at which it appeared, for expiry.
    created_tick: u64,
}

/// The library hub's menu, mapping a label to the view it opens.
pub const LIBRARY_ITEMS: &[(&str, View)] = &[
    ("liked songs", View::Liked),
    ("history", View::History),
    ("playlists", View::Playlists),
    ("downloads", View::Downloads),
];

/// The rows of the settings screen, in order.
pub const SETTINGS_ITEMS: &[&str] = &[
    "theme",
    "idle rain",
    "visualizer",
    "discord presence",
    "daily quote",
    "audio backend",
    "search results",
];

/// Everything that can happen, funnelled onto one channel.
pub enum AppEvent {
    /// A terminal input event (already filtered to key-presses + resizes).
    Input(Event),
    /// Something from the player / resolver.
    Player(PlayerEvent),
    /// A search finished (tagged with its sequence number to drop stale ones).
    SearchDone {
        seq: u64,
        query: String,
        result: Result<Vec<Track>, String>,
    },
    /// A background task wants to show a status message.
    Status { text: String, error: bool },
    /// A periodic tick (~4 Hz) for animations and debounced search.
    Tick,
}

/// The whole application state.
pub struct App {
    /// Set to leave the run loop.
    pub should_quit: bool,
    /// User configuration.
    pub config: Config,
    /// The active colour palette.
    pub theme: Theme,
    /// Local persistence.
    pub db: Database,
    /// Playback controller.
    pub player: Player,
    /// Discord Rich Presence (dormant unless configured).
    pub discord: Discord,
    /// For spawned tasks to report back.
    tx: UnboundedSender<AppEvent>,

    /// The current screen.
    pub view: View,
    /// Where `Esc` returns to.
    pub previous_view: View,
    /// The active overlay, if any.
    pub overlay: Option<Overlay>,
    /// Whether the search box is capturing text.
    pub editing: bool,

    /// The search box contents.
    pub search: Input,
    /// Current search results.
    pub results: Vec<Track>,
    /// Selection within the results.
    pub results_state: ListState,
    /// Whether a search is in flight.
    pub searching: bool,
    search_seq: u64,
    search_dirty: bool,
    search_dirty_at: u64,

    /// Selection within the library hub menu.
    pub library_state: ListState,
    /// Liked tracks.
    pub liked: Vec<Track>,
    /// Selection within liked.
    pub liked_state: ListState,
    /// Recently played.
    pub history: Vec<Track>,
    /// Selection within history (and the home screen).
    pub history_state: ListState,
    /// Downloaded tracks.
    pub downloads: Vec<Track>,
    /// Selection within downloads.
    pub downloads_state: ListState,
    /// Selection within the queue.
    pub queue_state: ListState,
    /// Selection within settings.
    pub settings_state: ListState,

    /// Transient status line.
    pub status: Option<Status>,
    /// Monotonic tick counter.
    pub tick_count: u64,
    /// Frames since the splash appeared (drives the fade-in).
    pub splash_frame: u16,
    /// Hide everything but the current track.
    pub focus_mode: bool,
    /// Fullscreen calm: art + lyrics only.
    pub zen_mode: bool,
    /// Today's haiku.
    pub quote: (&'static str, &'static str),
}

impl App {
    /// Build the app: load config, open the database, launch the player, and
    /// prime the library lists. Returns the app and the event receiver.
    pub async fn new() -> anyhow::Result<(Self, UnboundedReceiver<AppEvent>)> {
        let config = Config::load();
        let theme = Theme::from_name(&config.theme);
        let db = Database::open().or_else(|_| Database::in_memory())?;
        let (tx, rx) = mpsc::unbounded_channel();
        let player = Player::launch(&config, tx.clone());
        let discord = Discord::launch(&config);

        let liked = db.liked().unwrap_or_default();
        let history = db.history(100).unwrap_or_default();

        let app = App {
            should_quit: false,
            config,
            theme,
            db,
            player,
            discord,
            tx,
            view: View::Splash,
            previous_view: View::Home,
            overlay: None,
            editing: false,
            search: Input::default(),
            results: Vec::new(),
            results_state: ListState::default(),
            searching: false,
            search_seq: 0,
            search_dirty: false,
            search_dirty_at: 0,
            library_state: selected_at_zero(),
            liked_state: select_if_any(&liked),
            liked,
            history_state: select_if_any(&history),
            history,
            downloads: Vec::new(),
            downloads_state: ListState::default(),
            queue_state: ListState::default(),
            settings_state: selected_at_zero(),
            status: None,
            tick_count: 0,
            splash_frame: 0,
            focus_mode: false,
            zen_mode: false,
            quote: utils::daily_quote(),
        };

        Ok((app, rx))
    }

    /// Run the event loop until quit. Consumes the app.
    pub async fn run(
        mut self,
        mut terminal: DefaultTerminal,
        mut rx: UnboundedReceiver<AppEvent>,
    ) -> anyhow::Result<()> {
        spawn_input(self.tx.clone());
        spawn_ticker(self.tx.clone());

        loop {
            terminal.draw(|frame| ui::render(frame, &mut self))?;
            if self.should_quit {
                break;
            }
            match rx.recv().await {
                Some(event) => self.handle_event(event),
                None => break,
            }
            // Reconcile Discord with playback after each event. The handle
            // de-dupes, so this is a cheap no-op unless something changed.
            self.discord.sync(&self.player.state);
        }

        // Persist the final volume (and anything else in memory) on the way out.
        self.config.volume = self.player.state.volume;
        let _ = self.config.save();
        self.discord.shutdown();
        self.player.shutdown();
        Ok(())
    }

    /// Route an event to the right handler.
    fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Input(ev) => self.handle_input(ev),
            AppEvent::Player(pe) => self.handle_player(pe),
            AppEvent::SearchDone { seq, query, result } => self.handle_search_done(seq, query, result),
            AppEvent::Status { text, error } => self.set_status(text, error),
            AppEvent::Tick => self.tick(),
        }
    }

    /// Handle a terminal input event.
    fn handle_input(&mut self, event: Event) {
        // Resizes (and anything else) simply provoke the redraw that the loop
        // does each iteration; only keys carry intent.
        let Event::Key(key) = event else {
            return;
        };

        // The splash waits for a single breath, then opens.
        if self.view == View::Splash {
            self.leave_splash();
            return;
        }

        // An overlay swallows the next key to dismiss itself.
        if self.overlay.is_some() {
            self.overlay = None;
            return;
        }

        if let Some(action) = commands::resolve(key, self.editing) {
            self.dispatch(action);
        }
    }

    /// Fold a player event into state and react to anything notable.
    fn handle_player(&mut self, event: PlayerEvent) {
        use crate::player::Notice;
        match self.player.handle_event(event) {
            Notice::Started(track) => {
                let _ = self.db.record_play(&track);
                // Keep the recent list fresh if we're looking at it — but hold
                // the highlight on whatever track it was on, since the just-
                // played track jumps to the top and shifts everything down.
                if matches!(self.view, View::History | View::Home) {
                    let focused_id = self
                        .history_state
                        .selected()
                        .and_then(|i| self.history.get(i))
                        .map(|t| t.id.clone());
                    self.history = self.db.history(100).unwrap_or_default();
                    let index = focused_id
                        .and_then(|id| self.history.iter().position(|t| t.id == id))
                        .or_else(|| (!self.history.is_empty()).then_some(0));
                    self.history_state.select(index);
                }
            }
            Notice::Error(message) => self.set_status(message, true),
            Notice::Nothing => {}
        }
    }

    /// Apply a finished search, ignoring stale results.
    fn handle_search_done(&mut self, seq: u64, _query: String, result: Result<Vec<Track>, String>) {
        if seq != self.search_seq {
            return; // a newer search superseded this one
        }
        self.searching = false;
        match result {
            Ok(tracks) => {
                self.results = tracks;
                self.results_state.select(if self.results.is_empty() {
                    None
                } else {
                    Some(0)
                });
            }
            Err(e) => self.set_status(format!("search failed — {e}"), true),
        }
    }

    /// Periodic housekeeping: animations, status expiry, debounced search.
    fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);

        if self.view == View::Splash {
            self.splash_frame = self.splash_frame.saturating_add(1);
        }

        // Let an old status fade after roughly four seconds.
        if let Some(status) = &self.status
            && self.tick_count.saturating_sub(status.created_tick) > STATUS_TTL_TICKS
        {
            self.status = None;
        }

        // Fire a debounced search a short while after the last keystroke.
        if self.search_dirty
            && self.tick_count.saturating_sub(self.search_dirty_at) >= SEARCH_DEBOUNCE_TICKS
        {
            self.search_dirty = false;
            self.run_search();
        }
    }

    /// Leave the splash and settle into the home screen.
    fn leave_splash(&mut self) {
        self.view = View::Home;
        self.previous_view = View::Home;
    }

    /// Note that the search text changed, scheduling a debounced search.
    fn mark_search_dirty(&mut self) {
        self.search_dirty = true;
        self.search_dirty_at = self.tick_count;
    }

    /// Kick off a search for the current box contents.
    fn run_search(&mut self) {
        // Whatever scheduled this (debounce, Enter), the pending search is now
        // being serviced — clear the flag so it can't fire again redundantly.
        self.search_dirty = false;
        let query = self.search.text().trim().to_string();
        if query.is_empty() {
            self.results.clear();
            self.results_state.select(None);
            self.searching = false;
            return;
        }
        self.search_seq += 1;
        let seq = self.search_seq;
        self.searching = true;

        let tx = self.tx.clone();
        let ytdlp = self.config.ytdlp_path.clone();
        let limit = self.config.search_limit;
        tokio::spawn(async move {
            let result = youtube::search(&ytdlp, &query, limit)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::SearchDone { seq, query, result });
        });
    }

    /// Set the transient status line.
    fn set_status(&mut self, text: impl Into<String>, error: bool) {
        self.status = Some(Status {
            text: text.into(),
            error,
            created_tick: self.tick_count,
        });
    }
}

/// A `ListState` with the first row selected.
fn selected_at_zero() -> ListState {
    let mut state = ListState::default();
    state.select(Some(0));
    state
}

/// A `ListState` selecting row 0 only if the slice is non-empty.
fn select_if_any<T>(items: &[T]) -> ListState {
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(0));
    }
    state
}

/// Spawn the blocking input thread.
///
/// crossterm's `read()` blocks, so it lives on its own OS thread and forwards
/// events through the channel. We forward only key **presses** — on Windows
/// crossterm also reports key releases/repeats, and handling those would make
/// every keystroke fire twice (the classic Windows TUI bug).
fn spawn_input(tx: UnboundedSender<AppEvent>) {
    std::thread::spawn(move || {
        loop {
            match event::poll(Duration::from_millis(100)) {
                Ok(true) => match event::read() {
                    Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                        if tx.send(AppEvent::Input(Event::Key(key))).is_err() {
                            break;
                        }
                    }
                    Ok(Event::Resize(w, h)) => {
                        if tx.send(AppEvent::Input(Event::Resize(w, h))).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => break,
                },
                Ok(false) => {}
                Err(_) => break,
            }
        }
    });
}

/// Spawn the ticker that drives animations and the search debounce.
fn spawn_ticker(tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(TICK_MS));
        loop {
            interval.tick().await;
            if tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });
}
