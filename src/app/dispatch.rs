//! Action handling: turning a resolved [`Action`] into a state change.
//!
//! This is the single mutator. It is kept cheap and synchronous — anything slow
//! (search, download) is spawned and reports back through the event channel.

use ratatui::widgets::ListState;

use super::{
    App, AppEvent, LIBRARY_ITEMS, Overlay, PLAYLIST_LOAD_LIMIT, PendingLink, SETTINGS_ITEMS,
    SearchKind, View,
};
use crate::commands::Action;
use crate::models::Track;
use crate::{downloads, utils, youtube};

impl App {
    /// Apply an action to the application state.
    pub(super) fn dispatch(&mut self, action: Action) {
        use Action::*;
        match action {
            Quit => self.should_quit = true,
            Back => self.go_back(),
            OpenSearch => self.open_search(),
            Home => self.goto(View::Home),
            GotoLibrary => self.goto(View::Library),
            GotoQueue => self.goto(View::Queue),
            GotoNowPlaying => self.goto(View::NowPlaying),
            GotoSettings => self.goto(View::Settings),
            GotoPlaylists => self.goto(View::Playlists),
            Help => self.overlay = Some(Overlay::Help),

            MoveUp => self.move_selection(-1),
            MoveDown => self.move_selection(1),
            PageUp => self.move_selection(-10),
            PageDown => self.move_selection(10),
            SelectTop => self.select_edge(true),
            SelectBottom => self.select_edge(false),
            Activate => self.activate(),

            TogglePlay => self.player.toggle_pause(),
            Stop => {
                self.player.stop();
                self.set_status("stopped", false);
            }
            Next => self.player.next(),
            Previous => self.player.previous(),
            SeekForward => self.player.seek_relative(5.0),
            SeekBackward => self.player.seek_relative(-5.0),
            VolumeUp => {
                self.player.volume_up();
                self.persist_volume();
            }
            VolumeDown => {
                self.player.volume_down();
                self.persist_volume();
            }
            ToggleMute => {
                self.player.toggle_mute();
                self.persist_volume();
                let muted = self.player.is_muted();
                self.set_status(if muted { "muted" } else { "unmuted" }, false);
            }
            ToggleLike => self.toggle_like(),
            Enqueue => self.enqueue_action(),
            CycleRepeat => {
                self.player.cycle_repeat();
                let glyph = self.player.repeat.glyph();
                self.set_status(format!("repeat — {glyph}"), false);
            }
            ToggleShuffle => {
                self.player.toggle_shuffle();
                let on = self.player.shuffle;
                self.set_status(if on { "shuffle on" } else { "shuffle off" }, false);
            }
            Download => self.download_selected(),

            FocusMode => {
                self.focus_mode = !self.focus_mode;
                if self.focus_mode {
                    self.zen_mode = false;
                }
            }
            ZenMode => {
                self.zen_mode = !self.zen_mode;
                if self.zen_mode {
                    self.focus_mode = false;
                }
            }
            ToggleRain => {
                self.config.rain_on_idle = !self.config.rain_on_idle;
                let _ = self.config.save();
                let on = self.config.rain_on_idle;
                self.set_status(if on { "rain on" } else { "rain off" }, false);
            }
            ToggleVisualizer => {
                self.config.visualizer = !self.config.visualizer;
                let _ = self.config.save();
                let on = self.config.visualizer;
                self.set_status(if on { "visualizer on" } else { "visualizer off" }, false);
            }
            CycleTheme => {
                self.theme = self.theme.next().adapt(self.config.truecolor);
                self.config.theme = self.theme.key();
                let _ = self.config.save();
                self.set_status(format!("theme — {}", self.theme.name), false);
            }

            InputChar(c) => {
                self.search.insert(c);
                self.mark_search_dirty();
            }
            InputBackspace => {
                self.search.backspace();
                self.mark_search_dirty();
            }
            InputLeft => self.search.left(),
            InputRight => self.search.right(),
            InputHome => self.search.home(),
            InputEnd => self.search.end(),
            InputSubmit => self.submit_search(),
            InputCancel => self.cancel_search(),
        }
    }

    // --- navigation --------------------------------------------------------

    /// Focus the search box, remembering where we came from.
    fn open_search(&mut self) {
        if self.view != View::Search {
            self.previous_view = self.view;
        }
        self.view = View::Search;
        self.editing = true;
    }

