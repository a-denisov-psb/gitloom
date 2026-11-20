//! State of the theme editor in Preferences → Appearance → Manage themes (M17-03, ADR-059):
//! the edited theme, the typed hex values and the open color picker. Pure:
//! the App saves through `theme_file::Op::Save`.

use std::collections::BTreeMap;

use iced::Color;

use crate::highlight::CodeTheme;
use crate::theme::{FeatureColors, ThemeKind, ThemePalette};
use crate::theme_file::{parse_hex, to_hex, ThemeData};

/// An editable color of a theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    Background,
    Surface,
    Chrome,
    Text,
    TextMuted,
    Accent,
    Border,
    Success,
    Warning,
    Danger,
    Inset,
    Hover,
    Selected,
    HeaderBand,
    Backdrop,
    AddedEmphasis,
    RemovedEmphasis,
    Lane(usize),
    Production,
    Hotfix,
    Release,
    Development,
    Unknown,
    Feature(usize),
}

impl Field {
    /// The ten main colors (ADR-058), in editor order.
    pub const MAIN: [Self; 10] = [
        Self::Background,
        Self::Surface,
        Self::Chrome,
        Self::Text,
        Self::TextMuted,
        Self::Accent,
        Self::Border,
        Self::Success,
        Self::Warning,
        Self::Danger,
    ];
    /// The colors `derive` computes (ADR-058).
    pub const MORE: [Self; 7] = [
        Self::Inset,
        Self::Hover,
        Self::Selected,
        Self::HeaderBand,
        Self::Backdrop,
        Self::AddedEmphasis,
        Self::RemovedEmphasis,
    ];
    /// Git flow colors of the branch types.
    pub const FLOW: [Self; 5] = [
        Self::Production,
        Self::Hotfix,
        Self::Release,
        Self::Development,
        Self::Unknown,
    ];

    pub fn label(self) -> String {
        match self {
            Self::Background => "Background".into(),
            Self::Surface => "Surface".into(),
            Self::Chrome => "Chrome".into(),
            Self::Text => "Text".into(),
            Self::TextMuted => "Muted text".into(),
            Self::Accent => "Accent".into(),
            Self::Border => "Border".into(),
            Self::Success => "Success".into(),
            Self::Warning => "Warning".into(),
            Self::Danger => "Danger".into(),
            Self::Inset => "Inset".into(),
            Self::Hover => "Hover".into(),
            Self::Selected => "Selected".into(),
            Self::HeaderBand => "Header band".into(),
            Self::Backdrop => "Backdrop".into(),
            Self::AddedEmphasis => "Added characters".into(),
            Self::RemovedEmphasis => "Removed characters".into(),
            Self::Lane(i) => format!("Lane {}", i + 1),
            Self::Production => "Production".into(),
            Self::Hotfix => "Hotfix".into(),
            Self::Release => "Release".into(),
            Self::Development => "Development".into(),
            Self::Unknown => "Unknown".into(),
            Self::Feature(i) => format!("Feature {}", i + 1),
        }
    }

    /// Alpha is meaningful (ADR-059: backdrop and the emphasis colors).
    pub fn has_alpha(self) -> bool {
        matches!(
            self,
            Self::Backdrop | Self::AddedEmphasis | Self::RemovedEmphasis
        )
    }

    fn slot(self, p: &mut ThemePalette) -> Option<&mut Color> {
        Some(match self {
            Self::Background => &mut p.bg_graph,
            Self::Surface => &mut p.bg_panel,
            Self::Chrome => &mut p.bg_chrome,
            Self::Text => &mut p.text,
            Self::TextMuted => &mut p.text_muted,
            Self::Accent => &mut p.accent,
            Self::Border => &mut p.border,
            Self::Success => &mut p.success,
            Self::Warning => &mut p.warning,
            Self::Danger => &mut p.danger,
            Self::Inset => &mut p.bg_inset,
            Self::Hover => &mut p.bg_hover,
            Self::Selected => &mut p.bg_selected,
            Self::HeaderBand => &mut p.header_band,
            Self::Backdrop => &mut p.backdrop,
            Self::AddedEmphasis => &mut p.diff_added_emphasis,
            Self::RemovedEmphasis => &mut p.diff_removed_emphasis,
            Self::Lane(i) => p.lanes.get_mut(i)?,
            Self::Production => &mut p.flow.production,
            Self::Hotfix => &mut p.flow.hotfix,
            Self::Release => &mut p.flow.release,
            Self::Development => &mut p.flow.development,
            Self::Unknown => &mut p.flow.unknown,
            Self::Feature(_) => return None,
        })
    }
}

