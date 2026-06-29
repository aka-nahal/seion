//! User configuration, persisted as TOML in the platform config directory.
//!
//! The whole struct is `#[serde(default)]`, so a partial or older config file
//! still loads cleanly — any missing key falls back to its default. A malformed
//! file never stops the app; we quietly fall back to defaults.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::utils;

/// Everything the user can tune. Kept intentionally small.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Active theme, by key (e.g. `"kyoto_night"`). See [`crate::theme`].
    pub theme: String,
    /// Playback volume, `0..=100`.
    pub volume: u8,
    /// How many results to request per search.
    pub search_limit: usize,
    /// Command used to launch the audio backend.
    pub mpv_path: String,
    /// Command used to search / resolve streams.
    pub ytdlp_path: String,
    /// Show a daily haiku on the home screen.
    pub daily_quote: bool,
    /// Drift a little rain across the screen when idle.
    pub rain_on_idle: bool,
    /// Breathe a soft visualizer band behind the now-playing screen.
    pub visualizer: bool,
    /// Share the current track on Discord as Rich Presence.
    pub discord_presence: bool,
    /// Discord application id used for Rich Presence. Defaults to Seion's own app
    /// so presence works out of the box; set your own (from
    /// <https://discord.com/developers/applications>) for a custom name and art,
    /// or empty to leave the integration dormant.
    pub discord_client_id: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "kyoto_night".to_string(),
            volume: 70,
            search_limit: 20,
            mpv_path: "mpv".to_string(),
            ytdlp_path: "yt-dlp".to_string(),
            daily_quote: true,
            rain_on_idle: true,
            visualizer: true,
            discord_presence: true,
            discord_client_id: crate::discord::DEFAULT_CLIENT_ID.to_string(),
        }
    }
}

impl Config {
    /// Path to `config.toml` inside the platform config directory, if resolvable.
    pub fn path() -> Option<PathBuf> {
        utils::project_dirs().map(|d| d.config_dir().join("config.toml"))
    }

    /// Load the config, falling back to defaults on any problem (missing file,
    /// unreadable path, or invalid TOML). Always returns something usable.
    pub fn load() -> Self {
        let mut config = match Self::path().and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(text) => toml::from_str(&text).unwrap_or_default(),
            None => Self::default(),
        };
        config.normalize();
        config
    }

    /// Persist the current config to disk, creating the directory if needed.
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path()
            .ok_or_else(|| anyhow::anyhow!("could not resolve a config directory"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Clamp fields into sane ranges so a hand-edited file can't break the app.
    fn normalize(&mut self) {
        self.volume = self.volume.min(100);
        self.search_limit = self.search_limit.clamp(1, 50);
        if self.mpv_path.trim().is_empty() {
            self.mpv_path = "mpv".to_string();
        }
        if self.ytdlp_path.trim().is_empty() {
            self.ytdlp_path = "yt-dlp".to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane() {
        let c = Config::default();
        assert_eq!(c.theme, "kyoto_night");
        assert!(c.volume <= 100);
        assert!((1..=50).contains(&c.search_limit));
    }

    #[test]
    fn partial_toml_keeps_defaults_for_missing_keys() {
        let c: Config = toml::from_str("volume = 42").unwrap();
        assert_eq!(c.volume, 42);
        assert_eq!(c.theme, "kyoto_night"); // untouched -> default
        assert_eq!(c.ytdlp_path, "yt-dlp");
    }

    #[test]
    fn normalize_clamps_out_of_range_values() {
        let mut c: Config = toml::from_str("volume = 250\nsearch_limit = 0").unwrap();
        c.normalize();
        assert_eq!(c.volume, 100);
        assert_eq!(c.search_limit, 1);
    }

    #[test]
    fn round_trips_through_toml() {
        let c = Config::default();
        let text = toml::to_string_pretty(&c).unwrap();
        let back: Config = toml::from_str(&text).unwrap();
        assert_eq!(back.theme, c.theme);
        assert_eq!(back.volume, c.volume);
    }
}
