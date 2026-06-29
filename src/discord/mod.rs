//! Discord Rich Presence — quietly share what you're listening to.
//!
//! Discord exposes a local IPC endpoint (a named pipe on Windows, a Unix domain
//! socket elsewhere) named `discord-ipc-0` … `discord-ipc-9`. We connect as a
//! client, perform the one-frame handshake, then push `SET_ACTIVITY` frames as
//! the track or pause-state changes. All of that lives on a background task; the
//! public [`Discord`] handle is a thin, non-blocking sender that also de-dupes,
//! so we only speak to Discord when something has actually changed.
//!
//! Like the mpv backend, this degrades gently in every direction:
//!
//! * no application id configured → the whole thing stays dormant, no task, no
//!   sockets, zero overhead;
//! * Discord not running → we retry on a slow timer, so presence appears if it
//!   starts later;
//! * a bad application id → Discord refuses the handshake and we quietly give up.
//!
//! The wire format is two little-endian `u32`s — opcode and length — followed by
//! that many bytes of JSON.

use std::io::{self, ErrorKind};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::config::Config;
use crate::player::PlayerState;

/// Seion's registered Discord application id, used by default so Rich Presence
/// works out of the box. Its name is what appears after "Listening to …".
/// Override it with `discord_client_id` in `config.toml` to use your own app.
pub const DEFAULT_CLIENT_ID: &str = "1521147450839011348";

/// Frame opcodes from the Discord IPC protocol.
const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;

/// Activity type 2 renders as "Listening to …", which fits a music player.
const ACTIVITY_LISTENING: u8 = 2;

/// How long to wait between reconnection attempts while Discord is unreachable.
const RECONNECT_DELAY: Duration = Duration::from_secs(15);

/// How long to wait for Discord to answer a frame before treating it as dead.
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// A seek smaller than this (in ms) is treated as ordinary drift and does not
/// provoke a presence update; anything larger re-anchors the elapsed timer.
const SEEK_EPSILON_MS: i64 = 2_000;

/// The platform IPC stream type — a named pipe on Windows, a socket elsewhere.
#[cfg(windows)]
type Conn = tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(not(windows))]
type Conn = tokio::net::UnixStream;

/// A command for the background presence task.
enum Cmd {
    /// Set the activity to this.
    Update(Activity),
    /// Clear the activity entirely.
    Clear,
}

/// A fully-built activity, ready to serialise. Optional fields are omitted from
/// the wire form when absent (Discord rejects empty or one-character strings).
#[derive(Clone)]
struct Activity {
    /// The first, bold line — the song title.
    details: Option<String>,
    /// The second line — the artist.
    state: Option<String>,
    /// Cover art, as an image URL (Discord proxies it).
    large_image: Option<String>,
    /// Hover text for the cover art.
    large_text: Option<String>,
    /// `(start, end)` epoch-millisecond stamps for the elapsed/remaining bar.
    timestamps: Option<(i64, Option<i64>)>,
}

/// A compact fingerprint of the last presence we sent, for de-duplication.
struct Sig {
    id: String,
    paused: bool,
    start: i64,
}

/// The calm public face of the Discord integration.
///
/// Holds a sender to the background task (or nothing, when dormant) and remembers
/// what it last published so repeated [`sync`](Self::sync) calls — which happen on
/// every event-loop turn — stay silent unless the state really moved.
pub struct Discord {
    tx: Option<UnboundedSender<Cmd>>,
    enabled: bool,
    last: Option<Sig>,
}

impl Discord {
    /// Start the integration. Spawns the background task only when an application
    /// id is configured; otherwise the handle is inert and every method is a
    /// no-op. The `enabled` toggle gates *publishing*, not the connection, so it
    /// can be flipped at runtime without a restart.
    pub fn launch(config: &Config) -> Self {
        let client_id = config.discord_client_id.trim().to_string();
        let tx = if client_id.is_empty() {
            None
        } else {
            let (tx, rx) = mpsc::unbounded_channel();
            tokio::spawn(run(client_id, rx));
            Some(tx)
        };
        Discord {
            tx,
            enabled: config.discord_presence,
            last: None,
        }
    }

    /// Whether an application id is set (so there is a task to talk to).
    pub fn is_configured(&self) -> bool {
        self.tx.is_some()
    }