/// The color of `field` in `p` (transparent for a missing feature color).
pub fn color_of(p: &ThemePalette, field: Field) -> Color {
    match field {
        Field::Feature(i) => p
            .flow
            .features
            .as_slice()
            .get(i)
            .copied()
            .unwrap_or(Color::TRANSPARENT),
        _ => {
            let mut copy = *p;
            field.slot(&mut copy).map_or(Color::TRANSPARENT, |c| *c)
        }
    }
}

/// An edit in the theme editor.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    Name(String),
    Base(ThemeKind),
    /// Typed hex value of a color.
    Hex(Field, String),
    /// Opens or closes the RGB sliders of a color.
    Picker(Field),
    /// Channel 0 … 3 (R, G, B, A) of a color.
    Channel(Field, usize, u8),
    /// "Derive the other colors from the main colors".
    Derive,
    AddFeature,
    RemoveFeature(usize),
    /// Name of an embedded syntax theme.
    Syntax(String),
}

/// The theme editor.
#[derive(Debug, Clone, PartialEq)]
pub struct Editor {
    /// The user theme being edited (`user:<stem>`).
    pub id: String,
    /// The theme as saved.
    pub saved: ThemeData,
    /// The theme with the edits.
    pub data: ThemeData,
    /// Hex fields while they hold something else than the color.
    texts: BTreeMap<Field, String>,
    /// The color whose sliders are open.
    pub picker: Option<Field>,
    /// A main color changed since the last Derive: offer it.
    pub derive_hint: bool,
}

impl Editor {
    pub fn new(id: String, data: ThemeData) -> Self {
        Self {
            id,
            saved: data.clone(),
            data,
            texts: BTreeMap::new(),
            picker: None,
            derive_hint: false,
        }
    }

    /// The text of a hex field.
    pub fn text(&self, field: Field) -> String {
        self.texts
            .get(&field)
            .cloned()
            .unwrap_or_else(|| to_hex(self.color(field)))
    }

    /// The typed value of `field` is not applied (marked in the view).
    pub fn invalid(&self, field: Field) -> bool {
        self.texts
            .get(&field)
            .is_some_and(|text| hex_for(field, text).is_none())
    }

    pub fn color(&self, field: Field) -> Color {
        color_of(&self.data.palette, field)
    }

    /// There are edits that are not saved.
    pub fn dirty(&self) -> bool {
        self.data != self.saved
    }

    /// Back to the saved theme.
    pub fn revert(&mut self) {
        self.data = self.saved.clone();
        self.texts.clear();
        self.derive_hint = false;
    }

    /// After saving: the edited theme is the saved one.
    pub fn saved(&mut self) {
        self.saved = self.data.clone();
    }

    fn set(&mut self, field: Field, color: Color) {
        let p = &mut self.data.palette;
        match field {
            Field::Feature(i) => {
                let mut colors = p.flow.features.as_slice().to_vec();
                if let Some(slot) = colors.get_mut(i) {
                    *slot = color;
                }
                if let Some(features) = FeatureColors::new(&colors) {
                    p.flow.features = features;
                }
            }
            _ => {
                if let Some(slot) = field.slot(p) {
                    *slot = color;
                }
            }
        }
        if matches!(field, Field::Lane(_)) {
            // Node text follows the lanes (black or white, ADR-059).
            *p = p.with_graph(p.lanes, p.flow);
        }
        if Field::MAIN.contains(&field) {
            self.derive_hint = true;
        }
    }

