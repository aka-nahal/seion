//! Theme — the visual soul of 静音.
//!
//! Every colour here is muted on purpose. No neon, no pure white, no bright
//! blue, no red unless something is genuinely wrong. The default, *Kyoto
//! Night*, is the palette described in the project brief; the other presets
//! are variations in the same quiet spirit — paper, fog, rain, tatami, tea.
//!
//! A [`Theme`] is just data: a small bag of [`Color`]s. The widgets in
//! [`crate::ui`] turn these into concrete [`ratatui::style::Style`]s, which
//! keeps the palette free of any layout concerns.

use ratatui::style::Color;

/// A complete colour palette. Cheap to copy (it is eleven `Color`s).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Human-facing, lowercase name (e.g. `"kyoto night"`).
    pub name: &'static str,
    /// The deepest backdrop — the night outside the window.
    pub background: Color,
    /// A hair lighter than the background, for resting surfaces.
    pub surface: Color,
    /// Raised panels and the player bar.
    pub panel: Color,
    /// Primary text — warm paper, never pure white.
    pub text: Color,
    /// Secondary / dimmed text.
    pub muted: Color,
    /// The primary accent — sage green.
    pub accent: Color,
    /// A second, softer accent — moss.
    pub secondary: Color,
    /// Warm beige used to make a single thing glow gently.
    pub highlight: Color,
    /// The selection wash behind the focused list row.
    pub selection: Color,
    /// Quiet confirmation.
    pub success: Color,
    /// Muted clay — used sparingly, only for real errors.
    pub error: Color,
}

