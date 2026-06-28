//! Playback control.
//!
//! [`Player`] is the calm public face: it owns the queue, the repeat/shuffle
//! state, a mirror of "what is playing right now", and a small cache of resolved
//! stream URLs. It talks to a background [`mpv`] task over a channel and never
//! blocks. Stream resolution (yt-dlp) happens off to the side in spawned tasks
//! that report back as [`PlayerEvent`]s.
//!
//! If mpv is not installed the player still works as a quiet metadata display:
//! every control becomes a no-op and play requests surface a gentle error.

pub mod mpv;

use tokio::sync::mpsc::UnboundedSender;

use crate::app::AppEvent;
use crate::cache::StreamCache;
use crate::config::Config;
use crate::models::{RepeatMode, Track};
use crate::utils;

pub use mpv::{Mpv, MpvCommand};

/// Something the player learned from mpv (or from a resolver task).
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    /// Current playback position, in seconds.
    Position(f64),
    /// Track length, in seconds.
    Duration(f64),
    /// Pause state changed.
    Paused(bool),
    /// A stream URL finished resolving for the given track id.
    Resolved { id: String, url: String },
    /// mpv finished loading the file and playback has started.
    Loaded,
    /// The current file ended, for the given reason.
    Ended(EndReason),
    /// Something went wrong; carries a human-facing message.
    Error(String),
}

/// Why a track stopped playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// Reached the natural end — advance the queue.
    Eof,
    /// We stopped or replaced it deliberately — do nothing.
    Stopped,
    /// Playback failed (e.g. an expired stream URL).
    Failed,
    /// Anything else.
    Other,
}

impl EndReason {
    /// Map mpv's `end-file` `reason` string to our enum.
    pub fn from_mpv(reason: &str) -> Self {
        match reason {
            "eof" => EndReason::Eof,
            "stop" | "quit" | "redirect" => EndReason::Stopped,
            "error" => EndReason::Failed,
            _ => EndReason::Other,
        }
    }
}

/// A read-only snapshot of playback, mirrored from mpv for the UI to render.
#[derive(Debug, Default, Clone)]
pub struct PlayerState {
    /// The track we are playing (or last played).
    pub current: Option<Track>,
    /// True while we are waiting for a stream to resolve / load.
    pub loading: bool,
    /// True if paused.
    pub paused: bool,
    /// Position in seconds.
    pub position: f64,
    /// Duration in seconds (0.0 if unknown, e.g. a live stream).
    pub duration: f64,
    /// Volume, `0..=100`.
    pub volume: u8,
}

impl PlayerState {
    /// Are we actively making sound right now?
    pub fn is_playing(&self) -> bool {
        self.current.is_some() && !self.paused && !self.loading
    }

    /// Is there a track loaded at all?
    pub fn has_track(&self) -> bool {
        self.current.is_some()
    }
}

/// The result of handling a [`PlayerEvent`], for the App to act on.
pub enum Notice {
    /// Nothing the App needs to do.
    Nothing,
    /// A new track just started — record it in history.
    Started(Track),
    /// Surface this message to the user.
    Error(String),
}

/// The playback controller.
pub struct Player {
    available: bool,
    cmd_tx: Option<UnboundedSender<MpvCommand>>,
    app_tx: UnboundedSender<AppEvent>,
    ytdlp: String,
    cache: StreamCache,
    rng: utils::Rng,
    retry_used: bool,

    /// Live playback snapshot.
    pub state: PlayerState,
    /// The current queue (the context we're playing within).
    pub queue: Vec<Track>,
    /// Index of the current track inside [`queue`](Self::queue).
    pub queue_pos: Option<usize>,
    /// Repeat mode.
    pub repeat: RepeatMode,
    /// Whether the next track is chosen at random.
    pub shuffle: bool,
}

impl Player {
    /// Start the player, launching the mpv backend if it is installed.
    ///
    /// Returns immediately — mpv's IPC connection is established in the
    /// background, so building the player never delays startup.
    pub fn launch(config: &Config, app_tx: UnboundedSender<AppEvent>) -> Self {
        let state = PlayerState {
            volume: config.volume,
            ..PlayerState::default()
        };

        let (available, cmd_tx) = match Mpv::launch(&config.mpv_path, app_tx.clone()) {
            Ok(mpv) => {
                let tx = mpv.cmd_tx.clone();
                // Apply the saved volume up front, while idle.
                let _ = tx.send(MpvCommand::SetVolume(state.volume));
                (true, Some(tx))
            }
            Err(_) => (false, None),
        };

        Player {
            available,
            cmd_tx,
            app_tx,
            ytdlp: config.ytdlp_path.clone(),
            cache: StreamCache::default(),
            rng: utils::Rng::from_clock(),
            retry_used: false,
            state,
            queue: Vec::new(),
            queue_pos: None,
            repeat: RepeatMode::default(),
            shuffle: false,
        }
    }

