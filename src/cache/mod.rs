//! A tiny in-memory cache of resolved audio stream URLs.
//!
//! Resolving a stream through yt-dlp takes a second or two, so we remember the
//! result for a while. The URLs are time-limited by Google, so entries carry a
//! conservative TTL and are invalidated explicitly on a playback error.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Maps a track id to its most recently resolved stream URL, with an expiry.
pub struct StreamCache {
    ttl: Duration,
    entries: HashMap<String, (String, Instant)>,
}

impl StreamCache {
    /// A cache whose entries live for `ttl`.
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: HashMap::new(),
        }
    }

    /// Fetch a still-fresh URL for `id`, if one exists.
    pub fn get(&self, id: &str) -> Option<String> {
        self.entries.get(id).and_then(|(url, stored)| {
            if stored.elapsed() < self.ttl {
                Some(url.clone())
            } else {
                None
            }
        })
    }

    /// Remember `url` for `id` as of now.
    pub fn put(&mut self, id: impl Into<String>, url: impl Into<String>) {
        self.entries.insert(id.into(), (url.into(), Instant::now()));
    }

    /// Drop any cached URL for `id` (e.g. after it failed to play).
    pub fn invalidate(&mut self, id: &str) {
        self.entries.remove(id);
    }
}

impl Default for StreamCache {
    /// Four hours — comfortably shorter than YouTube's stream URL lifetime.
    fn default() -> Self {
        Self::new(Duration::from_secs(4 * 60 * 60))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_fresh_entries() {
        let mut c = StreamCache::new(Duration::from_secs(60));
        c.put("abc", "https://stream/1");
        assert_eq!(c.get("abc").as_deref(), Some("https://stream/1"));
        assert_eq!(c.get("missing"), None);
    }

    #[test]
    fn expires_old_entries() {
        let mut c = StreamCache::new(Duration::from_millis(0));
        c.put("abc", "https://stream/1");
        assert_eq!(c.get("abc"), None); // ttl of 0 -> immediately stale
    }

    #[test]
    fn invalidate_removes() {
        let mut c = StreamCache::default();
        c.put("abc", "u");
        c.invalidate("abc");
        assert_eq!(c.get("abc"), None);
    }
}
