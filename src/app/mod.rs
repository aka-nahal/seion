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
use crate::models::{Playlist, Track};
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
/// How many tracks to pull when opening a playlist (playlists can be enormous).
pub const PLAYLIST_LOAD_LIMIT: usize = 100;

/// What the search box is currently looking for — plain tracks, or playlists
/// (entered with the `/playlist` command prefix).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchKind {
    /// Ordinary track search.
    #[default]
    Tracks,
    /// Playlist search (`/playlist <query>`).
    Playlists,
}

/// A YouTube link recognised in the search box, ready to load on Enter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PendingLink {
    /// A playlist URL or id — Enter loads the whole playlist.
    Playlist(String),
    /// A single video URL — Enter plays just that track.
    Video(String),
}

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
    "progress time",
    "truecolor",
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
    /// A playlist search finished (same sequence-number staleness guard).
    PlaylistsDone {
        seq: u64,
        result: Result<Vec<Playlist>, String>,
    },
    /// A playlist's tracks finished loading — play them, or append if `append`.
    PlaylistOpened {
        title: String,
        append: bool,
        result: Result<Vec<Track>, String>,
    },
    /// A single pasted video link finished resolving — play it.
    TrackResolved { result: Result<Track, String> },
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
    /// What the search box is looking for (tracks or playlists).
    pub search_kind: SearchKind,
    /// Current track search results.
    pub results: Vec<Track>,
    /// Current playlist search results (shown when `search_kind` is `Playlists`).
    pub playlists: Vec<Playlist>,
    /// Selection within the results (shared by both result kinds — only one is
    /// shown at a time).
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
        let theme = Theme::from_name(&config.theme).adapt(config.truecolor);
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
            search_kind: SearchKind::default(),
            results: Vec::new(),
            playlists: Vec::new(),
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
            AppEvent::PlaylistsDone { seq, result } => self.handle_playlists_done(seq, result),
            AppEvent::PlaylistOpened { title, append, result } => {
                self.handle_playlist_opened(title, append, result)
            }
            AppEvent::TrackResolved { result } => self.handle_track_resolved(result),
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

    /// Apply a finished playlist search, ignoring stale results.
    fn handle_playlists_done(&mut self, seq: u64, result: Result<Vec<Playlist>, String>) {
        if seq != self.search_seq {
            return; // superseded by a newer search
        }
        self.searching = false;
        match result {
            Ok(playlists) => {
                self.playlists = playlists;
                self.results_state.select(if self.playlists.is_empty() {
                    None
                } else {
                    Some(0)
                });
            }
            Err(e) => self.set_status(format!("playlist search failed — {e}"), true),
        }
    }

    /// A playlist's tracks arrived — play them, or append to the queue.
    fn handle_playlist_opened(
        &mut self,
        title: String,
        append: bool,
        result: Result<Vec<Track>, String>,
    ) {
        match result {
            Ok(tracks) if !tracks.is_empty() => {
                let count = tracks.len();
                let name = if title.is_empty() {
                    "playlist".to_string()
                } else {
                    utils::truncate(&title, 40)
                };
                if append {
                    self.player.enqueue_all(tracks);
                    self.set_status(format!("queued {name} — {count} tracks"), false);
                } else {
                    self.player.play_from(tracks, 0);
                    self.set_status(format!("playing {name} — {count} tracks"), false);
                }
            }
            Ok(_) => self.set_status("that playlist was empty", false),
            Err(e) => self.set_status(format!("could not open playlist — {e}"), true),
        }
    }

    /// A pasted single-video link resolved — play it as a one-track queue.
    fn handle_track_resolved(&mut self, result: Result<Track, String>) {
        match result {
            Ok(track) => {
                let title = track.title.clone();
                self.player.play_from(vec![track], 0);
                self.set_status(format!("playing — {}", utils::truncate(&title, 40)), false);
            }
            Err(e) => self.set_status(format!("could not load track — {e}"), true),
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

    /// Kick off a search for the current box contents. A `/playlist <query>`
    /// prefix searches playlists instead of tracks.
    fn run_search(&mut self) {
        // Whatever scheduled this (debounce, Enter), the pending search is now
        // being serviced — clear the flag so it can't fire again redundantly.
        self.search_dirty = false;

        let raw = self.search.text().trim().to_string();
        let (kind, query) = match parse_playlist_command(&raw) {
            Some(rest) => (SearchKind::Playlists, rest),
            None => (SearchKind::Tracks, raw),
        };
        self.search_kind = kind;

        if query.is_empty() {
            self.results.clear();
            self.playlists.clear();
            self.results_state.select(None);
            self.searching = false;
            return;
        }

        // A pasted link (playlist or video) isn't a search term — don't waste a
        // yt-dlp round trip on it; the search view prompts to load it with Enter.
        if youtube::playlist_id_from(&query).is_some() || youtube::video_id_from(&query).is_some() {
            self.searching = false;
            return;
        }

        self.search_seq += 1;
        let seq = self.search_seq;
        self.searching = true;

        let tx = self.tx.clone();
        let ytdlp = self.config.ytdlp_path.clone();
        let limit = self.config.search_limit;
        match kind {
            SearchKind::Tracks => {
                tokio::spawn(async move {
                    let result = youtube::search(&ytdlp, &query, limit)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::SearchDone { seq, query, result });
                });
            }
            SearchKind::Playlists => {
                tokio::spawn(async move {
                    let result = youtube::search_playlists(&ytdlp, &query, limit)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::PlaylistsDone { seq, result });
                });
            }
        }
    }

    /// If the search box holds a YouTube link, what Enter would load — a playlist
    /// or a single video. Lets the search view show a load prompt and lets submit
    /// act on it. A playlist takes precedence (a watch link can carry both).
    pub fn pending_link(&self) -> Option<PendingLink> {
        let raw = self.search.text().trim();
        let candidate = parse_playlist_command(raw).unwrap_or_else(|| raw.to_string());
        if let Some(id) = youtube::playlist_id_from(&candidate) {
            return Some(PendingLink::Playlist(id));
        }
        youtube::video_id_from(&candidate).map(PendingLink::Video)
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

/// Recognise the `/playlist <query>` search command, returning the trimmed
/// query (which may be empty for a bare `/playlist`). Returns `None` for any
/// other text, so ordinary searches are untouched.
fn parse_playlist_command(text: &str) -> Option<String> {
    let rest = text.strip_prefix("/playlist")?;
    // Require the prefix to end at a word boundary so "/playlists" (a literal
    // search) isn't swallowed, while a bare "/playlist" still counts.
    if rest.is_empty() || rest.starts_with(char::is_whitespace) {
        Some(rest.trim().to_string())
    } else {
        None
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

#[cfg(test)]
mod tests {
    use super::parse_playlist_command;

    #[test]
    fn playlist_command_extracts_query() {
        assert_eq!(parse_playlist_command("/playlist lofi"), Some("lofi".to_string()));
        assert_eq!(
            parse_playlist_command("/playlist  rainy   jazz  "),
            Some("rainy   jazz".to_string())
        );
        assert_eq!(parse_playlist_command("/playlist"), Some(String::new()));
    }

    #[test]
    fn non_playlist_text_is_left_alone() {
        assert_eq!(parse_playlist_command("lofi beats"), None);
        assert_eq!(parse_playlist_command("/play something"), None);
        // A word that merely starts with the prefix must not be swallowed.
        assert_eq!(parse_playlist_command("/playlists of mine"), None);
    }
}