    /// Switch to a view, refreshing any data it shows.
    fn goto(&mut self, view: View) {
        if self.view != view {
            self.previous_view = self.view;
        }
        self.editing = false;
        self.overlay = None;
        self.view = view;

        match view {
            View::Liked => {
                self.liked = self.db.liked().unwrap_or_default();
                reselect(&mut self.liked_state, self.liked.len());
            }
            View::History | View::Home => {
                self.history = self.db.history(100).unwrap_or_default();
                reselect(&mut self.history_state, self.history.len());
            }
            View::Downloads => {
                self.downloads = downloads::list_tracks();
                reselect(&mut self.downloads_state, self.downloads.len());
            }
            View::Queue => {
                let len = self.player.queue.len();
                if len == 0 {
                    self.queue_state.select(None);
                } else {
                    let pos = self.player.queue_pos.unwrap_or(0).min(len - 1);
                    self.queue_state.select(Some(pos));
                }
            }
            _ => {}
        }
    }

    /// Go back: leave editing, close overlays, or return to the previous view.
    fn go_back(&mut self) {
        if self.editing {
            self.editing = false;
            return;
        }
        if self.overlay.is_some() {
            self.overlay = None;
            return;
        }
        let target = if self.previous_view == self.view {
            View::Home
        } else {
            self.previous_view
        };
        self.goto(target);
    }

    /// Move the selection in the active list by `delta`, clamped to its bounds.
    fn move_selection(&mut self, delta: i64) {
        if let Some((len, state)) = self.active_selection() {
            if len == 0 {
                return;
            }
            let current = state.selected().unwrap_or(0) as i64;
            let max = len as i64 - 1;
            let next = (current + delta).clamp(0, max) as usize;
            state.select(Some(next));
        }
    }

    /// Jump the active list's selection to the first (`top`) or last row.
    fn select_edge(&mut self, top: bool) {
        if let Some((len, state)) = self.active_selection() {
            if len == 0 {
                return;
            }
            state.select(Some(if top { 0 } else { len - 1 }));
        }
    }

    /// The list length and selection state for the current view, if it has one.
    fn active_selection(&mut self) -> Option<(usize, &mut ListState)> {
        let search_len = match self.search_kind {
            SearchKind::Tracks => self.results.len(),
            SearchKind::Playlists => self.playlists.len(),
        };
        match self.view {
            View::Search => Some((search_len, &mut self.results_state)),
            View::Liked => Some((self.liked.len(), &mut self.liked_state)),
            View::Home | View::History => Some((self.history.len(), &mut self.history_state)),
            View::Downloads => Some((self.downloads.len(), &mut self.downloads_state)),
            View::Queue => Some((self.player.queue.len(), &mut self.queue_state)),
            View::Library => Some((LIBRARY_ITEMS.len(), &mut self.library_state)),
            View::Settings => Some((SETTINGS_ITEMS.len(), &mut self.settings_state)),
            _ => None,
        }
    }

    /// The track under the cursor for the current view (or the playing track).
    pub(crate) fn selected_track(&self) -> Option<Track> {
        let pick = |list: &[Track], state: &ListState| {
            state.selected().and_then(|i| list.get(i).cloned())
        };
        match self.view {
            // A playlist row isn't a track — like/enqueue/download don't apply.
            View::Search if self.search_kind == SearchKind::Playlists => None,
            View::Search => pick(&self.results, &self.results_state),
            View::Liked => pick(&self.liked, &self.liked_state),
            View::Home | View::History => pick(&self.history, &self.history_state),
            View::Downloads => pick(&self.downloads, &self.downloads_state),
            View::Queue => pick(&self.player.queue, &self.queue_state),
            View::NowPlaying | View::Lyrics => self.player.state.current.clone(),
            _ => None,
        }
    }

    // --- the Enter key -----------------------------------------------------

    /// Act on the current selection.
    fn activate(&mut self) {
        match self.view {
            View::Search => match self.search_kind {
                SearchKind::Tracks => {
                    self.play_selected(self.results.clone(), self.results_state.selected())
                }
                SearchKind::Playlists => self.open_selected_playlist(false),
            },
            View::Liked => self.play_selected(self.liked.clone(), self.liked_state.selected()),
            View::Home | View::History => {
                self.play_selected(self.history.clone(), self.history_state.selected())
            }
            View::Downloads => {
                self.play_selected(self.downloads.clone(), self.downloads_state.selected())
            }
            View::Queue => {
                if let Some(i) = self.queue_state.selected() {
                    self.player.play_index(i);
                }
            }
            View::Library => {
                if let Some(i) = self.library_state.selected()
                    && let Some((_, view)) = LIBRARY_ITEMS.get(i)
                {
                    self.goto(*view);
                }
            }
            View::Settings => {
                if let Some(i) = self.settings_state.selected() {
                    self.cycle_setting(i);
                }
            }
            View::NowPlaying => self.goto(View::Lyrics),
            _ => {}
        }
    }