    /// Turn publishing on or off. Does not itself send anything — the caller
    /// follows with [`sync`](Self::sync) (or [`clear`](Self::clear)).
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }

    /// Reconcile Discord with the current playback state, sending an update only
    /// when the track, pause-state, or position (a seek) actually changed.
    pub fn sync(&mut self, state: &PlayerState) {
        let Some(tx) = &self.tx else {
            return;
        };

        // Disabled, or nothing playing → make sure we've cleared, once.
        let track = if self.enabled { state.current.as_ref() } else { None };
        let Some(track) = track else {
            if self.last.take().is_some() {
                let _ = tx.send(Cmd::Clear);
            }
            return;
        };

        // While playing we anchor a start time so Discord shows a live elapsed
        // timer; while paused (or still loading) we drop timestamps so the timer
        // doesn't keep ticking on a frozen track.
        let timed = !state.paused && !state.loading;
        let start = if timed {
            crate::utils::now_unix_millis() - (state.position * 1000.0) as i64
        } else {
            0
        };

        let changed = match &self.last {
            Some(prev) => {
                prev.id != track.id
                    || prev.paused != state.paused
                    || (timed && (prev.start - start).abs() > SEEK_EPSILON_MS)
            }
            None => true,
        };
        if !changed {
            return;
        }

        let timestamps = timed.then(|| {
            let end = (state.duration > 0.0).then(|| start + (state.duration * 1000.0) as i64);
            (start, end)
        });
        let artist = clamp_field(&track.artist);
        let activity = Activity {
            details: clamp_field(&track.title),
            state: artist.clone(),
            large_image: thumbnail_url(&track.id),
            large_text: artist,
            timestamps,
        };

        let _ = tx.send(Cmd::Update(activity));
        self.last = Some(Sig {
            id: track.id.clone(),
            paused: state.paused,
            start,
        });
    }

    /// Clear the presence now (e.g. the user toggled the feature off).
    pub fn clear(&mut self) {
        if let Some(tx) = &self.tx
            && self.last.take().is_some()
        {
            let _ = tx.send(Cmd::Clear);
        }
    }

    /// Ask the task to clear presence on the way out. Best-effort: dropping the
    /// IPC socket on exit also makes Discord remove the activity.
    pub fn shutdown(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Cmd::Clear);
        }
    }
}

/// The background task: connect, handshake, then relay commands to Discord,
/// reconnecting on a slow timer whenever the link drops.
async fn run(client_id: String, mut rx: UnboundedReceiver<Cmd>) {
    // One eager attempt up front so presence appears promptly when Discord is
    // already running. A refused handshake means a bad id — give up for good.
    let mut conn = match handshake(&client_id).await {
        Ok(conn) => Some(conn),
        Err(HandshakeError::Rejected) => return,
        Err(HandshakeError::NotConnected) => None,
    };
    let mut desired: Option<Activity> = None;

    loop {
        match &mut conn {
            // Connected: relay commands, dropping the link on any write error so
            // the disconnected arm can pick reconnection back up.
            Some(stream) => match rx.recv().await {
                None => break,
                Some(cmd) => {
                    desired = intent(&cmd);
                    if push(stream, &cmd).await.is_err() {
                        conn = None;
                    }
                }
            },
            // Disconnected: record intent and retry on a timer.
            None => {
                tokio::select! {
                    cmd = rx.recv() => match cmd {
                        None => break,
                        Some(cmd) => desired = intent(&cmd),
                    },
                    _ = tokio::time::sleep(RECONNECT_DELAY) => {
                        match handshake(&client_id).await {
                            Ok(mut stream) => {
                                if let Some(activity) = &desired {
                                    let _ = push(&mut stream, &Cmd::Update(activity.clone())).await;
                                }
                                conn = Some(stream);
                            }
                            Err(HandshakeError::Rejected) => break,
                            Err(HandshakeError::NotConnected) => {}
                        }
                    }
                }
            }
        }
    }

    // Leave a clean slate behind us if we still hold the connection.
    if let Some(mut stream) = conn {
        let _ = push(&mut stream, &Cmd::Clear).await;
    }
}

/// The "what we want shown" state a command implies, for the reconnect path to
/// replay once the link is back.
fn intent(cmd: &Cmd) -> Option<Activity> {
    match cmd {
        Cmd::Update(activity) => Some(activity.clone()),
        Cmd::Clear => None,
    }
}

