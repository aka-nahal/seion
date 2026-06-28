//! The mpv backend: a child process driven over mpv's JSON IPC interface.
//!
//! mpv runs headless (`--no-video --idle`) and exposes a duplex IPC endpoint —
//! a named pipe on Windows, a Unix socket elsewhere. We connect as the client,
//! split the connection, and run two tasks:
//!
//! * a **reader** that parses newline-delimited JSON and forwards
//!   [`PlayerEvent`]s to the app, and
//! * a **writer** that serialises [`MpvCommand`]s and, on a gentle 500ms timer,
//!   polls position / duration / pause (polling avoids the documented event
//!   flood you get from observing `time-pos`).
//!
//! The writer owns the child process and cleans it up when the command channel
//! closes or a [`MpvCommand::Quit`] arrives.

use std::process::Stdio;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::MissedTickBehavior;

use crate::app::AppEvent;
use crate::player::{EndReason, PlayerEvent};
use crate::utils;

/// Fixed request ids tag the replies to our polled `get_property` calls, so the
/// reader knows which metric a reply belongs to without a correlation map.
const REQ_TIME_POS: i64 = 1;
const REQ_DURATION: i64 = 2;
const REQ_PAUSE: i64 = 3;

/// How often we poll mpv for playback position.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// How long to wait for mpv to create its IPC endpoint before giving up.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// A high-level command for the audio backend.
#[derive(Debug, Clone)]
pub enum MpvCommand {
    /// Load and play a stream URL, replacing whatever is current.
    Load(String),
    /// Pause (`true`) or resume (`false`).
    SetPause(bool),
    /// Set the volume, `0..=100`.
    SetVolume(u8),
    /// Seek by a relative number of seconds.
    SeekRelative(f64),
    /// Seek to an absolute position in seconds.
    SeekAbsolute(f64),
    /// Stop playback (unloads the current file).
    Stop,
    /// Ask mpv to quit.
    Quit,
}

/// A handle to the running mpv backend — just the command channel.
pub struct Mpv {
    /// Send [`MpvCommand`]s here to control playback.
    pub cmd_tx: UnboundedSender<MpvCommand>,
}

impl Mpv {
    /// Spawn mpv and start its IPC loops. Returns an [`std::io::Error`] of kind
    /// `NotFound` if mpv is not on `PATH`, which the caller treats as "audio
    /// unavailable".
    ///
    /// Connecting to the pipe happens on a background task, so neither this call
    /// nor the application's first paint waits on mpv creating its endpoint.
    /// Commands issued before the connection lands queue in the channel.
    pub fn launch(mpv_path: &str, app_tx: UnboundedSender<AppEvent>) -> std::io::Result<Mpv> {
        let endpoint = utils::ipc_endpoint();
        // Spawning is synchronous — this is where "mpv not installed" surfaces.
        let child = spawn_mpv(mpv_path, &endpoint)?;
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            match connect(&endpoint).await {
                Ok(stream) => {
                    let (read_half, write_half) = tokio::io::split(stream);
                    tokio::spawn(reader_task(read_half, app_tx));
                    writer_task(write_half, cmd_rx, child).await;
                }
                Err(_) => {
                    // Never reached mpv — don't let the child linger.
                    let mut child = child;
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                }
            }
        });

        Ok(Mpv { cmd_tx })
    }
}

