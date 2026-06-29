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

use crate::models::{Playlist, Track};

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
    /// The container's own title — the playlist name, when loading a playlist.
    #[serde(default)]
    title: Option<String>,
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

/// Search YouTube for *playlists* matching `query`, returning up to `limit`.
///
/// yt-dlp has no `ytsearch` for playlists, so we extract the web results page
/// with YouTube's "Playlist" type filter (`sp=EgIQAw%3D%3D`) under flat mode —
/// fast, since nothing per-item is resolved.
pub async fn search_playlists(
    ytdlp: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<Playlist>, YtError> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!(
        "https://www.youtube.com/results?search_query={}&sp=EgIQAw%3D%3D",
        percent_encode(query)
    );
    let end = limit.max(1).to_string();
    let output = invoke(
        ytdlp,
        &[
            "--flat-playlist",
            "-J",
            "--no-warnings",
            "--ignore-errors",
            "--playlist-end",
            &end,
            &url,
        ],
    )
    .await?;

    if output.stdout.is_empty() {
        return Err(YtError::Failed(first_line(&output.stderr)));
    }
    let parsed: FlatSearch =
        serde_json::from_slice(&output.stdout).map_err(|e| YtError::Parse(e.to_string()))?;
    Ok(flat_to_playlists(parsed))
}

/// Load a playlist's tracks by its list id, returning the playlist's own title
/// alongside up to `limit` flat tracks (no per-item resolution).
pub async fn playlist_tracks(
    ytdlp: &str,
    id: &str,
    limit: usize,
) -> Result<(String, Vec<Track>), YtError> {
    let url = format!("https://www.youtube.com/playlist?list={id}");
    let end = limit.max(1).to_string();
    let output = invoke(
        ytdlp,
        &[
            "--flat-playlist",
            "-J",
            "--no-warnings",
            "--ignore-errors",
            "--playlist-end",
            &end,
            &url,
        ],
    )
    .await?;

    if output.stdout.is_empty() {
        return Err(YtError::Failed(first_line(&output.stderr)));
    }
    let parsed: FlatSearch =
        serde_json::from_slice(&output.stdout).map_err(|e| YtError::Parse(e.to_string()))?;
    let title = parsed.title.clone().unwrap_or_default();
    Ok((title, flat_to_tracks(parsed)))
}

/// Fetch one video's lightweight metadata (title, artist, duration) by id —
/// used when the user pastes a single video link. One extraction, no download.
pub async fn fetch_track(ytdlp: &str, id: &str) -> Result<Track, YtError> {
    let url = format!("https://www.youtube.com/watch?v={id}");
    let output = invoke(
        ytdlp,
        &[
            "--no-playlist",
            "--skip-download",
            "--no-warnings",
            "--print",
            "%(id)s\n%(title)s\n%(channel,uploader)s\n%(duration)s",
            &url,
        ],
    )
    .await?;

    if !output.status.success() {
        return Err(YtError::Failed(first_line(&output.stderr)));
    }

    // Four newline-separated fields, in print order. yt-dlp writes "NA" for any
    // it couldn't determine.
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let parsed_id = lines.next().unwrap_or("").trim();
    let title = lines.next().unwrap_or("").trim();
    let artist = lines.next().unwrap_or("").trim();
    let duration = lines.next().unwrap_or("").trim();

    let id = if parsed_id.is_empty() { id } else { parsed_id };
    let title = if title.is_empty() || title == "NA" {
        "(untitled)".to_string()
    } else {
        title.to_string()
    };
    let artist = if artist == "NA" { String::new() } else { artist.to_string() };
    let duration = duration.parse::<f64>().ok().map(|d| d.max(0.0) as u64);

    Ok(Track::new(id, title, artist, duration))
}

/// Extract a YouTube playlist id from `text` — either a URL carrying a `list=`
/// parameter (watch, playlist, or music links all work) or a bare playlist id.
/// Returns `None` for ordinary search text, so it's safe to probe any query.
pub fn playlist_id_from(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // URL form: pull the `list` query parameter out of anything URL-ish.
    if looks_url(text)
        && let Some(id) = url_query_value(text, "list")
    {
        return Some(id);
    }

    // Bare id form: a lone token that clearly looks like a playlist id.
    is_bare_playlist_id(text).then(|| text.to_string())
}

/// Extract a YouTube video id from a URL — `watch?v=`, `youtu.be/`, `/shorts/`,
/// `/embed/`, `/v/`, and the music variants. Only recognises proper URLs (an
/// 11-char id is too ambiguous to detect bare), so plain search text is left be.
pub fn video_id_from(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() || !looks_url(text) {
        return None;
    }
    // The `v=` query parameter (watch / music links).
    if let Some(id) = url_query_value(text, "v").filter(|id| is_video_id(id)) {
        return Some(id);
    }
    // Path-based forms: the id is the token right after the marker.
    for marker in ["youtu.be/", "/shorts/", "/embed/", "/v/"] {
        if let Some(pos) = text.find(marker) {
            let rest = &text[pos + marker.len()..];
            let id: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if is_video_id(&id) {
                return Some(id);
            }
        }
    }
    None
}

/// Whether `text` looks like a link we should try to parse rather than search.
fn looks_url(text: &str) -> bool {
    text.contains("://") || text.starts_with("www.") || text.contains("youtu")
}

