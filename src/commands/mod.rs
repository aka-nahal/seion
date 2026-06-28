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
    /// Act on the current selection (play / open / confirm).
    Activate,

    // playback
    /// Pause or resume.
    TogglePlay,
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
        'd' => Some(Action::Download),
        'f' => Some(Action::FocusMode),
        'z' => Some(Action::ZenMode),
        'w' => Some(Action::ToggleRain),
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

/// The bindings, paired with a quiet description — shown in the help overlay.
pub const HELP: &[(&str, &str)] = &[
    ("/", "search"),
    ("enter", "play · open"),
    ("space", "pause · resume"),
    ("j · k", "next · previous track"),
    ("↑ · ↓", "move selection"),
    ("← · →", "seek"),
    ("+ · -", "volume"),
    ("l", "like"),
    ("a", "add to queue"),
    ("r · m", "repeat · shuffle"),
    ("d", "download"),
    ("h", "home"),
    ("b", "library"),
    ("q", "queue"),
    ("n", "now playing"),
    ("p", "playlists"),
    ("s", "settings"),
    ("f · z", "focus · zen mode"),
    ("w", "toggle rain"),
    ("?", "this help"),
    ("esc", "back"),
    ("ctrl+c", "quit"),
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
}