    /// Start playing `list` from `index` (within that list as the new queue).
    fn play_selected(&mut self, list: Vec<Track>, index: Option<usize>) {
        let Some(index) = index else {
            return;
        };
        if list.is_empty() {
            return;
        }
        let title = list
            .get(index)
            .map(|t| t.title.clone())
            .unwrap_or_default();
        self.player.play_from(list, index);
        self.set_status(format!("playing — {}", utils::truncate(&title, 40)), false);
    }

    /// Open the selected playlist search result (Enter plays, `a` appends).
    fn open_selected_playlist(&mut self, append: bool) {
        let Some(playlist) = self
            .results_state
            .selected()
            .and_then(|i| self.playlists.get(i).cloned())
        else {
            return;
        };
        self.open_playlist_by_id(playlist.id, playlist.title, append);
    }

    /// Fetch a playlist's tracks off-thread by its id, then play them — or append
    /// them to the queue when `append` — back in [`App::handle_playlist_opened`].
    /// `name_hint` is shown while loading and used as a fallback title.
    fn open_playlist_by_id(&mut self, id: String, name_hint: String, append: bool) {
        let verb = if append { "queuing" } else { "loading" };
        let loading = if name_hint.is_empty() {
            format!("{verb} playlist …")
        } else {
            format!("{verb} {} …", utils::truncate(&name_hint, 40))
        };
        self.set_status(loading, false);

        let tx = self.tx.clone();
        let ytdlp = self.config.ytdlp_path.clone();
        tokio::spawn(async move {
            let (title, result) = match youtube::playlist_tracks(&ytdlp, &id, PLAYLIST_LOAD_LIMIT).await {
                // Prefer the playlist's real title; fall back to the hint.
                Ok((name, tracks)) => (if name.is_empty() { name_hint } else { name }, Ok(tracks)),
                Err(e) => (name_hint, Err(e.to_string())),
            };
            let _ = tx.send(AppEvent::PlaylistOpened { title, append, result });
        });
    }

    /// Resolve a pasted single-video link off-thread, then play it.
    fn play_video_by_id(&mut self, id: String) {
        self.set_status("loading track …", false);
        let tx = self.tx.clone();
        let ytdlp = self.config.ytdlp_path.clone();
        tokio::spawn(async move {
            let result = youtube::fetch_track(&ytdlp, &id)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(AppEvent::TrackResolved { result });
        });
    }

    // --- track actions -----------------------------------------------------

    /// Like or unlike the focused (or playing) track.
    fn toggle_like(&mut self) {
        let Some(track) = self
            .selected_track()
            .or_else(|| self.player.state.current.clone())
        else {
            self.set_status("nothing to like", false);
            return;
        };

        match self.db.toggle_like(&track) {
            Ok(true) => self.set_status(format!("liked — {}", utils::truncate(&track.title, 40)), false),
            Ok(false) => self.set_status("unliked", false),
            Err(e) => self.set_status(format!("could not update — {e}"), true),
        }

        self.liked = self.db.liked().unwrap_or_default();
        reselect(&mut self.liked_state, self.liked.len());
    }

    /// Handle the "add to queue" key: a focused playlist result appends the whole
    /// playlist; otherwise the focused track is appended.
    fn enqueue_action(&mut self) {
        if self.view == View::Search && self.search_kind == SearchKind::Playlists {
            self.open_selected_playlist(true);
        } else {
            self.enqueue_selected();
        }
    }

    /// Add the focused track to the queue.
    fn enqueue_selected(&mut self) {
        let Some(track) = self.selected_track() else {
            self.set_status("nothing to add", false);
            return;
        };
        let title = track.title.clone();
        self.player.enqueue(track);
        self.set_status(format!("added — {}", utils::truncate(&title, 40)), false);
    }

    /// Download the focused (or playing) track for offline listening.
    fn download_selected(&mut self) {
        let Some(track) = self
            .selected_track()
            .or_else(|| self.player.state.current.clone())
        else {
            self.set_status("nothing to download", false);
            return;
        };
        if track.id.starts_with(downloads::FILE_SCHEME) {
            self.set_status("already downloaded", false);
            return;
        }

        let title = track.title.clone();
        self.set_status(format!("downloading — {}", utils::truncate(&title, 40)), false);

        let tx = self.tx.clone();
        let ytdlp = self.config.ytdlp_path.clone();
        tokio::spawn(async move {
            let (text, error) = match downloads::download(&ytdlp, &track).await {
                Ok(_) => (format!("downloaded — {title}"), false),
                Err(e) => (format!("download failed — {e}"), true),
            };
            let _ = tx.send(AppEvent::Status { text, error });
        });
    }

