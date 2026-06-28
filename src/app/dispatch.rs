//! Action handling: turning a resolved [`Action`] into a state change.
//!
//! This is the single mutator. It is kept cheap and synchronous — anything slow
//! (search, download) is spawned and reports back through the event channel.

use ratatui::widgets::ListState;

use super::{App, AppEvent, LIBRARY_ITEMS, Overlay, SETTINGS_ITEMS, View};
use crate::commands::Action;
use crate::models::Track;
use crate::{downloads, utils};

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
            Activate => self.activate(),

            TogglePlay => self.player.toggle_pause(),
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
            ToggleLike => self.toggle_like(),
            Enqueue => self.enqueue_selected(),
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

    /// The list length and selection state for the current view, if it has one.
    fn active_selection(&mut self) -> Option<(usize, &mut ListState)> {
        match self.view {
            View::Search => Some((self.results.len(), &mut self.results_state)),
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
            View::Search => self.play_selected(self.results.clone(), self.results_state.selected()),
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

    /// Submit the search and move focus to the results.
    fn submit_search(&mut self) {
        self.editing = false;
        self.run_search();
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
                self.theme = self.theme.next();
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
                self.config.daily_quote = !self.config.daily_quote;
                let _ = self.config.save();
                let on = self.config.daily_quote;
                self.set_status(if on { "daily quote on" } else { "daily quote off" }, false);
            }
            4 => {
                let backend = self.config.mpv_path.clone();
                let note = if self.player.is_available() {
                    format!("audio backend — {backend}")
                } else {
                    format!("audio backend — {backend} (not found)")
                };
                self.set_status(note, false);
            }
            5 => {
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
