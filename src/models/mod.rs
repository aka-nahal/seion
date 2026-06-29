//! Domain types shared across the whole application.
//!
//! These are deliberately small, cloneable, plain-data structures with no I/O
//! and no knowledge of the UI. Everything else — the player, the database, the
//! views — speaks in terms of these.

use serde::{Deserialize, Serialize};

use crate::utils;

/// A single piece of music: one YouTube (Music) track.
///
/// `artist` is the best available "who made this" string. YouTube's flat search
/// does not expose a clean artist field, so it is populated from the channel /
/// uploader name; for hand-built playlists it can be anything.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    /// The YouTube video id (the canonical identity of a track).
    pub id: String,
    /// Song title, shown as the primary line.
    pub title: String,
    /// Artist / channel, shown as the quiet secondary line.
    pub artist: String,
    /// Album name, if known. Mostly unused for search results.
    #[serde(default)]
    pub album: Option<String>,
    /// Length in whole seconds, if known ahead of playback.
    #[serde(default)]
    pub duration: Option<u64>,
}

impl Track {
    /// Construct a track from its essential fields.
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        artist: impl Into<String>,
        duration: Option<u64>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            artist: artist.into(),
            album: None,
            duration,
        }
    }

    /// The canonical watch URL, used to resolve an audio stream via yt-dlp.
    pub fn watch_url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.id)
    }

    /// `mm:ss` (or `h:mm:ss`), or a quiet `--:--` when the length is unknown.
    pub fn duration_str(&self) -> String {
        match self.duration {
            Some(secs) => utils::format_duration(secs),
            None => "--:--".to_string(),
        }
    }
}

/// A YouTube playlist, as returned by a playlist search. Lightweight: just
/// enough to show it and, on selection, fetch its tracks by [`id`](Self::id).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playlist {
    /// The YouTube list id (e.g. `PL…`), used to load the playlist's tracks.
    pub id: String,
    /// The playlist's name.
    pub title: String,
    /// The owning channel, when known (flat search often omits it).
    #[serde(default)]
    pub uploader: String,
}

/// How playback repeats when a track ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    /// Stop (or advance to the next queued track) and do not loop.
    #[default]
    Off,
    /// Loop the whole queue.
    All,
    /// Loop the current track.
    One,
}

impl RepeatMode {
    /// Advance to the next mode in the cycle off → all → one → off.
    pub fn cycle(self) -> Self {
        match self {
            RepeatMode::Off => RepeatMode::All,
            RepeatMode::All => RepeatMode::One,
            RepeatMode::One => RepeatMode::Off,
        }
    }

    /// A compact glyph for the player bar.
    pub fn glyph(self) -> &'static str {
        match self {
            RepeatMode::Off => "↻",
            RepeatMode::All => "↻ all",
            RepeatMode::One => "↻ one",
        }
    }

    /// Whether repeat is doing anything at all.
    pub fn is_on(self) -> bool {
        !matches!(self, RepeatMode::Off)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_cycles_full_circle() {
        assert_eq!(RepeatMode::Off.cycle(), RepeatMode::All);
        assert_eq!(RepeatMode::All.cycle(), RepeatMode::One);
        assert_eq!(RepeatMode::One.cycle(), RepeatMode::Off);
    }

    #[test]
    fn duration_renders_gracefully() {
        assert_eq!(Track::new("x", "t", "a", Some(137)).duration_str(), "02:17");
        assert_eq!(Track::new("x", "t", "a", None).duration_str(), "--:--");
    }

}
