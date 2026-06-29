//! The keyboard language of Seion: a semantic [`Action`] enum and a pure
//! function that maps a key press to an action.
//!
//! Keeping this separate from the App makes the bindings easy to read in one
//! place and trivial to test. There are two contexts — normal navigation and
//! text editing (the search box) — selected by the `editing` flag.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Everything the user can ask Seion to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Leave the application.
    Quit,
    /// Go back / close (Esc).
    Back,

    // navigation between views
    /// Focus the search box.
    OpenSearch,
    /// Go to the home screen.
    Home,
    /// Go to the library hub.
    GotoLibrary,
    /// Go to the queue.
    GotoQueue,
    /// Go to the now-playing screen.
    GotoNowPlaying,
    /// Go to settings.
    GotoSettings,
    /// Go to playlists.
    GotoPlaylists,
    /// Open the help overlay.
    Help,

    // list movement
    /// Move the selection up one.
    MoveUp,
    /// Move the selection down one.
    MoveDown,
    /// Move the selection up a page.
    PageUp,
    /// Move the selection down a page.
    PageDown,
    /// Jump the selection to the first row.
    SelectTop,
    /// Jump the selection to the last row.
    SelectBottom,
    /// Act on the current selection (play / open / confirm).
    Activate,

    // playback
    /// Pause or resume.
    TogglePlay,
    /// Stop playback and let the screen fall quiet.
    Stop,
    /// Next track.
    Next,
    /// Previous track.
    Previous,
    /// Seek forwards.
    SeekForward,
    /// Seek backwards.
    SeekBackward,
    /// Raise the volume.
    VolumeUp,
    /// Lower the volume.
    VolumeDown,
    /// Mute / unmute (toggles volume to zero and back).
    ToggleMute,
    /// Like / unlike the focused track.
    ToggleLike,
    /// Add the focused track to the queue.
    Enqueue,
    /// Cycle repeat mode.
    CycleRepeat,
    /// Toggle shuffle.
    ToggleShuffle,
    /// Download the focused track for offline listening.
    Download,

    // ambience
    /// Toggle focus mode (hide everything but the current track).
    FocusMode,
    /// Toggle zen mode (fullscreen calm).
    ZenMode,
    /// Toggle the idle rain.
    ToggleRain,
    /// Toggle the now-playing visualizer.
    ToggleVisualizer,
    /// Cycle to the next colour theme.
    CycleTheme,

    // text editing (search box)
    /// Insert a character.
    InputChar(char),
    /// Delete the character before the cursor.
    InputBackspace,
    /// Move the cursor left.
    InputLeft,
    /// Move the cursor right.
    InputRight,
    /// Move the cursor to the start.
    InputHome,
    /// Move the cursor to the end.
    InputEnd,
    /// Submit the search.
    InputSubmit,
    /// Cancel editing.
    InputCancel,
}

/// Resolve a key press into an [`Action`], given whether we are editing text.
///
/// Returns `None` for keys with no binding in the current context.
pub fn resolve(key: KeyEvent, editing: bool) -> Option<Action> {
    // Ctrl+C always exits, in any context — the one universal escape hatch.
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
    {
        return Some(Action::Quit);
    }

    if editing {
        return resolve_editing(key);
    }

    match key.code {
        KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Down => Some(Action::MoveDown),
        KeyCode::PageUp => Some(Action::PageUp),
        KeyCode::PageDown => Some(Action::PageDown),
        KeyCode::Home => Some(Action::SelectTop),
        KeyCode::End => Some(Action::SelectBottom),
        KeyCode::Left => Some(Action::SeekBackward),
        KeyCode::Right => Some(Action::SeekForward),
        KeyCode::Enter => Some(Action::Activate),
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Char(' ') => Some(Action::TogglePlay),
        KeyCode::Char(c) => resolve_char(c),
        _ => None,
    }
}

/// Map a printable key in normal mode. Letters are matched case-insensitively.
fn resolve_char(c: char) -> Option<Action> {
    match c.to_ascii_lowercase() {
        '/' => Some(Action::OpenSearch),
        '?' => Some(Action::Help),
        'h' => Some(Action::Home),
        'b' => Some(Action::GotoLibrary),
        'q' => Some(Action::GotoQueue),
        'n' => Some(Action::GotoNowPlaying),
        's' => Some(Action::GotoSettings),
        'p' => Some(Action::GotoPlaylists),
        'j' => Some(Action::Next),
        'k' => Some(Action::Previous),
        'l' => Some(Action::ToggleLike),
        'a' => Some(Action::Enqueue),
        'r' => Some(Action::CycleRepeat),
        'm' => Some(Action::ToggleShuffle),
        'x' => Some(Action::ToggleMute),
        't' => Some(Action::CycleTheme),
        '.' => Some(Action::Stop),
        'd' => Some(Action::Download),
        'f' => Some(Action::FocusMode),
        'z' => Some(Action::ZenMode),
        'w' => Some(Action::ToggleRain),
        'v' => Some(Action::ToggleVisualizer),
        '+' | '=' => Some(Action::VolumeUp),
        '-' | '_' => Some(Action::VolumeDown),
        _ => None,
    }
}

