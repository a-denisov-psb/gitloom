//! Theme files (ADR-059): one JSON file per user theme in `themes/` next
//! to `settings.json`; the id is `user:<file stem>`. Parsing, validation,
//! filling gaps and writing are pure; [`load_dir`] reads a folder and is
//! blocking (call it off the UI thread).
//!
//! Format 1:
//!
//! ```json
//! {
//!   "format": 1,
//!   "name": "My theme",
//!   "base": "dark",
//!   "syntax": "OneHalfDark",
//!   "colors": { "background": "#1f232b", "surface": "#252a33", "chrome": "#181b21",
//!               "text": "#d5d9e0", "text_muted": "#8a93a3", "accent": "#4f9dde",
//!               "border": "#343a46", "success": "#5fb878", "warning": "#e5c07b",
//!               "danger": "#e06c75" },
//!   "lanes": ["#4f9dde", "…eight colors"],
//!   "flow": { "production": "#5fb878", "hotfix": "#56b6c2", "release": "#4f9dde",
//!             "development": "#c678dd", "unknown": "#8a93a3",
//!             "features": ["#e86fb0"] }
//! }
//! ```
//!
//! Required: `name`, `base` and the ten main colors. Optional colors
//! (`inset`, `hover`, `selected`, `header_band`, `backdrop`,
//! `diff_added_emphasis`, `diff_removed_emphasis`) come from
//! `ThemePalette::derive`; missing `lanes`, `flow` and `syntax` from the
//! built-in theme of the same base. Unknown keys are ignored.

// The file actions of Preferences → Themes use the rest (M17-02).
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use iced::Color;
use serde::{Deserialize, Serialize};

use crate::highlight::CodeTheme;
use crate::theme::{FeatureColors, FlowColors, MainColors, ThemeKind, ThemePalette};
use crate::themes::ThemeDef;

/// The format this version reads and writes.
pub const FORMAT: u32 = 1;
/// Folder of the theme files inside the config directory.
pub const DIR: &str = "themes";
/// Prefix of the ids of user themes.
pub const ID_PREFIX: &str = "user:";

/// Id of the theme in the file with stem `stem`.
pub fn id_for(stem: &str) -> String {
    format!("{ID_PREFIX}{stem}")
}

/// The file stem of a user theme id.
pub fn stem_of(id: &str) -> Option<&str> {
    id.strip_prefix(ID_PREFIX)
}

/// A file stem for a theme name: characters that are not allowed in file
/// names on any platform become `-`; an empty result is "theme".
pub fn stem_for_name(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let stem = stem.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if stem.is_empty() {
        "theme".to_owned()
    } else {
        stem.to_owned()
    }
}

/// `stem`, or `stem 2`, `stem 3`, … when it is taken (compared without
/// regard to case, like the file systems of Windows and macOS).
pub fn unique_stem(stem: &str, taken: &[String]) -> String {
    let free = |candidate: &str| !taken.iter().any(|t| t.eq_ignore_ascii_case(candidate));
    if free(stem) {
        return stem.to_owned();
    }
    (2..)
        .map(|n| format!("{stem} {n}"))
        .find(|candidate| free(candidate))
        .unwrap_or_else(|| stem.to_owned())
}

/// `#RRGGBB` or `#RRGGBBAA` (the `#` may be left out); anything else is
/// `None`.
pub fn parse_hex(text: &str) -> Option<Color> {
    let digits = text.trim();
    let digits = digits.strip_prefix('#').unwrap_or(digits);
    if !(digits.len() == 6 || digits.len() == 8) || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    let (r, g, b) = (byte(0)?, byte(2)?, byte(4)?);
    let a = if digits.len() == 8 { byte(6)? } else { 255 };
    Some(Color::from_rgba8(r, g, b, f32::from(a) / 255.0))
}