    pub fn update(&mut self, edit: Edit) {
        match edit {
            Edit::Name(name) => self.data.name = name,
            Edit::Base(base) => self.data.base = base,
            Edit::Hex(field, text) => {
                if let Some(color) = hex_for(field, &text) {
                    self.set(field, color);
                }
                self.texts.insert(field, text);
            }
            Edit::Picker(field) => {
                self.picker = (self.picker != Some(field)).then_some(field);
            }
            Edit::Channel(field, channel, value) => {
                let mut rgba = self.color(field).into_rgba8();
                if let Some(slot) = rgba.get_mut(channel) {
                    *slot = value;
                }
                if !field.has_alpha() {
                    rgba[3] = 255;
                }
                self.set(field, rgba8(rgba));
                self.texts.remove(&field);
            }
            Edit::Derive => {
                let main = self.data.palette.main_colors();
                let derived = ThemePalette::derive(main, self.data.base);
                let p = &mut self.data.palette;
                p.bg_inset = derived.bg_inset;
                p.bg_hover = derived.bg_hover;
                p.bg_selected = derived.bg_selected;
                p.header_band = derived.header_band;
                p.backdrop = derived.backdrop;
                p.diff_added_emphasis = derived.diff_added_emphasis;
                p.diff_removed_emphasis = derived.diff_removed_emphasis;
                for field in Field::MORE {
                    self.texts.remove(&field);
                }
                self.derive_hint = false;
            }
            Edit::AddFeature => {
                let p = &mut self.data.palette;
                let mut colors = p.flow.features.as_slice().to_vec();
                if colors.len() < FeatureColors::MAX {
                    // A lane color not in the list yet, else the last one.
                    let next = p
                        .lanes
                        .iter()
                        .find(|lane| !colors.contains(lane))
                        .copied()
                        .or_else(|| colors.last().copied())
                        .unwrap_or(p.accent);
                    colors.push(next);
                }
                if let Some(features) = FeatureColors::new(&colors) {
                    p.flow.features = features;
                }
                self.clear_feature_texts();
            }
            Edit::RemoveFeature(index) => {
                let p = &mut self.data.palette;
                let mut colors = p.flow.features.as_slice().to_vec();
                if colors.len() > 1 && index < colors.len() {
                    colors.remove(index);
                }
                if let Some(features) = FeatureColors::new(&colors) {
                    p.flow.features = features;
                }
                self.clear_feature_texts();
            }
            Edit::Syntax(name) => {
                if let Some(syntax) = CodeTheme::named(&name) {
                    self.data.syntax = syntax;
                }
            }
        }
    }

    fn clear_feature_texts(&mut self) {
        self.texts.retain(|f, _| !matches!(f, Field::Feature(_)));
        if matches!(self.picker, Some(Field::Feature(_))) {
            self.picker = None;
        }
    }
}

fn rgba8([r, g, b, a]: [u8; 4]) -> Color {
    Color::from_rgba8(r, g, b, f32::from(a) / 255.0)
}

/// A typed value for `field`: `#RRGGBB`, or `#RRGGBBAA` where alpha is
/// meaningful (the `#` may be left out).
pub fn hex_for(field: Field, text: &str) -> Option<Color> {
    let digits = text.trim().trim_start_matches('#').len();
    if digits == 8 && !field.has_alpha() {
        return None;
    }
    parse_hex(text)
}

/// Names of the embedded syntax themes for the Syntax list: Gitloom's own
/// (ADR-060) and the `two-face` set.
pub fn syntax_names() -> Vec<String> {
    let mut names: Vec<String> = crate::highlight::CodeTheme::names()
        .into_iter()
        .map(str::to_owned)
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names
}

/// Contrast warnings of a theme (M17-04, ADR-059); they never block
/// saving.
pub fn warnings(theme: &ThemeData) -> Vec<String> {
    use crate::theme::contrast;
    let p = &theme.palette;
    let mut found = Vec::new();
    let mut check = |what: &str, on: &str, fg: Color, bg: Color, need: f32| {
        let ratio = contrast(fg, bg);
        if ratio < need {
            found.push(format!("{what} on {on}: {ratio:.1}:1 ({need}:1 needed)"));
        }
    };
    for (on, bg) in [
        ("background", p.bg_graph),
        ("surface", p.bg_panel),
        ("chrome", p.bg_chrome),
        ("hover", p.bg_hover),
        ("selected", p.bg_selected),
    ] {
        check("Text", on, p.text, bg, 4.5);
    }
    for (on, bg) in [("background", p.bg_graph), ("surface", p.bg_panel)] {
        check("Muted text", on, p.text_muted, bg, 3.0);
    }
    for (what, color) in [
        ("Accent", p.accent),
        ("Success", p.success),
        ("Warning", p.warning),
        ("Danger", p.danger),
    ] {
        check(what, "background", color, p.bg_graph, 3.0);
    }
    let foreground = syntax_foreground(theme.syntax);
    check("Syntax text", "background", foreground, p.bg_graph, 4.5);
    found
}

