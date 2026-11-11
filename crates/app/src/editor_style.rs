//! Preferences → Editor (M11-06): font, font size, tab size and line
//! numbers of diffs, blame, file history and the conflict view, and the
//! sizes derived from them.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use iced::Font;

use crate::settings::EditorSettings;

/// Font size and tab size limits of the Preferences.
pub const MIN_FONT_SIZE: u32 = 8;
pub const MAX_FONT_SIZE: u32 = 24;
pub const MIN_TAB_SIZE: u32 = 1;
pub const MAX_TAB_SIZE: u32 = 8;

/// The values the code views are drawn with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EditorStyle {
    pub font: Font,
    pub size: f32,
    pub tab: usize,
    pub numbers: bool,
}

impl Default for EditorStyle {
    fn default() -> Self {
        Self::from_settings(&EditorSettings::default())
    }
}

impl EditorStyle {
    pub fn from_settings(settings: &EditorSettings) -> Self {
        Self {
            font: font_named(&settings.font),
            size: settings.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE) as f32,
            tab: settings.tab_size.clamp(MIN_TAB_SIZE, MAX_TAB_SIZE) as usize,
            numbers: settings.line_numbers,
        }
    }

    /// Height of a diff or conflict row: 20 px at the default 12 px.
    pub fn line_height(&self) -> f32 {
        (self.size * 5.0 / 3.0).round()
    }

    /// Height of a blame row: 18 px at 12 px.
    pub fn blame_row(&self) -> f32 {
        (self.size * 1.5).round()
    }

    /// Width of one line number column (0 when numbers are hidden): 52 px
    /// at 12 px.
    pub fn number_width(&self) -> f32 {
        if self.numbers {
            (self.size * 13.0 / 3.0).round()
        } else {
            0.0
        }
    }

    /// Width of the `+`/`-` marker column: 16 px at 12 px.
    pub fn marker_width(&self) -> f32 {
        (self.size * 4.0 / 3.0).round()
    }

    /// `text` with tabs expanded to the next multiple of the tab size (for
    /// display only).
    pub fn expand<'a>(&self, text: &'a str) -> Cow<'a, str> {
        expand_tabs(text, self.tab)
    }
}

/// `text` with every tab replaced by spaces up to the next tab stop.
pub fn expand_tabs(text: &str, tab: usize) -> Cow<'_, str> {
    if !text.contains('\t') {
        return Cow::Borrowed(text);
    }
    let tab = tab.max(1);
    let mut out = String::with_capacity(text.len() + 8);
    let mut column = 0;
    for c in text.chars() {
        if c == '\t' {
            let spaces = tab - column % tab;
            out.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
        } else {
            out.push(c);
            column = if c == '\n' { 0 } else { column + 1 };
        }
    }
    Cow::Owned(out)
}

/// The font of a family name; empty ("Default") is the platform monospace.
/// iced needs `'static` names: each chosen family is kept once.
pub fn font_named(family: &str) -> Font {
    if family.trim().is_empty() {
        return Font::MONOSPACE;
    }
    static NAMES: OnceLock<Mutex<HashMap<String, &'static str>>> = OnceLock::new();
    let names = NAMES.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut names) = names.lock() else {
        return Font::MONOSPACE;
    };
    let name = *names
        .entry(family.to_owned())
        .or_insert_with(|| Box::leak(family.to_owned().into_boxed_str()));
    Font {
        family: iced::font::Family::Name(name),
        ..Font::MONOSPACE
    }
}

/// Monospace font families of the font database iced already loaded,
/// sorted and without duplicates.
pub fn monospace_families() -> Vec<String> {
    let system = iced::advanced::graphics::text::font_system();
    let Ok(mut system) = system.write() else {
        return Vec::new();
    };
    let mut families: Vec<String> = system
        .raw()
        .db()
        .faces()
        .filter(|face| face.monospaced)
        .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
        .collect();
    families.sort_by_key(|name| name.to_lowercase());
    families.dedup();
    families
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_keep_todays_sizes() {
        let style = EditorStyle::default();
        assert_eq!(style.font, Font::MONOSPACE);
        assert_eq!(style.size, 12.0);
        assert_eq!(style.line_height(), 20.0);
        assert_eq!(style.blame_row(), 18.0);
        assert_eq!(style.number_width(), 52.0);
        assert_eq!(style.marker_width(), 16.0);
        assert_eq!(style.tab, 4);
    }

    #[test]
    fn sizes_follow_the_font_size() {
        let settings = EditorSettings {
            font_size: 18,
            line_numbers: false,
            ..EditorSettings::default()
        };
        let style = EditorStyle::from_settings(&settings);
        assert_eq!(style.line_height(), 30.0);
        assert_eq!(style.blame_row(), 27.0);
        assert_eq!(style.number_width(), 0.0);
        assert_eq!(style.marker_width(), 24.0);
        // Out-of-range values from an edited file are clamped.
        let style = EditorStyle::from_settings(&EditorSettings {
            font_size: 99,
            tab_size: 0,
            ..EditorSettings::default()
        });
        assert_eq!((style.size, style.tab), (24.0, 1));
    }

    #[test]
    fn tabs_expand_to_tab_stops() {
        assert_eq!(expand_tabs("a\tb", 4), "a   b");
        assert_eq!(expand_tabs("\tx", 2), "  x");
        assert_eq!(expand_tabs("abcd\te", 4), "abcd    e");
        assert_eq!(expand_tabs("ab\t\tc", 8), "ab              c");
        assert!(matches!(expand_tabs("plain", 4), Cow::Borrowed(_)));
    }

    #[test]
    fn named_fonts_are_kept_once() {
        let a = font_named("Fira Code");
        let b = font_named("Fira Code");
        assert_eq!(a, b);
        assert_ne!(a, Font::MONOSPACE);
        assert_eq!(font_named(""), Font::MONOSPACE);
    }
}