/// `#rrggbb`, or `#rrggbbaa` when the color is not opaque.
pub fn to_hex(color: Color) -> String {
    let [r, g, b, a] = color.into_rgba8();
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct FileFormat {
    format: Option<u32>,
    name: Option<String>,
    base: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    syntax: Option<String>,
    #[serde(default)]
    colors: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lanes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flow: Option<FlowFormat>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct FlowFormat {
    production: Option<String>,
    hotfix: Option<String>,
    release: Option<String>,
    development: Option<String>,
    unknown: Option<String>,
    features: Option<Vec<String>>,
}

/// The main color keys in file order.
const MAIN_KEYS: [&str; 10] = [
    "background",
    "surface",
    "chrome",
    "text",
    "text_muted",
    "accent",
    "border",
    "success",
    "warning",
    "danger",
];

/// A theme read from a file, without its id (that comes from the file
/// name).
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeData {
    pub name: String,
    pub base: ThemeKind,
    pub palette: ThemePalette,
    pub syntax: CodeTheme,
}

impl ThemeData {
    /// The registry entry with `id`.
    pub fn into_def(self, id: String) -> ThemeDef {
        ThemeDef {
            id,
            name: self.name,
            base: self.base,
            palette: Arc::new(self.palette),
            syntax: self.syntax,
        }
    }

    /// The data of a registry entry.
    pub fn of(def: &ThemeDef) -> Self {
        Self {
            name: def.name.clone(),
            base: def.base,
            palette: *def.palette,
            syntax: def.syntax,
        }
    }
}

fn color(key: &str, value: &str) -> Result<Color, String> {
    parse_hex(value).ok_or_else(|| format!("{key}: '{value}' is not a color (#RRGGBB)"))
}

/// Reads a theme file's text (UTF-8, with or without BOM).
pub fn parse(text: &str) -> Result<ThemeData, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let file: FileFormat =
        serde_json::from_str(text).map_err(|e| format!("not a theme file: {e}"))?;
    if let Some(format) = file.format {
        if format != FORMAT {
            return Err(format!(
                "format {format} is not supported (this version reads {FORMAT})"
            ));
        }
    }
    let name = file
        .name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .ok_or("\"name\" is missing")?;
    let base = match file.base.as_deref() {
        Some("dark") => ThemeKind::Dark,
        Some("light") => ThemeKind::Light,
        Some(other) => return Err(format!("base: '{other}' is neither \"dark\" nor \"light\"")),
        None => return Err("\"base\" is missing".to_owned()),
    };
    let main_color = |key: &str| -> Result<Color, String> {
        let value = file
            .colors
            .get(key)
            .ok_or_else(|| format!("colors.{key} is missing"))?;
        color(&format!("colors.{key}"), value)
    };
    let main = MainColors {
        background: main_color("background")?,
        surface: main_color("surface")?,
        chrome: main_color("chrome")?,
        text: main_color("text")?,
        text_muted: main_color("text_muted")?,
        accent: main_color("accent")?,
        border: main_color("border")?,
        success: main_color("success")?,
        warning: main_color("warning")?,
        danger: main_color("danger")?,
    };
    let defaults = crate::themes::default_of(base);
    let lanes = match &file.lanes {
        Some(list) => {
            if list.len() != 8 {
                return Err(format!("lanes: {} colors, 8 needed", list.len()));
            }
            let mut lanes = defaults.palette.lanes;
            for (i, value) in list.iter().enumerate() {
                lanes[i] = color(&format!("lanes[{i}]"), value)?;
            }
            lanes
        }
        None => defaults.palette.lanes,
    };
    let flow = match &file.flow {
        Some(flow) => {
            let d = defaults.palette.flow;
            let one = |key: &str, value: &Option<String>, default: Color| match value {
                Some(value) => color(&format!("flow.{key}"), value),
                None => Ok(default),
            };
            let features = match &flow.features {
                Some(list) => {
                    let colors = list
                        .iter()
                        .enumerate()
                        .map(|(i, v)| color(&format!("flow.features[{i}]"), v))
                        .collect::<Result<Vec<_>, _>>()?;
                    FeatureColors::new(&colors).ok_or_else(|| {
                        format!("flow.features: {} colors, 1 to 4 needed", colors.len())
                    })?
                }
                None => d.features,
            };
            FlowColors {
                production: one("production", &flow.production, d.production)?,
                hotfix: one("hotfix", &flow.hotfix, d.hotfix)?,
                release: one("release", &flow.release, d.release)?,
                development: one("development", &flow.development, d.development)?,
                unknown: one("unknown", &flow.unknown, d.unknown)?,
                features,
            }
        }
        None => defaults.palette.flow,
    };
    let syntax = match file.syntax.as_deref() {
        Some(name) => CodeTheme::named(name)
            .ok_or_else(|| format!("syntax: '{name}' is not an embedded syntax theme"))?,
        None => defaults.syntax,
    };
    let mut palette = ThemePalette::derive(main, base).with_graph(lanes, flow);
    let optional = |key: &str| -> Result<Option<Color>, String> {
        file.colors
            .get(key)
            .map(|v| color(&format!("colors.{key}"), v))
            .transpose()
    };
    for (key, slot) in [
        ("inset", &mut palette.bg_inset),
        ("hover", &mut palette.bg_hover),
        ("selected", &mut palette.bg_selected),
        ("header_band", &mut palette.header_band),
        ("backdrop", &mut palette.backdrop),
        ("diff_added_emphasis", &mut palette.diff_added_emphasis),
        ("diff_removed_emphasis", &mut palette.diff_removed_emphasis),
    ] {
        if let Some(value) = optional(key)? {
            *slot = value;
        }
    }
    Ok(ThemeData {
        name,
        base,
        palette,
        syntax,
    })
}

/// The complete file of a theme (the editor always writes every key).
pub fn to_json(theme: &ThemeData) -> String {
    let p = &theme.palette;
    let main = p.main_colors();
    let mut colors = BTreeMap::new();
    for (key, value) in MAIN_KEYS.iter().zip([
        main.background,
        main.surface,
        main.chrome,
        main.text,
        main.text_muted,
        main.accent,
        main.border,
        main.success,
        main.warning,
        main.danger,
    ]) {
        colors.insert((*key).to_owned(), to_hex(value));
    }
    for (key, value) in [
        ("inset", p.bg_inset),
        ("hover", p.bg_hover),
        ("selected", p.bg_selected),
        ("header_band", p.header_band),
        ("backdrop", p.backdrop),
        ("diff_added_emphasis", p.diff_added_emphasis),
        ("diff_removed_emphasis", p.diff_removed_emphasis),
    ] {
        colors.insert(key.to_owned(), to_hex(value));
    }
    let file = FileFormat {
        format: Some(FORMAT),
        name: Some(theme.name.clone()),
        base: Some(
            match theme.base {
                ThemeKind::Dark => "dark",
                ThemeKind::Light => "light",
            }
            .to_owned(),
        ),
        syntax: Some(theme.syntax.name().to_owned()),
        colors,
        lanes: Some(p.lanes.iter().map(|c| to_hex(*c)).collect()),
        flow: Some(FlowFormat {
            production: Some(to_hex(p.flow.production)),
            hotfix: Some(to_hex(p.flow.hotfix)),
            release: Some(to_hex(p.flow.release)),
            development: Some(to_hex(p.flow.development)),
            unknown: Some(to_hex(p.flow.unknown)),
            features: Some(
                p.flow
                    .features
                    .as_slice()
                    .iter()
                    .map(|c| to_hex(*c))
                    .collect(),
            ),
        }),
    };
    // Plain data: serializing cannot fail.
    let mut json = serde_json::to_string_pretty(&file).unwrap_or_else(|_| "{}".into());
    json.push('\n');
    json
}

/// A theme file in the folder.
#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    Theme(ThemeDef),
    /// Not loaded; the file stays untouched.
    Invalid {
        file: String,
        reason: String,
    },
}