/// `const`-friendly hex helper so the palettes read like the brief.
const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    /// The default palette from the brief — a Kyoto tea house on a rainy night.
    pub const fn kyoto_night() -> Self {
        Self {
            name: "kyoto night",
            background: rgb(0x11, 0x13, 0x15),
            surface: rgb(0x1A, 0x1D, 0x1E),
            panel: rgb(0x22, 0x27, 0x28),
            text: rgb(0xEC, 0xE8, 0xDF),
            muted: rgb(0xA9, 0xA7, 0x9D),
            accent: rgb(0x8B, 0xA8, 0x88),
            secondary: rgb(0x6D, 0x8F, 0x72),
            highlight: rgb(0xD8, 0xCB, 0xB5),
            selection: rgb(0x6E, 0x8C, 0x78),
            success: rgb(0x8F, 0xB9, 0x8A),
            error: rgb(0xB7, 0x7A, 0x6B),
        }
    }

    /// First light through paper screens — faintly warm, faintly pink.
    pub const fn sakura_dawn() -> Self {
        Self {
            name: "sakura dawn",
            background: rgb(0x17, 0x14, 0x16),
            surface: rgb(0x20, 0x1B, 0x1E),
            panel: rgb(0x2A, 0x23, 0x27),
            text: rgb(0xF0, 0xE6, 0xE6),
            muted: rgb(0xB2, 0xA1, 0xA4),
            accent: rgb(0xC9, 0x9A, 0xA6),
            secondary: rgb(0xA8, 0x86, 0x90),
            highlight: rgb(0xE3, 0xC9, 0xC2),
            selection: rgb(0x8C, 0x6E, 0x77),
            success: rgb(0x9A, 0xB8, 0x9A),
            error: rgb(0xB7, 0x7A, 0x6B),
        }
    }

    /// Green light filtered through a bamboo grove in mist.
    pub const fn bamboo_mist() -> Self {
        Self {
            name: "bamboo mist",
            background: rgb(0x10, 0x14, 0x12),
            surface: rgb(0x18, 0x1E, 0x1A),
            panel: rgb(0x21, 0x29, 0x24),
            text: rgb(0xE6, 0xEC, 0xE0),
            muted: rgb(0x9F, 0xAA, 0x9D),
            accent: rgb(0x88, 0xA8, 0x8E),
            secondary: rgb(0x6F, 0x8E, 0x76),
            highlight: rgb(0xCD, 0xD8, 0xBE),
            selection: rgb(0x6E, 0x8C, 0x74),
            success: rgb(0x8F, 0xB9, 0x8A),
            error: rgb(0xB0, 0x82, 0x70),
        }
    }

    /// Cold stone and water at a temple in the rain.
    pub const fn rain_temple() -> Self {
        Self {
            name: "rain temple",
            background: rgb(0x0F, 0x12, 0x14),
            surface: rgb(0x16, 0x1B, 0x1E),
            panel: rgb(0x1F, 0x25, 0x29),
            text: rgb(0xE3, 0xE7, 0xE8),
            muted: rgb(0x97, 0xA1, 0xA6),
            accent: rgb(0x84, 0xA0, 0xA8),
            secondary: rgb(0x6C, 0x86, 0x8E),
            highlight: rgb(0xC4, 0xCE, 0xD2),
            selection: rgb(0x5E, 0x7A, 0x82),
            success: rgb(0x88, 0xB2, 0xA6),
            error: rgb(0xB1, 0x7E, 0x72),
        }
    }

    /// A moonlit garden — silver-blue and very still.
    pub const fn moon_garden() -> Self {
        Self {
            name: "moon garden",
            background: rgb(0x10, 0x12, 0x18),
            surface: rgb(0x17, 0x1A, 0x22),
            panel: rgb(0x20, 0x24, 0x2D),
            text: rgb(0xE4, 0xE6, 0xEC),
            muted: rgb(0x9C, 0x9F, 0xAD),
            accent: rgb(0x96, 0xA0, 0xC0),
            secondary: rgb(0x77, 0x80, 0x9E),
            highlight: rgb(0xC8, 0xCC, 0xDE),
            selection: rgb(0x66, 0x6F, 0x8E),
            success: rgb(0x8F, 0xB9, 0xA8),
            error: rgb(0xB0, 0x82, 0x82),
        }
    }

    /// Late autumn — faded maple, amber and rust, kept soft.
    pub const fn autumn_maple() -> Self {
        Self {
            name: "autumn maple",
            background: rgb(0x16, 0x12, 0x10),
            surface: rgb(0x1F, 0x19, 0x15),
            panel: rgb(0x29, 0x21, 0x1B),
            text: rgb(0xEE, 0xE4, 0xD8),
            muted: rgb(0xAE, 0xA0, 0x90),
            accent: rgb(0xC2, 0x95, 0x6E),
            secondary: rgb(0x9C, 0x7A, 0x5C),
            highlight: rgb(0xDD, 0xC6, 0xA4),
            selection: rgb(0x84, 0x63, 0x4C),
            success: rgb(0x9E, 0xB0, 0x7E),
            error: rgb(0xB7, 0x6F, 0x5C),
        }
    }

    /// A shrine under snow — cool, pale, and almost silent.
    pub const fn winter_shrine() -> Self {
        Self {
            name: "winter shrine",
            background: rgb(0x12, 0x14, 0x16),
            surface: rgb(0x1B, 0x1E, 0x21),
            panel: rgb(0x25, 0x29, 0x2D),
            text: rgb(0xE9, 0xEC, 0xEF),
            muted: rgb(0xA3, 0xA8, 0xAE),
            accent: rgb(0x9D, 0xAE, 0xB2),
            secondary: rgb(0x7E, 0x8E, 0x93),
            highlight: rgb(0xD6, 0xDD, 0xE0),
            selection: rgb(0x6A, 0x7A, 0x80),
            success: rgb(0x8F, 0xB9, 0xA2),
            error: rgb(0xAE, 0x84, 0x7C),
        }
    }

    /// Sumi-e — wet ink on warm paper, almost without colour.
    pub const fn ink_wash() -> Self {
        Self {
            name: "ink wash",
            background: rgb(0x12, 0x12, 0x11),
            surface: rgb(0x1A, 0x1A, 0x18),
            panel: rgb(0x23, 0x23, 0x20),
            text: rgb(0xE6, 0xE3, 0xDA),
            muted: rgb(0x9C, 0x99, 0x90),
            accent: rgb(0xA8, 0xA2, 0x96),
            secondary: rgb(0x82, 0x7E, 0x74),
            highlight: rgb(0xCF, 0xC8, 0xB8),
            selection: rgb(0x6A, 0x66, 0x5C),
            success: rgb(0x93, 0xA8, 0x90),
            error: rgb(0xB0, 0x82, 0x74),
        }
    }

    /// Plum rain (梅雨) — the early-summer downpour, indigo deepening to violet.
    pub const fn plum_rain() -> Self {
        Self {
            name: "plum rain",
            background: rgb(0x12, 0x10, 0x16),
            surface: rgb(0x1B, 0x17, 0x20),
            panel: rgb(0x25, 0x20, 0x2C),
            text: rgb(0xE8, 0xE2, 0xEC),
            muted: rgb(0xA6, 0x9F, 0xB0),
            accent: rgb(0xA0, 0x8C, 0xC0),
            secondary: rgb(0x80, 0x70, 0xA0),
            highlight: rgb(0xCF, 0xC2, 0xDC),
            selection: rgb(0x6C, 0x5C, 0x86),
            success: rgb(0x8F, 0xB9, 0xA8),
            error: rgb(0xB8, 0x7C, 0x86),
        }
    }

    /// Every preset, in display order. Used by Settings to cycle themes.
    pub const PRESETS: [fn() -> Theme; 9] = [
        Theme::kyoto_night,
        Theme::sakura_dawn,
        Theme::bamboo_mist,
        Theme::rain_temple,
        Theme::moon_garden,
        Theme::autumn_maple,
        Theme::winter_shrine,
        Theme::ink_wash,
        Theme::plum_rain,
    ];

    /// Resolve a theme by (case-insensitive) name, falling back to the default.
    ///
    /// Accepts either the spaced display name (`"kyoto night"`) or a
    /// `snake_case` config key (`"kyoto_night"`).
    pub fn from_name(name: &str) -> Self {
        let key = name.trim().to_ascii_lowercase().replace(['_', '-'], " ");
        Self::PRESETS
            .iter()
            .map(|f| f())
            .find(|t| t.name == key)
            .unwrap_or_else(Self::kyoto_night)
    }

    /// The next preset after this one, wrapping — for "cycle theme" in Settings.
    pub fn next(self) -> Self {
        let idx = Self::PRESETS
            .iter()
            .map(|f| f())
            .position(|t| t.name == self.name)
            .unwrap_or(0);
        Self::PRESETS[(idx + 1) % Self::PRESETS.len()]()
    }

    /// The config key form of the name (`"kyoto night"` -> `"kyoto_night"`).
    pub fn key(&self) -> String {
        self.name.replace(' ', "_")
    }

    /// Adapt the palette to the terminal's colour depth. On a 24-bit
    /// ("truecolor") terminal this is the identity — the RGB palette is used as
    /// written. When `truecolor` is false every colour is mapped to the nearest
    /// entry in the 256-colour xterm palette (`Color::Indexed`), so the muted
    /// look degrades in a controlled way rather than being left to the
    /// terminal's own (often harsher) RGB approximation. The `name` is kept, so
    /// cycling and config round-tripping still work on an adapted theme.
    pub fn adapt(self, truecolor: bool) -> Self {
        if truecolor {
            return self;
        }
        let d = degrade;
        Self {
            name: self.name,
            background: d(self.background),
            surface: d(self.surface),
            panel: d(self.panel),
            text: d(self.text),
            muted: d(self.muted),
            accent: d(self.accent),
            secondary: d(self.secondary),
            highlight: d(self.highlight),
            selection: d(self.selection),
            success: d(self.success),
            error: d(self.error),
        }
    }
}

