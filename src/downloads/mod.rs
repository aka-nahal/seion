//! Offline downloads.
//!
//! A download is just the best audio stream saved to the data directory via
//! yt-dlp (no re-encoding, so no ffmpeg dependency). Downloaded files surface as
//! ordinary [`Track`]s whose id uses a `file:` scheme, which the player loads
//! directly — so offline tracks queue, skip, and play exactly like online ones.

use std::path::PathBuf;
use std::process::Stdio;

use tokio::process::Command;

use crate::models::Track;
use crate::utils;

/// The `file:` URI scheme used in [`Track::id`] for local files.
pub const FILE_SCHEME: &str = "file:";

/// Extensions we treat as playable audio. yt-dlp's in-progress (`.part`,
/// `.part-FragNN`) and resume (`.ytdl`) files are deliberately excluded, so an
/// interrupted or running download never appears as a (broken) track.
const AUDIO_EXTS: &[&str] = &[
    "m4a", "webm", "opus", "mp3", "ogg", "oga", "flac", "aac", "wav", "mka", "m4b",
];

/// Does this path look like a finished audio file we can play?
fn is_audio_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Directory where downloaded audio lives.
pub fn downloads_dir() -> Option<PathBuf> {
    utils::project_dirs().map(|d| d.data_dir().join("downloads"))
}

/// List downloaded files as playable tracks (id = `file:<absolute path>`).
pub fn list_tracks() -> Vec<Track> {
    let Some(dir) = downloads_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut tracks: Vec<Track> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_audio_file(p))
        .map(|path| {
            let title = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "(unknown)".to_string());
            Track::new(
                format!("{FILE_SCHEME}{}", path.to_string_lossy()),
                title,
                "downloaded",
                None,
            )
        })
        .collect();

    tracks.sort_by_key(|t| t.title.to_lowercase());
    tracks
}

/// Download a track's best audio to the downloads directory.
///
/// Returns the directory on success. Errors are returned as a human-facing
/// string for the status line.
pub async fn download(ytdlp: &str, track: &Track) -> Result<PathBuf, String> {
    let dir = downloads_dir().ok_or("no downloads directory available")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let template = dir.join("%(title)s [%(id)s].%(ext)s");
    let status = Command::new(ytdlp)
        .args([
            "-f",
            "bestaudio/best",
            "--no-playlist",
            "--no-warnings",
            "-o",
        ])
        .arg(&template)
        .arg(track.watch_url())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => "yt-dlp not found".to_string(),
            _ => e.to_string(),
        })?;

    if status.success() {
        Ok(dir)
    } else {
        Err("download failed".to_string())
    }
}
