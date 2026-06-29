//! Small, pure helpers used across the app: time formatting, text truncation,
//! platform paths, colour interpolation, a tiny RNG for shuffle, and the daily
//! haiku. Nothing here does I/O beyond resolving well-known directories.

use std::time::{SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;
use ratatui::style::Color;

/// Format a whole number of seconds as `mm:ss`, or `h:mm:ss` past an hour.
pub fn format_duration(total_secs: u64) -> String {
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// Format a (possibly fractional, possibly negative) position in seconds.
pub fn format_position(secs: f64) -> String {
    format_duration(secs.max(0.0) as u64)
}

/// Truncate to at most `max` characters, appending a single-character ellipsis
/// when something was cut. Operates on `char`s, which is good enough for the
/// calm latin/CJK mix we display (ratatui clips anything still too wide).
pub fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let keep = max.saturating_sub(1);
    let mut out: String = s.chars().take(keep).collect();
    out.push('…');
    out
}

/// The platform directories for Seion (config / data / cache), if resolvable.
pub fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", "seion")
}

/// The current wall-clock time as Unix epoch milliseconds. Used for the Discord
/// presence timestamps; falls back to `0` if the clock is somehow before 1970.
pub fn now_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Linearly interpolate between two truecolor values (`t` clamped to `0..=1`).
///
/// Used for the gentle splash fade-in. Non-`Rgb` colours fall through to `to`.
pub fn lerp_color(from: Color, to: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (from, to) {
        (Color::Rgb(fr, fg, fb), Color::Rgb(tr, tg, tb)) => {
            let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
            Color::Rgb(lerp(fr, tr), lerp(fg, tg), lerp(fb, tb))
        }
        _ => to,
    }
}

/// The mpv IPC endpoint for this process — a Windows named pipe or a Unix
/// domain socket, made unique per PID so concurrent Seion sessions never clash.
pub fn ipc_endpoint() -> String {
    let pid = std::process::id();
    #[cfg(windows)]
    {
        format!(r"\\.\pipe\seion-mpv-{pid}")
    }
    #[cfg(not(windows))]
    {
        let base = std::env::temp_dir();
        base.join(format!("seion-mpv-{pid}.sock"))
            .to_string_lossy()
            .into_owned()
    }
}

/// A minimal xorshift64* RNG — enough for shuffle and picking a daily quote,
/// with zero dependencies. Not cryptographic; never used for anything sensitive.
pub struct Rng(u64);

impl Rng {
    /// Seed from the wall clock. Two instances created in the same nanosecond
    /// would share a sequence — irrelevant for shuffling a playlist.
    pub fn from_clock() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self::seeded(nanos ^ 0x9E37_79B9_7F4A_7C15)
    }

    /// Seed explicitly (handy for deterministic tests / the daily quote).
    pub fn seeded(seed: u64) -> Self {
        Self(if seed == 0 { 0xDEAD_BEEF_CAFE_F00D } else { seed })
    }

    /// Next pseudo-random `u64`.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A value in `0..n` (returns 0 when `n == 0`).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }
}

/// A small, hand-picked set of Japanese proverbs and haiku, shown on startup.
/// Each is the original followed by a quiet translation.
pub const HAIKU: &[(&str, &str)] = &[
    ("七転び八起き", "fall seven times, rise eight"),
    ("一期一会", "one time, one meeting — treasure this moment"),
    ("古池や蛙飛び込む水の音", "an old pond — a frog leaps in, the sound of water"),
    ("塵も積もれば山となる", "even dust, piled up, becomes a mountain"),
    ("急がば回れ", "when in haste, take the long way round"),
    ("花は桜木人は武士", "among flowers the cherry, among people the quiet heart"),
    ("明日は明日の風が吹く", "tomorrow the wind will blow as tomorrow's wind"),
    ("月日は百代の過客", "the days and months are travelers of eternity"),
    ("静けさや岩にしみ入る蝉の声", "such stillness — the cicada's cry sinks into the rocks"),
    ("継続は力なり", "to continue is itself a kind of strength"),
];

/// The quote for today — stable across a day, gently different each morning.
pub fn daily_quote() -> (&'static str, &'static str) {
    let days = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0);
    HAIKU[(days as usize) % HAIKU.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_format() {
        assert_eq!(format_duration(0), "00:00");
        assert_eq!(format_duration(137), "02:17");
        assert_eq!(format_duration(3661), "1:01:01");
    }

    #[test]
    fn truncate_adds_ellipsis_only_when_needed() {
        assert_eq!(truncate("lofi", 10), "lofi");
        assert_eq!(truncate("lofi jazz", 5), "lofi…");
        assert_eq!(truncate("anything", 0), "");
    }

    #[test]
    fn rng_below_is_in_range() {
        let mut rng = Rng::seeded(42);
        for _ in 0..1000 {
            assert!(rng.below(10) < 10);
        }
        assert_eq!(rng.below(0), 0);
    }

    #[test]
    fn daily_quote_is_in_the_set() {
        let q = daily_quote();
        assert!(HAIKU.contains(&q));
    }
}