/// Why a handshake didn't yield a usable connection.
enum HandshakeError {
    /// Couldn't reach Discord — transient; worth retrying.
    NotConnected,
    /// Discord answered but refused us (e.g. an invalid id) — permanent.
    Rejected,
}

/// Connect and perform the opening handshake, returning a ready connection.
async fn handshake(client_id: &str) -> Result<Conn, HandshakeError> {
    let mut conn = connect().await.map_err(|_| HandshakeError::NotConnected)?;

    let hello = json!({ "v": 1, "client_id": client_id });
    let bytes = serde_json::to_vec(&hello).unwrap_or_default();
    write_frame(&mut conn, OP_HANDSHAKE, &bytes)
        .await
        .map_err(|_| HandshakeError::NotConnected)?;

    // Discord replies with a READY frame on success, or a CLOSE frame if it
    // rejects us (commonly a bad client id).
    match tokio::time::timeout(IO_TIMEOUT, read_frame(&mut conn)).await {
        Ok(Ok((OP_CLOSE, _))) => Err(HandshakeError::Rejected),
        Ok(Ok(_)) => Ok(conn),
        _ => Err(HandshakeError::NotConnected),
    }
}

/// Send one command as a `SET_ACTIVITY` frame and drain Discord's reply. Reading
/// the reply both keeps the pipe from backing up and doubles as a liveness check.
async fn push(conn: &mut Conn, cmd: &Cmd) -> io::Result<()> {
    let activity = match cmd {
        Cmd::Update(activity) => activity_value(activity),
        Cmd::Clear => Value::Null,
    };
    let payload = json!({
        "cmd": "SET_ACTIVITY",
        "args": { "pid": std::process::id(), "activity": activity },
        "nonce": next_nonce(),
    });
    let bytes = serde_json::to_vec(&payload).unwrap_or_default();
    write_frame(conn, OP_FRAME, &bytes).await?;

    match tokio::time::timeout(IO_TIMEOUT, read_frame(conn)).await {
        Ok(Ok((OP_CLOSE, _))) => Err(io::Error::new(
            ErrorKind::ConnectionAborted,
            "discord closed the connection",
        )),
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(io::Error::new(ErrorKind::TimedOut, "discord did not respond")),
    }
}

/// Serialise an [`Activity`] into Discord's activity object, omitting any field
/// we don't have so we never send an empty or too-short string.
fn activity_value(activity: &Activity) -> Value {
    let mut value = json!({ "type": ACTIVITY_LISTENING });
    if let Some(details) = &activity.details {
        value["details"] = json!(details);
    }
    if let Some(state) = &activity.state {
        value["state"] = json!(state);
    }
    if let Some((start, end)) = activity.timestamps {
        let mut stamps = json!({ "start": start });
        if let Some(end) = end {
            stamps["end"] = json!(end);
        }
        value["timestamps"] = stamps;
    }
    if let Some(image) = &activity.large_image {
        let mut assets = json!({ "large_image": image });
        if let Some(text) = &activity.large_text {
            assets["large_text"] = json!(text);
        }
        value["assets"] = assets;
    }
    value
}

/// A monotonically increasing nonce for `SET_ACTIVITY` frames.
fn next_nonce() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed).to_string()
}

/// Trim a field to Discord's limits, or `None` if there's nothing to show.
///
/// Discord rejects activity strings outside 1–128 characters and silently drops
/// one-character ones, so we cap the length and pad a lone character to two.
fn clamp_field(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut out: String = trimmed.chars().take(128).collect();
    if out.chars().count() < 2 {
        out.push(' ');
    }
    Some(out)
}

/// The YouTube thumbnail URL for a track id, or `None` for local files.
fn thumbnail_url(id: &str) -> Option<String> {
    if id.is_empty() || id.starts_with(crate::downloads::FILE_SCHEME) {
        return None;
    }
    Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"))
}

/// Write one framed message: little-endian opcode, length, then the payload.
async fn write_frame<S: AsyncWrite + Unpin>(
    conn: &mut S,
    opcode: u32,
    payload: &[u8],
) -> io::Result<()> {
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&opcode.to_le_bytes());
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(payload);
    conn.write_all(&frame).await?;
    conn.flush().await
}

