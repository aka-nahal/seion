//! Local persistence: liked songs and listening history, in a small SQLite file.
//!
//! rusqlite is synchronous. The tables are tiny and queries are sub-millisecond,
//! so calls are made directly from the (single-threaded) event loop rather than
//! pushed onto a blocking pool — the simplest thing that stays imperceptible.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::models::Track;
use crate::utils;

/// Schema for the on-disk store. `IF NOT EXISTS` makes opening idempotent.
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS liked (
    id        TEXT PRIMARY KEY,
    title     TEXT NOT NULL,
    artist    TEXT NOT NULL,
    duration  INTEGER,
    added_at  INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS history (
    id        TEXT PRIMARY KEY,
    title     TEXT NOT NULL,
    artist    TEXT NOT NULL,
    duration  INTEGER,
    played_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS history_played_at ON history(played_at DESC);
";

/// A handle to the Seion database.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (creating if needed) the database in the platform data directory.
    pub fn open() -> anyhow::Result<Self> {
        let path = Self::path().context("could not resolve a data directory")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(&path)
            .with_context(|| format!("opening database at {}", path.display()))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// An ephemeral in-memory database — used by tests and as a last-resort
    /// fallback if the on-disk file cannot be opened.
    pub fn in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    /// Path to `seion.db` within the platform data directory.
    pub fn path() -> Option<PathBuf> {
        utils::project_dirs().map(|d| d.data_dir().join("seion.db"))
    }

    fn migrate(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(SCHEMA)?;
        Ok(())
    }

    // --- liked songs -------------------------------------------------------

    /// Is this track in the liked set?
    pub fn is_liked(&self, id: &str) -> bool {
        self.conn
            .query_row("SELECT 1 FROM liked WHERE id = ?1", params![id], |_| Ok(()))
            .optional()
            .unwrap_or(None)
            .is_some()
    }

    /// Add a track to the liked set (no-op if already present).
    pub fn like(&self, track: &Track) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO liked (id, title, artist, duration, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                track.id,
                track.title,
                track.artist,
                track.duration.map(|d| d as i64),
                now(),
            ],
        )?;
        Ok(())
    }

    /// Remove a track from the liked set.
    pub fn unlike(&self, id: &str) -> anyhow::Result<()> {
        self.conn
            .execute("DELETE FROM liked WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Flip the liked state of a track, returning the new state (`true` = liked).
    pub fn toggle_like(&self, track: &Track) -> anyhow::Result<bool> {
        if self.is_liked(&track.id) {
            self.unlike(&track.id)?;
            Ok(false)
        } else {
            self.like(track)?;
            Ok(true)
        }
    }

    /// All liked tracks, most recently liked first.
    pub fn liked(&self) -> anyhow::Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, artist, duration FROM liked ORDER BY added_at DESC, rowid DESC",
        )?;
        let rows = stmt.query_map([], row_to_track)?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    // --- history -----------------------------------------------------------

    /// Record that a track was played now. Each track appears once, bumped to
    /// the top of the list on replay.
    pub fn record_play(&self, track: &Track) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO history (id, title, artist, duration, played_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                track.id,
                track.title,
                track.artist,
                track.duration.map(|d| d as i64),
                now(),
            ],
        )?;
        Ok(())
    }

    /// The most recently played tracks, newest first.
    pub fn history(&self, limit: usize) -> anyhow::Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, artist, duration FROM history ORDER BY played_at DESC, rowid DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], row_to_track)?;
        Ok(rows.filter_map(Result::ok).collect())
    }
}

/// Milliseconds since the Unix epoch, as a signed integer for SQLite.
///
/// Millisecond resolution keeps "most recent first" ordering well-defined even
/// when two tracks are touched in quick succession; a `rowid` tiebreak in the
/// queries handles any remaining ties (a replayed row gets a fresh, higher
/// rowid via `INSERT OR REPLACE`).
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Build a [`Track`] from a row selecting `id, title, artist, duration`.
fn row_to_track(row: &Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get("id")?,
        title: row.get("title")?,
        artist: row.get("artist")?,
        album: None,
        duration: row.get::<_, Option<i64>>("duration")?.map(|d| d as u64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: &str) -> Track {
        Track::new(id, format!("title {id}"), "artist", Some(180))
    }

    #[test]
    fn like_toggle_and_query() {
        let db = Database::in_memory().unwrap();
        let t = track("a");
        assert!(!db.is_liked("a"));
        assert!(db.toggle_like(&t).unwrap()); // now liked
        assert!(db.is_liked("a"));
        assert_eq!(db.liked().unwrap().len(), 1);
        assert!(!db.toggle_like(&t).unwrap()); // now unliked
        assert!(db.liked().unwrap().is_empty());
    }

    #[test]
    fn history_dedupes_and_orders() {
        let db = Database::in_memory().unwrap();
        db.record_play(&track("a")).unwrap();
        db.record_play(&track("b")).unwrap();
        db.record_play(&track("a")).unwrap(); // replay bumps 'a' to the top
        let h = db.history(10).unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].id, "a");
    }
}