    /// Is an audio backend available? When false, the player is display-only.
    pub fn is_available(&self) -> bool {
        self.available
    }

    // --- starting playback -------------------------------------------------

    /// Replace the queue with `list` and start playing item `index`.
    pub fn play_from(&mut self, list: Vec<Track>, index: usize) {
        if list.is_empty() {
            return;
        }
        let index = index.min(list.len() - 1);
        let track = list[index].clone();
        self.queue = list;
        self.queue_pos = Some(index);
        self.start(track);
    }

    /// Jump to a specific position in the current queue and play it.
    pub fn play_index(&mut self, index: usize) {
        if index >= self.queue.len() {
            return;
        }
        self.queue_pos = Some(index);
        let track = self.queue[index].clone();
        self.start(track);
    }

    /// Add a track to the end of the queue, starting it if nothing is playing.
    pub fn enqueue(&mut self, track: Track) {
        self.queue.push(track.clone());
        if !self.state.has_track() {
            self.queue_pos = Some(self.queue.len() - 1);
            self.start(track);
        }
    }

    /// Begin playing a specific track (internal: sets state + resolves stream).
    fn start(&mut self, track: Track) {
        self.state.current = Some(track.clone());
        self.state.position = 0.0;
        self.state.duration = track.duration.map(|d| d as f64).unwrap_or(0.0);
        self.state.paused = false;
        self.retry_used = false;
        self.request_stream(false);
    }

    /// Ask the backend to load the current track's audio stream, resolving it
    /// (via yt-dlp) if we don't have a fresh URL cached. `force` re-resolves.
    fn request_stream(&mut self, force: bool) {
        let Some(track) = self.state.current.clone() else {
            return;
        };

        if !self.available {
            self.state.loading = false;
            let _ = self.app_tx.send(AppEvent::Player(PlayerEvent::Error(
                "mpv not found — install mpv to play audio".to_string(),
            )));
            return;
        }

        self.state.loading = true;

        // A downloaded file plays directly — no yt-dlp round trip.
        if let Some(path) = track.id.strip_prefix(crate::downloads::FILE_SCHEME) {
            self.send(MpvCommand::Load(path.to_string()));
            return;
        }

        if force {
            self.cache.invalidate(&track.id);
        } else if let Some(url) = self.cache.get(&track.id) {
            self.send(MpvCommand::Load(url));
            return;
        }

        // Resolve off-thread; the result comes back as a PlayerEvent.
        let id = track.id.clone();
        let ytdlp = self.ytdlp.clone();
        let app_tx = self.app_tx.clone();
        tokio::spawn(async move {
            let event = match crate::youtube::resolve_audio_url(&ytdlp, &id).await {
                Ok(url) => PlayerEvent::Resolved { id, url },
                Err(e) => PlayerEvent::Error(format!("could not load track — {e}")),
            };
            let _ = app_tx.send(AppEvent::Player(event));
        });
    }

    // --- transport ---------------------------------------------------------

    /// Toggle pause/resume.
    pub fn toggle_pause(&mut self) {
        if !self.state.has_track() {
            return;
        }
        let paused = !self.state.paused;
        self.state.paused = paused;
        self.send(MpvCommand::SetPause(paused));
    }

    /// Advance to the next track, honouring repeat and shuffle.
    pub fn next(&mut self) {
        if matches!(self.repeat, RepeatMode::One) {
            if let Some(track) = self.state.current.clone() {
                self.start(track);
            }
            return;
        }
        if self.queue.is_empty() {
            return;
        }

        let len = self.queue.len();
        let current = self.queue_pos.unwrap_or(0);

        let next = if self.shuffle && len > 1 {
            let mut n = self.rng.below(len);
            if n == current {
                n = (n + 1) % len;
            }
            n
        } else if current + 1 < len {
            current + 1
        } else if matches!(self.repeat, RepeatMode::All) {
            0
        } else {
            // End of the queue with no repeat — come gently to rest.
            self.stop();
            return;
        };

        self.queue_pos = Some(next);
        let track = self.queue[next].clone();
        self.start(track);
    }