/// Map a key while editing the search box.
fn resolve_editing(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char(c) => Some(Action::InputChar(c)),
        KeyCode::Backspace => Some(Action::InputBackspace),
        KeyCode::Left => Some(Action::InputLeft),
        KeyCode::Right => Some(Action::InputRight),
        KeyCode::Home => Some(Action::InputHome),
        KeyCode::End => Some(Action::InputEnd),
        KeyCode::Enter => Some(Action::InputSubmit),
        KeyCode::Esc => Some(Action::InputCancel),
        _ => None,
    }
}

/// A titled group of bindings, shown as one block in the help overlay.
pub struct HelpSection {
    /// The quiet heading (e.g. `"playback"`).
    pub title: &'static str,
    /// The `(key, description)` rows under it.
    pub keys: &'static [(&'static str, &'static str)],
}

/// The bindings, grouped into calm sections — rendered as a two-column
/// cheatsheet by the help overlay.
pub const HELP_SECTIONS: &[HelpSection] = &[
    HelpSection {
        title: "playback",
        keys: &[
            ("space", "pause · resume"),
            (".", "stop"),
            ("j · k", "next · previous"),
            ("← · →", "seek"),
            ("+ · -", "volume"),
            ("x", "mute"),
            ("r · m", "repeat · shuffle"),
        ],
    },
    HelpSection {
        title: "navigate",
        keys: &[
            ("/", "search"),
            ("/playlist", "search playlists"),
            ("↑ · ↓", "move selection"),
            ("home · end", "top · bottom"),
            ("h · b", "home · library"),
            ("q · n", "queue · now playing"),
            ("p · s", "playlists · settings"),
        ],
    },
    HelpSection {
        title: "track",
        keys: &[
            ("enter", "play · open"),
            ("l", "like"),
            ("a", "add to queue"),
            ("d", "download"),
        ],
    },
    HelpSection {
        title: "ambience",
        keys: &[
            ("f · z", "focus · zen"),
            ("w", "idle rain"),
            ("v", "visualizer"),
            ("t", "cycle theme"),
        ],
    },
    HelpSection {
        title: "app",
        keys: &[("?", "this help"), ("esc", "back"), ("ctrl+c", "quit")],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn ctrl_c_quits_in_any_mode() {
        let k = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(resolve(k, false), Some(Action::Quit));
        assert_eq!(resolve(k, true), Some(Action::Quit));
    }

    #[test]
    fn normal_mode_bindings() {
        assert_eq!(resolve(key(KeyCode::Char('/')), false), Some(Action::OpenSearch));
        assert_eq!(resolve(key(KeyCode::Char('j')), false), Some(Action::Next));
        assert_eq!(resolve(key(KeyCode::Char('J')), false), Some(Action::Next)); // case-insensitive
        assert_eq!(resolve(key(KeyCode::Char(' ')), false), Some(Action::TogglePlay));
        assert_eq!(resolve(key(KeyCode::Left), false), Some(Action::SeekBackward));
    }

    #[test]
    fn editing_mode_captures_text() {
        assert_eq!(resolve(key(KeyCode::Char('h')), true), Some(Action::InputChar('h')));
        assert_eq!(resolve(key(KeyCode::Enter), true), Some(Action::InputSubmit));
        assert_eq!(resolve(key(KeyCode::Esc), true), Some(Action::InputCancel));
    }

    #[test]
    fn new_control_bindings() {
        assert_eq!(resolve(key(KeyCode::Char('x')), false), Some(Action::ToggleMute));
        assert_eq!(resolve(key(KeyCode::Char('.')), false), Some(Action::Stop));
        assert_eq!(resolve(key(KeyCode::Char('t')), false), Some(Action::CycleTheme));
        assert_eq!(resolve(key(KeyCode::Home), false), Some(Action::SelectTop));
        assert_eq!(resolve(key(KeyCode::End), false), Some(Action::SelectBottom));
        // Home/End still edit text when the search box is focused.
        assert_eq!(resolve(key(KeyCode::Home), true), Some(Action::InputHome));
        assert_eq!(resolve(key(KeyCode::End), true), Some(Action::InputEnd));
    }
}