/// Read one framed message, returning its opcode and JSON body bytes.
async fn read_frame<S: AsyncRead + Unpin>(conn: &mut S) -> io::Result<(u32, Vec<u8>)> {
    let mut header = [0u8; 8];
    conn.read_exact(&mut header).await?;
    let opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    // Guard against a corrupt length trying to allocate the world.
    if len > 1 << 20 {
        return Err(io::Error::new(ErrorKind::InvalidData, "discord frame too large"));
    }
    let mut body = vec![0u8; len];
    conn.read_exact(&mut body).await?;
    Ok((opcode, body))
}

/// Connect to the first available Discord IPC pipe (`discord-ipc-0`..=`9`).
#[cfg(windows)]
async fn connect() -> io::Result<Conn> {
    use tokio::net::windows::named_pipe::ClientOptions;
    for i in 0..10 {
        let path = format!(r"\\.\pipe\discord-ipc-{i}");
        if let Ok(client) = ClientOptions::new().open(&path) {
            return Ok(client);
        }
    }
    Err(io::Error::new(ErrorKind::NotFound, "no discord ipc pipe"))
}

/// Connect to the first available Discord IPC socket across the known runtime
/// directories (`discord-ipc-0`..=`9` under each).
#[cfg(not(windows))]
async fn connect() -> io::Result<Conn> {
    use tokio::net::UnixStream;
    for base in ipc_dirs() {
        for i in 0..10 {
            let path = base.join(format!("discord-ipc-{i}"));
            if let Ok(stream) = UnixStream::connect(&path).await {
                return Ok(stream);
            }
        }
    }
    Err(io::Error::new(ErrorKind::NotFound, "no discord ipc socket"))
}

/// The directories Discord may place its socket in, most-specific last. Covers a
/// plain install plus the Flatpak and Snap sandbox layouts.
#[cfg(not(windows))]
fn ipc_dirs() -> Vec<std::path::PathBuf> {
    use std::path::PathBuf;
    let mut roots: Vec<PathBuf> = Vec::new();
    for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
        if let Ok(dir) = std::env::var(key)
            && !dir.is_empty()
        {
            roots.push(PathBuf::from(dir));
        }
    }
    roots.push(PathBuf::from("/tmp"));

    let mut dirs = Vec::with_capacity(roots.len() * 3);
    for root in roots {
        dirs.push(root.join("app/com.discordapp.Discord"));
        dirs.push(root.join("snap.discord"));
        dirs.push(root);
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_clamped_and_padded() {
        assert_eq!(clamp_field("  "), None);
        assert_eq!(clamp_field("x").as_deref(), Some("x ")); // padded to two
        assert_eq!(clamp_field("lofi").as_deref(), Some("lofi"));
        assert_eq!(clamp_field(&"a".repeat(200)).unwrap().chars().count(), 128);
    }

    #[test]
    fn thumbnails_only_for_youtube_ids() {
        assert_eq!(
            thumbnail_url("dQw4w9WgXcQ").as_deref(),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg")
        );
        assert_eq!(thumbnail_url("file:/home/x/song.opus"), None);
        assert_eq!(thumbnail_url(""), None);
    }

    #[test]
    fn activity_omits_absent_fields() {
        let activity = Activity {
            details: Some("Title".into()),
            state: None,
            large_image: None,
            large_text: None,
            timestamps: Some((1_000, None)),
        };
        let value = activity_value(&activity);
        assert_eq!(value["type"], json!(ACTIVITY_LISTENING));
        assert_eq!(value["details"], json!("Title"));
        assert!(value.get("state").is_none());
        assert!(value.get("assets").is_none());
        assert_eq!(value["timestamps"]["start"], json!(1_000));
        assert!(value["timestamps"].get("end").is_none());
    }

    #[test]
    fn activity_includes_full_payload() {
        let activity = Activity {
            details: Some("Title".into()),
            state: Some("Artist".into()),
            large_image: Some("http://img".into()),
            large_text: Some("Artist".into()),
            timestamps: Some((1_000, Some(5_000))),
        };
        let value = activity_value(&activity);
        assert_eq!(value["state"], json!("Artist"));
        assert_eq!(value["assets"]["large_image"], json!("http://img"));
        assert_eq!(value["assets"]["large_text"], json!("Artist"));
        assert_eq!(value["timestamps"]["end"], json!(5_000));
    }
}