/// Map a colour to the nearest xterm-256 palette entry. Non-RGB colours (already
/// indexed or named) are returned unchanged.
fn degrade(color: Color) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    Color::Indexed(nearest_256(r, g, b))
}

/// The six rung values of the 6×6×6 colour cube used by the xterm-256 palette.
const CUBE_STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];

/// Index (and snapped value) of the nearest cube rung to `v`.
fn nearest_cube_step(v: u8) -> (u8, u8) {
    let mut best = 0usize;
    let mut best_dist = u16::MAX;
    for (i, &step) in CUBE_STEPS.iter().enumerate() {
        let dist = (step as i16 - v as i16).unsigned_abs();
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    (best as u8, CUBE_STEPS[best])
}

/// Squared euclidean distance between two RGB triples.
fn rgb_dist(a: (u8, u8, u8), b: (u8, u8, u8)) -> u32 {
    let d = |x: u8, y: u8| {
        let v = x as i32 - y as i32;
        (v * v) as u32
    };
    d(a.0, b.0) + d(a.1, b.1) + d(a.2, b.2)
}

/// The nearest xterm-256 index for an RGB colour, choosing between the colour
/// cube (16–231) and the 24-step grey ramp (232–255), whichever lands closer.
fn nearest_256(r: u8, g: u8, b: u8) -> u8 {
    let (ri, rv) = nearest_cube_step(r);
    let (gi, gv) = nearest_cube_step(g);
    let (bi, bv) = nearest_cube_step(b);
    let cube_index = 16 + 36 * ri + 6 * gi + bi;
    let cube_dist = rgb_dist((r, g, b), (rv, gv, bv));

    // Grey ramp: index 232+i carries the value 8 + 10*i.
    let grey_avg = ((r as u16 + g as u16 + b as u16) / 3) as u8;
    let mut grey_i = 0u8;
    let mut grey_best = i16::MAX;
    for i in 0..24u8 {
        let value = 8 + 10 * i as i16;
        let dist = (value - grey_avg as i16).abs();
        if dist < grey_best {
            grey_best = dist;
            grey_i = i;
        }
    }
    let grey_value = 8 + 10 * grey_i;
    let grey_dist = rgb_dist((r, g, b), (grey_value, grey_value, grey_value));

    if grey_dist < cube_dist {
        232 + grey_i
    } else {
        cube_index
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::kyoto_night()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_kyoto_night() {
        assert_eq!(Theme::default().name, "kyoto night");
    }

    #[test]
    fn from_name_is_forgiving() {
        assert_eq!(Theme::from_name("Kyoto Night").name, "kyoto night");
        assert_eq!(Theme::from_name("kyoto_night").name, "kyoto night");
        assert_eq!(Theme::from_name("rain-temple").name, "rain temple");
        // unknown -> default
        assert_eq!(Theme::from_name("neon city").name, "kyoto night");
    }

    #[test]
    fn cycling_wraps_and_round_trips() {
        let first = Theme::kyoto_night();
        // Stepping forward through every preset returns to the start.
        let mut t = first;
        for _ in 0..Theme::PRESETS.len() {
            t = t.next();
        }
        assert_eq!(t.name, first.name);
    }

    #[test]
    fn key_round_trips_through_from_name() {
        for make in Theme::PRESETS {
            let t = make();
            assert_eq!(Theme::from_name(&t.key()).name, t.name);
        }
    }

    #[test]
    fn adapt_is_identity_on_truecolor() {
        let t = Theme::kyoto_night();
        assert_eq!(t.adapt(true), t);
    }

    #[test]
    fn adapt_downsamples_to_indexed_and_keeps_name() {
        let t = Theme::kyoto_night().adapt(false);
        assert_eq!(t.name, "kyoto night"); // name survives, so cycling still works
        // Every colour becomes an indexed palette entry off truecolor.
        for c in [t.background, t.text, t.accent, t.error, t.highlight] {
            assert!(matches!(c, Color::Indexed(_)), "expected indexed, got {c:?}");
        }
        // An adapted theme can still be cycled (lookup is by name).
        assert_eq!(t.next().name, Theme::kyoto_night().next().name);
    }

    #[test]
    fn nearest_256_maps_extremes() {
        assert_eq!(nearest_256(0, 0, 0), 16); // black -> cube origin
        assert_eq!(nearest_256(255, 255, 255), 231); // white -> cube apex
        // A pure cube primary lands on its exact cube cell.
        assert_eq!(nearest_256(255, 0, 0), 196);
    }
}