    /// Mirror the player's volume into config (in memory). The file is written
    /// once on exit, so holding `+`/`-` doesn't write to disk on every press.
    fn persist_volume(&mut self) {
        self.config.volume = self.player.state.volume;
    }

    // --- search editing ----------------------------------------------------

    /// Submit the search and move focus to the results. If the box holds a
    /// YouTube link (optionally after `/playlist`), load it directly instead of
    /// searching — a playlist loads & plays, a video plays just that track.
    fn submit_search(&mut self) {
        self.editing = false;
        match self.pending_link() {
            Some(PendingLink::Playlist(id)) => {
                self.search.clear();
                self.open_playlist_by_id(id, String::new(), false);
            }
            Some(PendingLink::Video(id)) => {
                self.search.clear();
                self.play_video_by_id(id);
            }
            None => self.run_search(),
        }
    }

    /// Cancel editing — go back if the box is empty, else just defocus it.
    fn cancel_search(&mut self) {
        self.editing = false;
        // Drop any pending debounced search so nothing fires after cancelling.
        self.search_dirty = false;
        if self.search.is_empty() {
            self.go_back();
        }
    }

    // --- settings ----------------------------------------------------------

    /// Cycle the value of the settings row at `index`.
    fn cycle_setting(&mut self, index: usize) {
        match index {
            0 => {
                self.theme = self.theme.next().adapt(self.config.truecolor);
                self.config.theme = self.theme.key();
                let _ = self.config.save();
                self.set_status(format!("theme — {}", self.theme.name), false);
            }
            1 => {
                self.config.rain_on_idle = !self.config.rain_on_idle;
                let _ = self.config.save();
                let on = self.config.rain_on_idle;
                self.set_status(if on { "idle rain on" } else { "idle rain off" }, false);
            }
            2 => {
                self.config.visualizer = !self.config.visualizer;
                let _ = self.config.save();
                let on = self.config.visualizer;
                self.set_status(if on { "visualizer on" } else { "visualizer off" }, false);
            }
            3 => {
                let on = !self.config.discord_presence;
                self.config.discord_presence = on;
                let _ = self.config.save();
                self.discord.set_enabled(on);
                if on {
                    self.discord.sync(&self.player.state);
                } else {
                    self.discord.clear();
                }
                let note = if !self.discord.is_configured() {
                    "discord presence — set discord_client_id in config.toml"
                } else if on {
                    "discord presence on"
                } else {
                    "discord presence off"
                };
                self.set_status(note, false);
            }
            4 => {
                self.config.daily_quote = !self.config.daily_quote;
                let _ = self.config.save();
                let on = self.config.daily_quote;
                self.set_status(if on { "daily quote on" } else { "daily quote off" }, false);
            }
            5 => {
                let backend = self.config.mpv_path.clone();
                let note = if self.player.is_available() {
                    format!("audio backend — {backend}")
                } else {
                    format!("audio backend — {backend} (not found)")
                };
                self.set_status(note, false);
            }
            6 => {
                let next = match self.config.search_limit {
                    n if n < 20 => 20,
                    n if n < 30 => 30,
                    n if n < 50 => 50,
                    _ => 10,
                };
                self.config.search_limit = next;
                let _ = self.config.save();
                self.set_status(format!("search results — {next}"), false);
            }
            7 => {
                self.config.progress_remaining = !self.config.progress_remaining;
                let _ = self.config.save();
                let remaining = self.config.progress_remaining;
                self.set_status(
                    if remaining { "progress — remaining" } else { "progress — total" },
                    false,
                );
            }
            8 => {
                self.config.truecolor = !self.config.truecolor;
                let _ = self.config.save();
                // Rebuild the palette from the preset and re-adapt to the new
                // depth so the change shows immediately.
                self.theme = crate::theme::Theme::from_name(&self.config.theme)
                    .adapt(self.config.truecolor);
                let on = self.config.truecolor;
                self.set_status(if on { "truecolor on" } else { "truecolor off (256-colour)" }, false);
            }
            _ => {}
        }
    }
}

/// Keep a selection valid after a list changes: clamp it, or clear it if empty.
fn reselect(state: &mut ListState, len: usize) {
    if len == 0 {
        state.select(None);
    } else {
        let index = state.selected().unwrap_or(0).min(len - 1);
        state.select(Some(index));
    }
}
