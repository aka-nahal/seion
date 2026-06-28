//! Lyrics — best effort.
//!
//! This build ships without a remote lyrics provider (it would mean another
//! network dependency, against the spirit of the app), so [`for_track`] returns
//! `None` and the lyrics view shows a quiet, centered display of the song. The
//! type is here so a provider can be added later without touching the UI.

use crate::models::Track;

/// A set of lyric lines for a track.
#[derive(Debug, Clone, Default)]
pub struct Lyrics {
    /// The lines, top to bottom.
    pub lines: Vec<String>,
}

/// Fetch lyrics for a track. Always `None` for now (no provider configured).
pub fn for_track(_track: &Track) -> Option<Lyrics> {
    None
}
