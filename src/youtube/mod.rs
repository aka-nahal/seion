//! YouTube (Music) access via the `yt-dlp` command-line tool.
//!
//! We shell out rather than depend on a networking stack — it keeps the binary
//! small and lets yt-dlp handle the perpetual churn of YouTube internals. Two
//! operations are exposed, both async (they spawn a subprocess and await it, so
//! the UI never blocks):
//!
//! * [`search`] — fast, flat search returning lightweight [`Track`]s.
//! * [`resolve_audio_url`] — turn a track id into a direct audio stream URL.
//!
//! The flat-search JSON shape and the resolve flags below were verified against
//! yt-dlp 2026.03 by running it directly.

use std::io::ErrorKind;
use std::process::{Output, Stdio};

use serde::Deserialize;
use tokio::process::Command;

use crate::models::Track;

/// Things that can go wrong talking to yt-dlp.
#[derive(Debug, thiserror::Error)]
pub enum YtError {
    /// The `yt-dlp` executable was not found on `PATH`.
    #[error("yt-dlp not found — install it and make sure it is on your PATH")]
    NotInstalled,
    /// yt-dlp ran but reported a failure.
    #[error("yt-dlp: {0}")]
    Failed(String),
    /// yt-dlp produced output we couldn't understand.
    #[error("could not parse yt-dlp output: {0}")]
    Parse(String),
    /// An underlying I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// One entry from `yt-dlp --flat-playlist -J`. Most fields are optional because
/// flat extraction is deliberately shallow.
#[derive(Debug, Deserialize)]
struct FlatEntry {
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    channel: Option<String>,
    #[serde(default)]
    uploader: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
}

/// The top-level object yt-dlp emits for a search "playlist".
///
/// Entries are `Option<FlatEntry>` because, under `--ignore-errors`, yt-dlp can
/// emit a failed item as a literal `null` in the array. Typing it as optional
/// lets one bad item be skipped instead of aborting the whole parse.
#[derive(Debug, Deserialize)]
struct FlatSearch {
    #[serde(default)]
    entries: Vec<Option<FlatEntry>>,
}

/// Search YouTube and return up to `limit` lightweight tracks.
///
/// Uses `ytsearchN:` with `--flat-playlist`, which resolves nothing per-item and
/// so comes back quickly. An empty / whitespace query yields an empty list.
pub async fn search(ytdlp: &str, query: &str, limit: usize) -> Result<Vec<Track>, YtError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let term = format!("ytsearch{}:{}", limit.max(1), query);
    let output = invoke(
        ytdlp,
        &[
            "--flat-playlist",
            "-J",
            "--no-warnings",
            "--ignore-errors",
            &term,
        ],
    )
    .await?;

    // With --ignore-errors yt-dlp can exit non-zero yet still have produced good
    // JSON on stdout, so we parse whenever there is anything to parse.
    if output.stdout.is_empty() {
        return Err(YtError::Failed(first_line(&output.stderr)));
    }

    let parsed: FlatSearch =
        serde_json::from_slice(&output.stdout).map_err(|e| YtError::Parse(e.to_string()))?;

    Ok(flat_to_tracks(parsed))
}

/// Convert a parsed flat search into tracks, skipping null / id-less entries and
/// using channel (then uploader) as the artist.
fn flat_to_tracks(parsed: FlatSearch) -> Vec<Track> {
    parsed
        .entries
        .into_iter()
        .flatten() // drop any null entries yt-dlp emitted for failed items
        .filter_map(|entry| {
            let id = entry.id?;
            Some(Track::new(
                id,
                entry.title.unwrap_or_else(|| "(untitled)".to_string()),
                entry.channel.or(entry.uploader).unwrap_or_default(),
                entry.duration.map(|d| d.max(0.0) as u64),
            ))
        })
        .collect()
}

/// Resolve a track id to a direct, playable audio stream URL.
///
/// The returned googlevideo URL is time-limited, so callers should treat it as
/// perishable (re-resolve on playback error) rather than caching it forever.
pub async fn resolve_audio_url(ytdlp: &str, id: &str) -> Result<String, YtError> {
    let watch_url = format!("https://www.youtube.com/watch?v={id}");
    let output = invoke(
        ytdlp,
        &[
            "-f",
            "bestaudio/best",
            "-g",
            "--no-playlist",
            "--no-warnings",
            &watch_url,
        ],
    )
    .await?;

    if !output.status.success() {
        return Err(YtError::Failed(first_line(&output.stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("http"))
        .map(str::to_string)
        .ok_or_else(|| YtError::Failed("yt-dlp returned no stream url".to_string()))
}

/// Run yt-dlp with the given args, mapping "executable missing" to a clear error
/// and reading the full output. `stdin` is closed so it never waits on input.
async fn invoke(program: &str, args: &[&str]) -> Result<Output, YtError> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound => YtError::NotInstalled,
            _ => YtError::Io(e),
        })
}

/// The first non-empty line of (UTF-8-lossy) bytes, for surfacing yt-dlp errors.
fn first_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("unknown error")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_skips_null_and_id_less_entries() {
        // A real-world shape: one good entry, a null (failed under
        // --ignore-errors), and an entry with only an uploader (no channel).
        let json = r#"{"entries":[
            {"id":"aaa","title":"rainy cafe jazz","channel":"nujabes","duration":342.0},
            null,
            {"id":"bbb","title":"lamp","uploader":"lamp official"},
            {"title":"no id here"}
        ]}"#;
        let parsed: FlatSearch = serde_json::from_str(json).expect("parses despite the null");
        let tracks = flat_to_tracks(parsed);

        assert_eq!(tracks.len(), 2); // null and id-less entries dropped
        assert_eq!(tracks[0].id, "aaa");
        assert_eq!(tracks[0].artist, "nujabes");
        assert_eq!(tracks[0].duration, Some(342));
        assert_eq!(tracks[1].id, "bbb");
        assert_eq!(tracks[1].artist, "lamp official"); // uploader fallback
    }
}