/// Launch mpv headless with IPC enabled. All standard streams are silenced and
/// `kill_on_drop` ensures the process never outlives us.
fn spawn_mpv(mpv_path: &str, endpoint: &str) -> std::io::Result<Child> {
    Command::new(mpv_path)
        .args(["--no-video", "--no-terminal", "--idle=yes"])
        .arg(format!("--input-ipc-server={endpoint}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
}

/// Connect to mpv's IPC endpoint, retrying while mpv is still creating it.
#[cfg(windows)]
async fn connect(
    endpoint: &str,
) -> std::io::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    use tokio::net::windows::named_pipe::ClientOptions;

    // mpv creates the pipe server asynchronously after spawn, so the first
    // open() typically fails with ERROR_FILE_NOT_FOUND; ERROR_PIPE_BUSY means
    // it exists but is momentarily occupied. Retry on both.
    const ERROR_FILE_NOT_FOUND: i32 = 2;
    const ERROR_PIPE_BUSY: i32 = 231;

    let start = std::time::Instant::now();
    loop {
        match ClientOptions::new().open(endpoint) {
            Ok(client) => return Ok(client),
            Err(e)
                if matches!(
                    e.raw_os_error(),
                    Some(ERROR_FILE_NOT_FOUND) | Some(ERROR_PIPE_BUSY)
                ) =>
            {
                if start.elapsed() > CONNECT_TIMEOUT {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "timed out connecting to mpv ipc pipe",
                    ));
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Unix variant: connect to mpv's Unix domain socket, retrying until it exists.
#[cfg(not(windows))]
async fn connect(endpoint: &str) -> std::io::Result<tokio::net::UnixStream> {
    use std::io::ErrorKind;

    let start = std::time::Instant::now();
    loop {
        match tokio::net::UnixStream::connect(endpoint).await {
            Ok(stream) => return Ok(stream),
            Err(e) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused) => {
                if start.elapsed() > CONNECT_TIMEOUT {
                    return Err(std::io::Error::new(
                        ErrorKind::TimedOut,
                        "timed out connecting to mpv ipc socket",
                    ));
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => return Err(e),
        }
    }
}

/// Read newline-delimited JSON from mpv and forward player events to the app.
async fn reader_task<R: AsyncRead + Unpin>(read: R, app_tx: UnboundedSender<AppEvent>) {
    let mut lines = BufReader::new(read).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            continue;
        };

        if let Some(event) = value.get("event").and_then(Value::as_str) {
            let player_event = match event {
                "file-loaded" => Some(PlayerEvent::Loaded),
                "end-file" => {
                    let reason = value.get("reason").and_then(Value::as_str).unwrap_or("other");
                    Some(PlayerEvent::Ended(EndReason::from_mpv(reason)))
                }
                _ => None,
            };
            if let Some(pe) = player_event
                && app_tx.send(AppEvent::Player(pe)).is_err()
            {
                break; // the app is gone
            }
        } else if let Some(request_id) = value.get("request_id").and_then(Value::as_i64) {
            // Only successful replies carry a usable value.
            if value.get("error").and_then(Value::as_str) != Some("success") {
                continue;
            }
            let data = value.get("data");
            let player_event = match request_id {
                REQ_TIME_POS => data.and_then(Value::as_f64).map(PlayerEvent::Position),
                REQ_DURATION => data.and_then(Value::as_f64).map(PlayerEvent::Duration),
                REQ_PAUSE => data.and_then(Value::as_bool).map(PlayerEvent::Paused),
                _ => None,
            };
            if let Some(pe) = player_event
                && app_tx.send(AppEvent::Player(pe)).is_err()
            {
                break;
            }
        }
    }
}

/// Serialise commands to mpv and poll playback state on a timer. Owns the child
/// process and tears it down on exit.
async fn writer_task<W: AsyncWrite + Unpin>(
    mut write: W,
    mut cmd_rx: UnboundedReceiver<MpvCommand>,
    mut child: Child,
) {
    let mut poll = tokio::time::interval(POLL_INTERVAL);
    poll.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            maybe_cmd = cmd_rx.recv() => match maybe_cmd {
                Some(MpvCommand::Quit) | None => break,
                Some(cmd) => {
                    // If mpv has gone away, stop trying.
                    if send_line(&mut write, &command_json(&cmd)).await.is_err() {
                        break;
                    }
                }
            },
            _ = poll.tick() => {
                // Position, duration, and pause — replies are tagged by request id.
                let polls = [
                    get_property_json(REQ_TIME_POS, "time-pos"),
                    get_property_json(REQ_DURATION, "duration"),
                    get_property_json(REQ_PAUSE, "pause"),
                ];
                let mut alive = true;
                for poll_cmd in &polls {
                    if send_line(&mut write, poll_cmd).await.is_err() {
                        alive = false;
                        break;
                    }
                }
                if !alive {
                    break;
                }
            }
        }
    }

    // Politely ask mpv to quit, then make sure the process is really gone.
    let _ = send_line(&mut write, &json!({ "command": ["quit"] })).await;
    let _ = child.start_kill();
    let _ = child.wait().await;
}

/// Translate a high-level command into an mpv IPC JSON message.
fn command_json(command: &MpvCommand) -> Value {
    match command {
        MpvCommand::Load(url) => json!({ "command": ["loadfile", url, "replace"] }),
        MpvCommand::SetPause(paused) => json!({ "command": ["set_property", "pause", paused] }),
        MpvCommand::SetVolume(volume) => json!({ "command": ["set_property", "volume", volume] }),
        MpvCommand::SeekRelative(secs) => json!({ "command": ["seek", secs, "relative"] }),
        MpvCommand::SeekAbsolute(secs) => json!({ "command": ["seek", secs, "absolute"] }),
        MpvCommand::Stop => json!({ "command": ["stop"] }),
        MpvCommand::Quit => json!({ "command": ["quit"] }),
    }
}

/// A `get_property` request tagged with a fixed request id.
fn get_property_json(request_id: i64, property: &str) -> Value {
    json!({ "command": ["get_property", property], "request_id": request_id })
}

/// Write one JSON message followed by the mandatory newline, then flush.
async fn send_line<W: AsyncWrite + Unpin>(write: &mut W, value: &Value) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(value).unwrap_or_default();
    bytes.push(b'\n');
    write.write_all(&bytes).await?;
    write.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_serialise_to_mpv_protocol() {
        assert_eq!(
            command_json(&MpvCommand::Load("http://x".into())),
            json!({ "command": ["loadfile", "http://x", "replace"] })
        );
        assert_eq!(
            command_json(&MpvCommand::SetPause(true)),
            json!({ "command": ["set_property", "pause", true] })
        );
        assert_eq!(
            command_json(&MpvCommand::SetVolume(80)),
            json!({ "command": ["set_property", "volume", 80] })
        );
        assert_eq!(
            get_property_json(REQ_TIME_POS, "time-pos"),
            json!({ "command": ["get_property", "time-pos"], "request_id": 1 })
        );
    }
}