/// Default text color of a syntax theme.
pub fn syntax_foreground(syntax: CodeTheme) -> Color {
    crate::highlight::theme(syntax)
        .settings
        .foreground
        .map(|c| rgba8([c.r, c.g, c.b, c.a]))
        .unwrap_or(Color::TRANSPARENT)
}

/// The code of the preview's diff (old and new file).
pub const PREVIEW_OLD: &str =
    "fn total(items: &[u32]) -> u32 {\n    let sum = items.iter().sum();\n    sum\n}\n";
pub const PREVIEW_NEW: &str = "fn total(items: &[u32]) -> u64 {\n    let sum: u64 = items.iter().map(|&n| u64::from(n)).sum();\n    sum\n}\n";

type PreviewCode = (crate::highlight::Outcome, crate::highlight::Outcome);

/// The preview code highlighted with `syntax` (old, new), computed once per
/// syntax theme (a few lines; at most one entry per embedded theme).
pub fn preview_code(syntax: CodeTheme) -> PreviewCode {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock, PoisonError};
    static CACHE: OnceLock<Mutex<HashMap<CodeTheme, PreviewCode>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    let mut cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
    cache
        .entry(syntax)
        .or_insert_with(|| {
            (
                crate::highlight::highlight("preview.rs", PREVIEW_OLD, syntax),
                crate::highlight::highlight("preview.rs", PREVIEW_NEW, syntax),
            )
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::DARK;

    fn editor() -> Editor {
        Editor::new(
            "user:Mine".into(),
            ThemeData {
                name: "Mine".into(),
                base: ThemeKind::Dark,
                palette: DARK,
                syntax: CodeTheme::DARK,
            },
        )
    }

    #[test]
    fn hex_fields() {
        let mut e = editor();
        e.update(Edit::Hex(Field::Accent, "#ff0000".into()));
        assert_eq!(e.color(Field::Accent), Color::from_rgb8(255, 0, 0));
        e.update(Edit::Hex(Field::Accent, "00ff00".into()));
        assert_eq!(e.color(Field::Accent), Color::from_rgb8(0, 255, 0));
        assert!(!e.invalid(Field::Accent));
        // Invalid: marked, not applied.
        for bad in ["#12345", "#0000ff80", "blue", ""] {
            e.update(Edit::Hex(Field::Accent, bad.into()));
            assert!(e.invalid(Field::Accent), "{bad:?}");
            assert_eq!(e.color(Field::Accent), Color::from_rgb8(0, 255, 0));
            assert_eq!(e.text(Field::Accent), bad);
        }
        // Alpha only where it is meaningful.
        e.update(Edit::Hex(Field::Backdrop, "#00000080".into()));
        assert_eq!(
            e.color(Field::Backdrop),
            Color::from_rgba8(0, 0, 0, 128.0 / 255.0)
        );
        assert!(e.dirty());
        e.revert();
        assert!(!e.dirty());
        assert_eq!(e.text(Field::Accent), to_hex(DARK.accent));
    }

    #[test]
    fn derive_changes_only_the_derived_colors() {
        let mut e = editor();
        e.update(Edit::Hex(Field::Background, "#102030".into()));
        assert!(e.derive_hint);
        e.update(Edit::Hex(Field::Lane(0), "#ffcc00".into()));
        e.update(Edit::Syntax("Nord".into()));
        let before = e.data.clone();
        e.update(Edit::Derive);
        assert!(!e.derive_hint);
        let derived = ThemePalette::derive(before.palette.main_colors(), ThemeKind::Dark);
        let p = &e.data.palette;
        assert_eq!(p.bg_selected, derived.bg_selected);
        assert_eq!(p.bg_hover, derived.bg_hover);
        assert_eq!(p.main_colors(), before.palette.main_colors());
        assert_eq!(p.lanes, before.palette.lanes);
        assert_eq!(p.flow, before.palette.flow);
        assert_eq!(e.data.syntax, before.syntax);
    }

    #[test]
    fn feature_list_keeps_one_to_four() {
        let mut e = editor();
        assert_eq!(e.data.palette.flow.features.as_slice().len(), 4);
        e.update(Edit::AddFeature);
        assert_eq!(e.data.palette.flow.features.as_slice().len(), 4);
        for _ in 0..5 {
            e.update(Edit::RemoveFeature(0));
        }
        assert_eq!(e.data.palette.flow.features.as_slice().len(), 1);
        e.update(Edit::Hex(Field::Feature(0), "#123456".into()));
        let one = e.data.palette.flow.features;
        assert!((0..4).all(|i| one.get(i) == Color::from_rgb8(0x12, 0x34, 0x56)));
        e.update(Edit::AddFeature);
        assert_eq!(e.data.palette.flow.features.as_slice().len(), 2);
    }

    #[test]
    fn sliders_and_picker() {
        let mut e = editor();
        e.update(Edit::Picker(Field::Text));
        assert_eq!(e.picker, Some(Field::Text));
        e.update(Edit::Channel(Field::Text, 0, 10));
        e.update(Edit::Channel(Field::Text, 3, 10));
        assert_eq!(e.color(Field::Text).into_rgba8()[0], 10);
        assert_eq!(e.color(Field::Text).a, 1.0, "no alpha for text");
        e.update(Edit::Channel(Field::Backdrop, 3, 51));
        assert_eq!(e.color(Field::Backdrop).into_rgba8()[3], 51);
        e.update(Edit::Picker(Field::Text));
        assert_eq!(e.picker, None);
        assert!(syntax_names().contains(&"OneHalfDark".to_owned()));
        // Gitloom's own syntax themes are listed too (ADR-060).
        for name in ["Dusk", "Studio Dark"] {
            assert!(syntax_names().contains(&name.to_owned()), "{name}");
        }
    }

    /// M17-04: every warning rule at its threshold.
    #[test]
    fn warnings_at_their_thresholds() {
        let base = editor().data;
        assert!(warnings(&base).is_empty(), "{:?}", warnings(&base));
        let gray = |v: u8| Color::from_rgb8(v, v, v);
        let black = gray(0);
        // The darkest gray that reaches `need` on black.
        let at = |need: f32| {
            (0..=255u8)
                .find(|v| crate::theme::contrast(gray(*v), black) >= need)
                .unwrap()
        };
        let mut t = base.clone();
        let p = &mut t.palette;
        p.bg_graph = black;
        p.bg_panel = black;
        p.bg_chrome = black;
        p.bg_hover = black;
        p.bg_selected = black;
        p.text = gray(at(4.5));
        p.text_muted = gray(at(3.0));
        p.accent = gray(at(3.0));
        p.success = gray(at(3.0));
        p.warning = gray(at(3.0));
        p.danger = gray(at(3.0));
        assert!(warnings(&t).is_empty(), "{:?}", warnings(&t));
        type Slot = fn(&mut ThemePalette) -> &mut Color;
        let cases: [(&str, Slot, f32); 6] = [
            ("Text on background", |p| &mut p.text, 4.5),
            ("Muted text on background", |p| &mut p.text_muted, 3.0),
            ("Accent on background", |p| &mut p.accent, 3.0),
            ("Success on background", |p| &mut p.success, 3.0),
            ("Warning on background", |p| &mut p.warning, 3.0),
            ("Danger on background", |p| &mut p.danger, 3.0),
        ];
        for (start, slot, need) in cases {
            let mut bad = t.clone();
            *slot(&mut bad.palette) = gray(at(need) - 1);
            let found = warnings(&bad);
            assert!(
                found.iter().any(|w| w.starts_with(start)),
                "{start}: {found:?}"
            );
        }
        for (start, slot) in [
            ("Text on surface", (|p| &mut p.bg_panel) as Slot),
            ("Text on chrome", |p| &mut p.bg_chrome),
            ("Text on hover", |p| &mut p.bg_hover),
            ("Text on selected", |p| &mut p.bg_selected),
            ("Muted text on surface", |p| &mut p.bg_panel),
        ] {
            let mut bad = t.clone();
            *slot(&mut bad.palette) = gray(40);
            let found = warnings(&bad);
            assert!(
                found.iter().any(|w| w.starts_with(start)),
                "{start}: {found:?}"
            );
        }
        // The syntax theme's text on the background.
        let mut bad = base.clone();
        bad.palette.bg_graph = syntax_foreground(bad.syntax);
        let found = warnings(&bad);
        assert!(
            found
                .iter()
                .any(|w| w.starts_with("Syntax text on background")),
            "{found:?}"
        );
    }
}
