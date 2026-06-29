<div align="center">

# 静音 · Seion

**quiet sound** — an ultra-lightweight, distraction-free terminal client for YouTube Music

```
        静音
   ─────────────────
      peaceful music
```

*reading a novel by a window in the rain · lofi in Kyoto at night · a quiet tea house*

</div>

---

Seion is not a feature-heavy music player. It is a peaceful music companion. It
aims to feel calm and intentional: keyboard-first, softly coloured, no bright
colours, no sharp borders, no unnecessary animation — silence over complexity.

It is a small native binary (**~1.8 MB**, no runtime, RAM in the low tens of MB)
written in Rust with [ratatui](https://ratatui.rs). Audio is played by **mpv**
over its JSON IPC interface; search and stream resolution use **yt-dlp**. While a
track plays, a soft visualizer breathes quietly behind the now-playing screen.

## Requirements

| Tool | Purpose | Required? |
|------|---------|-----------|
| [yt-dlp](https://github.com/yt-dlp/yt-dlp) | search + stream/download resolution | yes (for anything online) |
| [mpv](https://mpv.io) | audio playback | yes (for sound — Seion runs without it, as a quiet display) |
| [Discord](https://discord.com) | Rich Presence — show what you're listening to | optional |
| Rust ≥ 1.88 (edition 2024) | building | to build from source |

A terminal with **24-bit ("truecolor")** support is recommended — Windows
Terminal, WezTerm, Kitty, Alacritty, Ghostty, foot, iTerm2 all qualify. Without
it the muted palette is approximated.

If mpv is not installed, Seion still opens and lets you browse and search; it
simply tells you, quietly, that audio is unavailable.

## Build & run

```sh
cargo run --release
```

The optimized binary lands at `target/release/seion`.

## A breath, then the interface

On launch you get a single calm screen:

```
        静音

        breathe.
        press enter
```

Press **enter** and it fades into the interface.

## Keys

Everything is keyboard-driven. Press **?** at any time for this list.

| key | |
|-----|-|
| `/` | search |
| `enter` | play · open |
| `space` | pause · resume |
| `j` · `k` | next · previous track |
| `↑` · `↓` | move selection |
| `←` · `→` | seek |
| `+` · `-` | volume |
| `l` | like |
| `a` | add to queue |
| `r` · `m` | repeat · shuffle |
| `d` | download (offline) |
| `h` | home |
| `b` | library |
| `q` | queue |
| `n` | now playing |
| `p` | playlists |
| `s` | settings |
| `f` · `z` | focus · zen mode |
| `w` | toggle idle rain |
| `v` | toggle visualizer |
| `esc` | back |
| `ctrl+c` | quit |

## Themes

Nine calm presets, cycled from **settings** (or set `theme` in the config):
*kyoto night* (default), *sakura dawn*, *bamboo mist*, *rain temple*,
*moon garden*, *autumn maple*, *winter shrine*, *ink wash*, *plum rain*. None are
saturated; none are loud. The settings screen shows a live swatch of the palette
as you cycle, and the same colours flow through the visualizer.

## Discord Rich Presence

With Discord running, Seion shows the song you're listening to — title, artist,
cover art, and a live progress bar — published over Discord's local IPC. It works
out of the box (Seion ships with its own Discord application), and you can toggle
it any time from **settings** (`discord presence`).

To show it under your own application name and art instead, create an app in the
[Discord Developer Portal](https://discord.com/developers/applications) and put
its **Application ID** in `config.toml`:

```toml
discord_presence = true
discord_client_id = "0000000000000000000"   # your Application ID, or "" to disable
```

If Discord isn't running it simply waits and connects when it is. It never blocks
playback and degrades quietly, exactly like mpv.

## Configuration & data

Stored under your platform's standard directories (via `directories`):

- **config** — `config.toml` (theme, volume, search result count, mpv/yt-dlp
  paths, idle rain, visualizer, daily quote, Discord presence + application id).
  Hand-editable; missing keys fall back to defaults, malformed files fall back
  entirely.
- **data** — `seion.db` (SQLite: liked songs + history) and `downloads/`
  (offline audio, played directly like any other track).

## Architecture

Small, modular, async throughout. One unbounded channel of events drives a
single redraw-then-handle loop; slow work (search, stream resolution, downloads)
is spawned and reports back as events.

```
  ui  ←  app (event loop)  →  player  →  mpv  (JSON IPC over a named pipe)
               │                 │
               ├─ youtube   (yt-dlp: search + stream resolution)
               ├─ database  (sqlite: liked, history)
               ├─ discord   (rich presence over local IPC)
               ├─ config (toml) · cache (resolved stream urls, TTL)
               └─ theme · widgets · commands · utils
```

```
src/
  app/        event loop, state, action dispatch
  ui/         one module per screen + the player bar
  widgets/    shared list / progress / input / overlay
  player/     controller + the mpv IPC backend
  youtube/    yt-dlp search & stream resolution
  database/   sqlite persistence
  cache/      stream-url TTL cache
  config/     toml config
  discord/    rich presence over local IPC
  downloads/  offline audio
  lyrics/     (placeholder — no provider in this build)
  theme/      the palette and presets
  models/     shared domain types
  commands/   the keybinding language
  utils/      small pure helpers
```

## Status

The calm core is complete and working: splash, search, playback, queue,
liked/history, downloads & offline playback, now-playing with a soft visualizer,
lyrics view, settings, nine themes, focus/zen modes, idle rain, and optional
Discord Rich Presence. Playlists and a remote lyrics provider are intentionally
left as quiet placeholders.

## License

MIT — see [LICENSE](LICENSE).