    /// Go to the previous track, or restart the current one if we're past its
    /// opening — the familiar "back" behaviour.
    pub fn previous(&mut self) {
        if self.state.position > 3.0 {
            self.seek_absolute(0.0);
            return;
        }
        if self.queue.is_empty() {
            return;
        }
        let len = self.queue.len();
        let current = self.queue_pos.unwrap_or(0);
        let prev = if current > 0 {
            current - 1
        } else if matches!(self.repeat, RepeatMode::All) {
            len - 1
        } else {
            0
        };
        self.queue_pos = Some(prev);
        let track = self.queue[prev].clone();
        self.start(track);
    }

    /// Stop playback and let the screen fall quiet.
    pub fn stop(&mut self) {
        self.send(MpvCommand::Stop);
        self.state.loading = false;
        self.state.paused = false;
        self.state.position = 0.0;
    }

    /// Seek by a relative number of seconds (negative to go back).
    pub fn seek_relative(&mut self, secs: f64) {
        if !self.state.has_track() {
            return;
        }
        self.send(MpvCommand::SeekRelative(secs));
        // Optimistic local update so the bar responds immediately.
        let max = if self.state.duration > 0.0 {
            self.state.duration
        } else {
            f64::MAX
        };
        self.state.position = (self.state.position + secs).clamp(0.0, max);
    }

    /// Seek to an absolute position in seconds.
    pub fn seek_absolute(&mut self, secs: f64) {
        if !self.state.has_track() {
            return;
        }
        let secs = secs.max(0.0);
        self.send(MpvCommand::SeekAbsolute(secs));
        self.state.position = secs;
    }

    /// Set the volume, persisting nothing (the caller decides about config).
    pub fn set_volume(&mut self, volume: u8) {
        let volume = volume.min(100);
        self.state.volume = volume;
        self.send(MpvCommand::SetVolume(volume));
    }

    /// Nudge the volume up by five.
    pub fn volume_up(&mut self) {
        self.set_volume(self.state.volume.saturating_add(5));
    }

    /// Nudge the volume down by five.
    pub fn volume_down(&mut self) {
        self.set_volume(self.state.volume.saturating_sub(5));
    }

    /// Cycle repeat mode off → all → one.
    pub fn cycle_repeat(&mut self) {
        self.repeat = self.repeat.cycle();
    }

    /// Toggle shuffle.
    pub fn toggle_shuffle(&mut self) {
        self.shuffle = !self.shuffle;
    }

    // --- events ------------------------------------------------------------

    /// Fold a [`PlayerEvent`] into player state, returning anything the App must
    /// act on (recording history, surfacing an error).
    pub fn handle_event(&mut self, event: PlayerEvent) -> Notice {
        match event {
            PlayerEvent::Position(pos) => {
                self.state.position = pos;
                Notice::Nothing
            }
            PlayerEvent::Duration(dur) => {
                if dur > 0.0 {
                    self.state.duration = dur;
                }
                Notice::Nothing
            }
            PlayerEvent::Paused(paused) => {
                self.state.paused = paused;
                Notice::Nothing
            }
            PlayerEvent::Resolved { id, url } => {
                self.cache.put(&id, &url);
                let is_current = self
                    .state
                    .current
                    .as_ref()
                    .map(|t| t.id == id)
                    .unwrap_or(false);
                if is_current && self.state.loading {
                    self.send(MpvCommand::Load(url));
                }
                Notice::Nothing
            }
            PlayerEvent::Loaded => {
                self.state.loading = false;
                self.state.paused = false;
                self.retry_used = false;
                match self.state.current.clone() {
                    Some(track) => Notice::Started(track),
                    None => Notice::Nothing,
                }
            }
            PlayerEvent::Ended(reason) => match reason {
                EndReason::Eof => {
                    self.next();
                    Notice::Nothing
                }
                EndReason::Failed => {
                    if !self.retry_used && self.state.has_track() {
                        // The stream URL likely expired — re-resolve once.
                        self.retry_used = true;
                        self.request_stream(true);
                        Notice::Nothing
                    } else {
                        self.next();
                        Notice::Error("could not play that track".to_string())
                    }
                }
                EndReason::Stopped | EndReason::Other => Notice::Nothing,
            },
            PlayerEvent::Error(message) => {
                self.state.loading = false;
                Notice::Error(message)
            }
        }
    }

    /// Ask mpv to quit. Called on shutdown.
    pub fn shutdown(&self) {
        self.send(MpvCommand::Quit);
    }

    /// Send a command to the backend, if one exists.
    fn send(&self, command: MpvCommand) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.send(command);
        }
    }
}