/// The value of the `name=` query parameter (matched after `?` or `&`, so it
/// can't match inside another key), up to the next separator.
fn url_query_value(url: &str, name: &str) -> Option<String> {
    let start = url
        .find(&format!("?{name}="))
        .map(|p| p + name.len() + 2)
        .or_else(|| url.find(&format!("&{name}=")).map(|p| p + name.len() + 2))?;
    let rest = &url[start..];
    let end = rest
        .find(|c: char| c == '&' || c == '#' || c.is_whitespace())
        .unwrap_or(rest.len());
    let value = &rest[..end];
    (!value.is_empty()).then(|| value.to_string())
}

/// Whether `s` is a syntactically valid YouTube video id (exactly 11 chars of
/// the URL-safe id alphabet).
fn is_video_id(s: &str) -> bool {
    s.len() == 11 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Heuristic for a bare playlist id pasted on its own: id charset, no spaces, a
/// recognisable prefix, and at least one digit/underscore/hyphen — so a plain
/// word like `PLAYLIST` is never mistaken for one.
fn is_bare_playlist_id(text: &str) -> bool {
    if text.len() < 13 || text.contains(char::is_whitespace) {
        return false;
    }
    if !text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        return false;
    }
    let prefixes = ["PL", "OLAK", "RDCLAK", "UU", "FL", "LL"];
    let has_mark = text.bytes().any(|b| b.is_ascii_digit() || b == b'_' || b == b'-');
    has_mark && prefixes.iter().any(|p| text.starts_with(p))
}

/// Convert a parsed flat search into playlists, skipping null / id-less entries.
fn flat_to_playlists(parsed: FlatSearch) -> Vec<Playlist> {
    parsed
        .entries
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = entry.id?;
            Some(Playlist {
                id,
                title: entry
                    .title
                    .unwrap_or_else(|| "(untitled playlist)".to_string()),
                uploader: entry.channel.or(entry.uploader).unwrap_or_default(),
            })
        })
        .collect()
}

/// Percent-encode a query string for a URL (RFC 3986 unreserved set is left as-is;
/// everything else, spaces included, becomes `%XX`).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
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

    #[test]
    fn playlist_search_parses_list_entries() {
        // The shape yt-dlp emits for a playlist-filtered results page: channel and
        // uploader come back null, so only id + title survive.
        let json = r#"{"entries":[
            {"id":"PLaaa","title":"lofi beats","_type":"url","ie_key":"YoutubeTab"},
            null,
            {"id":"PLbbb","title":"rainy day jazz"},
            {"title":"no id"}
        ]}"#;
        let parsed: FlatSearch = serde_json::from_str(json).expect("parses despite the null");
        let playlists = flat_to_playlists(parsed);

        assert_eq!(playlists.len(), 2); // null and id-less entries dropped
        assert_eq!(playlists[0].id, "PLaaa");
        assert_eq!(playlists[0].title, "lofi beats");
        assert_eq!(playlists[1].id, "PLbbb");
    }

    #[test]
    fn playlist_title_is_kept() {
        let json = r#"{"title":"1 Hour of Lofi","entries":[
            {"id":"aaa","title":"track one","duration":200.0}
        ]}"#;
        let parsed: FlatSearch = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.title.as_deref(), Some("1 Hour of Lofi"));
        let tracks = flat_to_tracks(parsed);
        assert_eq!(tracks.len(), 1);
    }

    #[test]
    fn percent_encode_escapes_spaces_and_keeps_unreserved() {
        assert_eq!(percent_encode("lofi hip-hop"), "lofi%20hip-hop");
        assert_eq!(percent_encode("a&b=c"), "a%26b%3Dc");
        assert_eq!(percent_encode("plain.text~1_2"), "plain.text~1_2");
    }

    #[test]
    fn playlist_id_extracted_from_urls() {
        assert_eq!(
            playlist_id_from("https://www.youtube.com/playlist?list=PL123abc_-XYZ4567").as_deref(),
            Some("PL123abc_-XYZ4567")
        );
        // A watch link that also carries a list= still loads the playlist.
        assert_eq!(
            playlist_id_from("https://www.youtube.com/watch?v=dQw4&list=PLwxyz1234567&index=2")
                .as_deref(),
            Some("PLwxyz1234567")
        );
        assert_eq!(
            playlist_id_from("https://music.youtube.com/playlist?list=OLAK5uy_abcd1234EF").as_deref(),
            Some("OLAK5uy_abcd1234EF")
        );
    }

    #[test]
    fn playlist_id_handles_bare_ids_and_rejects_plain_text() {
        assert_eq!(
            playlist_id_from("PL1234567890abc").as_deref(),
            Some("PL1234567890abc")
        );
        assert_eq!(playlist_id_from("lofi hip hop"), None);
        assert_eq!(playlist_id_from("PLAYLISTOFSONGS"), None); // letters only — not an id
        assert_eq!(playlist_id_from("https://youtu.be/abc12345"), None); // a video, no list=
    }

    #[test]
    fn video_id_extracted_from_link_forms() {
        assert_eq!(
            video_id_from("https://www.youtube.com/watch?v=dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            video_id_from("https://youtu.be/dQw4w9WgXcQ?si=abc").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            video_id_from("https://music.youtube.com/watch?v=dQw4w9WgXcQ&list=RDAMVM").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            video_id_from("https://www.youtube.com/shorts/dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn video_id_ignores_plain_text() {
        assert_eq!(video_id_from("never gonna give you up"), None);
        // 11 chars but not a URL — too ambiguous to treat as an id.
        assert_eq!(video_id_from("dQw4w9WgXcQ"), None);
    }
}