/// Reads every `*.json` file of `dir` (sorted by name); a missing folder
/// is empty. Blocking.
pub fn load_dir(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<_> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .collect();
    paths.sort();
    paths.iter().map(|path| load_file(path)).collect()
}

/// Reads one theme file.
pub fn load_file(path: &Path) -> Entry {
    let file = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let result = std::fs::read(path)
        .map_err(|e| format!("cannot be read: {e}"))
        .and_then(|bytes| String::from_utf8(bytes).map_err(|_| "not UTF-8 text".to_owned()))
        .and_then(|text| parse(&text));
    match result {
        Ok(data) => Entry::Theme(data.into_def(id_for(&stem))),
        Err(reason) => Entry::Invalid { file, reason },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{DARK, LIGHT};

    const MINIMAL: &str = r##"{
        "name": "Minimal",
        "base": "light",
        "colors": {
            "background": "#ffffff", "surface": "#f4f5f7", "chrome": "#e8eaee",
            "text": "#1f2328", "text_muted": "#59616e", "accent": "#1f6fc5",
            "border": "#d0d4db", "success": "#2e7d32", "warning": "#875a00",
            "danger": "#c62828"
        }
    }"##;

    #[test]
    fn hex_values() {
        assert_eq!(parse_hex("#ff0000"), Some(Color::from_rgb8(255, 0, 0)));
        assert_eq!(parse_hex("00ff00"), Some(Color::from_rgb8(0, 255, 0)));
        assert_eq!(
            parse_hex("#0000ff80"),
            Some(Color::from_rgba8(0, 0, 255, 128.0 / 255.0))
        );
        assert_eq!(
            parse_hex(" #ABCDEF "),
            Some(Color::from_rgb8(0xab, 0xcd, 0xef))
        );
        for bad in [
            "",
            "#",
            "#fff",
            "#12345",
            "#1234567",
            "#gg0000",
            "red",
            "#ff00000000",
            "##ff0000",
        ] {
            assert_eq!(parse_hex(bad), None, "{bad:?}");
        }
        assert_eq!(to_hex(Color::from_rgb8(1, 2, 255)), "#0102ff");
        assert_eq!(to_hex(Color::from_rgba8(1, 2, 3, 0.5)), "#01020380");
    }

    #[test]
    fn minimal_file_derives_the_rest() {
        let data = parse(MINIMAL).unwrap();
        assert_eq!(data.name, "Minimal");
        assert_eq!(data.base, ThemeKind::Light);
        let main = data.palette.main_colors();
        assert_eq!(main, LIGHT.main_colors());
        let derived = ThemePalette::derive(main, ThemeKind::Light);
        assert_eq!(data.palette, derived);
        assert_eq!(data.palette.lanes, LIGHT.lanes);
        assert_eq!(data.palette.flow, LIGHT.flow);
        assert_eq!(data.syntax, CodeTheme::LIGHT);
    }

    #[test]
    fn complete_file_round_trips() {
        // A complete file: every color in bytes.
        let mut data = parse(&to_json(&parse(MINIMAL).unwrap())).unwrap();
        data.palette.flow.features = FeatureColors::new(&[Color::from_rgb8(1, 2, 3)]).unwrap();
        data.palette.bg_hover = Color::from_rgb8(9, 9, 9);
        data.syntax = CodeTheme::DARK;
        let saved = to_json(&data);
        let loaded = parse(&saved).unwrap();
        assert_eq!(loaded, data);
        assert_eq!(to_json(&loaded), saved);
        // A built-in also survives (alpha values are rounded to bytes once).
        let dark = ThemeData {
            name: "Dark copy".into(),
            base: ThemeKind::Dark,
            palette: DARK,
            syntax: CodeTheme::DARK,
        };
        let once = parse(&to_json(&dark)).unwrap();
        assert_eq!(parse(&to_json(&once)).unwrap(), once);
        assert_eq!(once.palette.main_colors(), DARK.main_colors());
    }

    #[test]
    fn invalid_files_name_the_reason() {
        let with = |from: &str, to: &str| MINIMAL.replace(from, to);
        let cases = [
            (with("\"#1f6fc5\"", "\"#1f6fc\""), "colors.accent: '#1f6fc' is not a color"),
            (with("\"base\": \"light\",", ""), "\"base\" is missing"),
            (with("\"light\"", "\"dim\""), "base: 'dim'"),
            (with("\"name\": \"Minimal\",", ""), "\"name\" is missing"),
            (with("\"danger\": \"#c62828\"", "\"x\": \"#c62828\""), "colors.danger is missing"),
            (
                with("\"name\"", "\"flow\": {\"features\": []}, \"name\""),
                "flow.features: 0 colors, 1 to 4 needed",
            ),
            (
                with(
                    "\"name\"",
                    "\"flow\": {\"features\": [\"#111111\",\"#222222\",\"#333333\",\"#444444\",\"#555555\"]}, \"name\"",
                ),
                "flow.features: 5 colors, 1 to 4 needed",
            ),
            (with("\"name\"", "\"lanes\": [\"#111111\"], \"name\""), "lanes: 1 colors, 8 needed"),
            (with("\"name\"", "\"syntax\": \"Nope\", \"name\""), "syntax: 'Nope'"),
            (with("\"name\"", "\"format\": 2, \"name\""), "format 2 is not supported"),
            ("[1, 2]".to_owned(), "not a theme file"),
        ];
        for (text, reason) in cases {
            let error = parse(&text).unwrap_err();
            assert!(error.contains(reason), "{error:?} lacks {reason:?}");
        }
    }

    #[test]
    fn unknown_keys_bom_and_crlf_are_fine() {
        let text = format!(
            "\u{feff}{}",
            MINIMAL
                .replace('\n', "\r\n")
                .replace("\"name\"", "\"comment\": \"made by hand\", \"name\"")
        );
        assert_eq!(parse(&text).unwrap(), parse(MINIMAL).unwrap());
    }

    #[test]
    fn no_panic_on_random_bytes() {
        // A small deterministic generator (no randomness in tests).
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        for len in 0..400 {
            let bytes: Vec<u8> = (0..len)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    (seed & 0xff) as u8
                })
                .collect();
            let text = String::from_utf8_lossy(&bytes);
            let _ = parse(&text);
            // Valid JSON pieces with wrong types.
            let _ = parse(&format!("{{\"name\": {len}, \"base\": [{text:?}]}}"));
        }
        for text in ["{}", "null", "{\"colors\": 5}", "{\"lanes\": {}}", "\"\""] {
            assert!(parse(text).is_err(), "{text}");
        }
    }

    #[test]
    fn folder_keeps_invalid_files_listed() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Mine.json"), MINIMAL).unwrap();
        std::fs::write(dir.path().join("broken.json"), "{ not json").unwrap();
        std::fs::write(dir.path().join("binary.json"), [0xff, 0xfe, 0x00]).unwrap();
        std::fs::write(dir.path().join("notes.txt"), "ignored").unwrap();
        let entries = load_dir(dir.path());
        assert_eq!(entries.len(), 3);
        let Entry::Theme(def) = &entries[0] else {
            panic!("{entries:?}");
        };
        assert_eq!(def.id, "user:Mine");
        assert_eq!(def.name, "Minimal");
        assert!(matches!(&entries[1], Entry::Invalid { file, reason }
            if file == "binary.json" && reason == "not UTF-8 text"));
        assert!(matches!(&entries[2], Entry::Invalid { file, .. } if file == "broken.json"));
        // The broken file is untouched.
        assert_eq!(
            std::fs::read_to_string(dir.path().join("broken.json")).unwrap(),
            "{ not json"
        );
        // A missing folder is empty.
        assert!(load_dir(&dir.path().join("missing")).is_empty());
        assert_eq!(stem_of("user:Mine"), Some("Mine"));
        assert_eq!(stem_of("dark"), None);
    }

    #[test]
    fn file_names_for_names_and_collisions() {
        assert_eq!(stem_for_name("Dark copy"), "Dark copy");
        assert_eq!(stem_for_name("a/b:c?"), "a-b-c-");
        assert_eq!(stem_for_name("  ..  "), "theme");
        let taken = vec!["Dark copy".to_owned(), "dark copy 2".to_owned()];
        assert_eq!(unique_stem("Light", &taken), "Light");
        assert_eq!(unique_stem("Dark copy", &taken), "Dark copy 3");
        assert_eq!(unique_stem("DARK COPY", &taken), "DARK COPY 3");
    }
}
