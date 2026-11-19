//! Theme registry (ADR-058): every theme is data (id, name, base and
//! palette) kept in one process-wide list. Style functions only receive the
//! iced [`Theme`](iced::Theme); its name is the theme id, so
//! [`crate::theme::palette`] finds the palette here. Entries are shared
//! (`Arc`) and replaced, never leaked: a theme that is replaced or removed
//! is freed once the last frame that used it is gone.

use std::sync::{Arc, LazyLock, PoisonError, RwLock};

use crate::highlight::CodeTheme;
use two_face::theme::EmbeddedThemeName;

use crate::theme::{
    hex, FeatureColors, FlowColors, MainColors, ThemeKind, ThemePalette, DARK, LIGHT,
};

/// Id of the default dark theme (also the fallback for unknown ids).
pub const DARK_ID: &str = "dark";
/// Id of the default light theme.
pub const LIGHT_ID: &str = "light";

/// One theme.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeDef {
    /// Kebab-case id stored in `settings.json`; also the iced theme name.
    pub id: String,
    /// Name in the UI.
    pub name: String,
    /// Dark or Light: iced's `is_dark`, icon tint and the System slot.
    pub base: ThemeKind,
    pub palette: Arc<ThemePalette>,
    /// The embedded syntax theme (ADR-058).
    pub syntax: CodeTheme,
}

impl ThemeDef {
    fn builtin(
        id: &str,
        name: &str,
        base: ThemeKind,
        palette: ThemePalette,
        syntax: CodeTheme,
    ) -> Self {
        Self {
            id: id.to_owned(),
            name: name.to_owned(),
            base,
            palette: Arc::new(palette),
            syntax,
        }
    }
}

// Palettes of the built-in themes (ADR-058). Values from published,
// MIT-licensed palettes where they exist (VS Code's Dark+, Light+ and
// Monokai, GitHub's Primer, Solarized; see the third-party notices),
// adjusted where Gitloom's contrast checks need it; Warm Dark and Warm
// Light are own values (warm neutrals, terracotta accent). Secondary colors are derived from the main colors.

/// The built-in themes in list order.
fn builtins() -> Vec<Arc<ThemeDef>> {
    [
        ThemeDef::builtin(DARK_ID, "Dark", ThemeKind::Dark, DARK, CodeTheme::DARK),
        ThemeDef::builtin(LIGHT_ID, "Light", ThemeKind::Light, LIGHT, CodeTheme::LIGHT),
        // Syntax pairings: the closest embedded theme that passes the
        // contrast checks (ADR-058).
        ThemeDef::builtin(
            "dark-plus",
            "Dark+",
            ThemeKind::Dark,
            dark_plus(),
            CodeTheme(EmbeddedThemeName::Nord),
        ),
        ThemeDef::builtin(
            "light-plus",
            "Light+",
            ThemeKind::Light,
            light_plus(),
            CodeTheme(EmbeddedThemeName::MonokaiExtendedLight),
        ),
        ThemeDef::builtin(
            "monokai",
            "Monokai",
            ThemeKind::Dark,
            monokai(),
            CodeTheme(EmbeddedThemeName::MonokaiExtended),
        ),
        ThemeDef::builtin(
            "github-dark",
            "GitHub Dark",
            ThemeKind::Dark,
            github_dark(),
            CodeTheme(EmbeddedThemeName::ColdarkDark),
        ),
        ThemeDef::builtin(
            "github-light",
            "GitHub Light",
            ThemeKind::Light,
            github_light(),
            CodeTheme(EmbeddedThemeName::Github),
        ),
        ThemeDef::builtin(
            "solarized-light",
            "Solarized Light",
            ThemeKind::Light,
            solarized_light(),
            CodeTheme(EmbeddedThemeName::Github),
        ),
        ThemeDef::builtin(
            "warm-dark",
            "Warm Dark",
            ThemeKind::Dark,
            warm_dark(),
            CodeTheme(EmbeddedThemeName::GruvboxDark),
        ),
        ThemeDef::builtin(
            "warm-light",
            "Warm Light",
            ThemeKind::Light,
            warm_light(),
            CodeTheme(EmbeddedThemeName::GruvboxLight),
        ),
    ]
    .into_iter()
    .map(Arc::new)
    .collect()
}

/// Dark+.
fn dark_plus() -> ThemePalette {
    let main = MainColors {
        background: hex(0x1e1e1e),
        surface: hex(0x252526),
        chrome: hex(0x333333),
        text: hex(0xd4d4d4),
        text_muted: hex(0x9d9d9d),
        accent: hex(0x3794ff),
        border: hex(0x3c3c3c),
        success: hex(0x81b88b),
        warning: hex(0xcca700),
        danger: hex(0xf14c4c),
    };
    let lanes = [
        hex(0x3b8eea),
        hex(0xf14c4c),
        hex(0x23d18b),
        hex(0xf5f543),
        hex(0xd670d6),
        hex(0x29b8db),
        hex(0xce9178),
        hex(0xb5cea8),
    ];
    let flow = FlowColors {
        production: hex(0x23d18b),
        hotfix: hex(0x29b8db),
        release: hex(0x3b8eea),
        development: hex(0xc586c0),
        unknown: hex(0x858585),
        features: FeatureColors::four([hex(0xd670d6), hex(0xf14c4c), hex(0xce9178), hex(0xdcdcaa)]),
    };
    ThemePalette::derive(main, ThemeKind::Dark).with_graph(lanes, flow)
}

/// Light+.
fn light_plus() -> ThemePalette {
    let main = MainColors {
        background: hex(0xffffff),
        surface: hex(0xf3f3f3),
        chrome: hex(0xdddddd),
        text: hex(0x1f1f1f),
        text_muted: hex(0x5f5f5f),
        accent: hex(0x005fb8),
        border: hex(0xcecece),
        success: hex(0x387314),
        warning: hex(0x895503),
        danger: hex(0xc72e0f),
    };
    let lanes = [
        hex(0x0451a5),
        hex(0xcd3131),
        hex(0x107c10),
        hex(0x795e26),
        hex(0xbc05bc),
        hex(0x0e7c91),
        hex(0xa31515),
        hex(0x098658),
    ];
    let flow = FlowColors {
        production: hex(0x107c10),
        hotfix: hex(0x0e7c91),
        release: hex(0x0451a5),
        development: hex(0xaf00db),
        unknown: hex(0x6e6e6e),
        features: FeatureColors::four([hex(0xbc05bc), hex(0xcd3131), hex(0xb35900), hex(0x795e26)]),
    };
    ThemePalette::derive(main, ThemeKind::Light).with_graph(lanes, flow)
}

/// Monokai.
fn monokai() -> ThemePalette {
    let main = MainColors {
        background: hex(0x272822),
        surface: hex(0x2f302a),
        chrome: hex(0x1e1f1c),
        text: hex(0xf8f8f2),
        text_muted: hex(0xa8a593),
        accent: hex(0x66d9ef),
        border: hex(0x414339),
        success: hex(0xa6e22e),
        warning: hex(0xe6db74),
        danger: hex(0xf92672),
    };
    let lanes = [
        hex(0x66d9ef),
        hex(0xf92672),
        hex(0xa6e22e),
        hex(0xe6db74),
        hex(0xae81ff),
        hex(0xfd971f),
        hex(0xa1efe4),
        hex(0xcfcfc2),
    ];
    let flow = FlowColors {
        production: hex(0xa6e22e),
        hotfix: hex(0xa1efe4),
        release: hex(0x66d9ef),
        development: hex(0xae81ff),
        unknown: hex(0x90908a),
        features: FeatureColors::four([hex(0xf92672), hex(0xfd971f), hex(0xe6db74), hex(0xff6188)]),
    };
    // The bright green and pink would hide the syntax colors of changed
    // words at the derived 36 %.
    ThemePalette {
        diff_added_emphasis: main.success.scale_alpha(0.2),
        diff_removed_emphasis: main.danger.scale_alpha(0.26),
        ..ThemePalette::derive(main, ThemeKind::Dark).with_graph(lanes, flow)
    }
}

/// GitHub Dark.
fn github_dark() -> ThemePalette {
    let main = MainColors {
        background: hex(0x0d1117),
        surface: hex(0x161b22),
        chrome: hex(0x010409),
        text: hex(0xe6edf3),
        text_muted: hex(0x8d96a0),
        accent: hex(0x2f81f7),
        border: hex(0x30363d),
        success: hex(0x3fb950),
        warning: hex(0xd29922),
        danger: hex(0xf85149),
    };
    let lanes = [
        hex(0x58a6ff),
        hex(0xff7b72),
        hex(0x3fb950),
        hex(0xd29922),
        hex(0xbc8cff),
        hex(0x39c5cf),
        hex(0xffa657),
        hex(0xf778ba),
    ];
    let flow = FlowColors {
        production: hex(0x3fb950),
        hotfix: hex(0x39c5cf),
        release: hex(0x58a6ff),
        development: hex(0xbc8cff),
        unknown: hex(0x8b949e),
        features: FeatureColors::four([hex(0xf778ba), hex(0xff7b72), hex(0xffa657), hex(0xd29922)]),
    };
    ThemePalette::derive(main, ThemeKind::Dark).with_graph(lanes, flow)
}

/// GitHub Light.
fn github_light() -> ThemePalette {
    let main = MainColors {
        background: hex(0xffffff),
        surface: hex(0xf6f8fa),
        chrome: hex(0xeaeef2),
        text: hex(0x1f2328),
        text_muted: hex(0x59636e),
        accent: hex(0x0969da),
        border: hex(0xd0d7de),
        success: hex(0x1a7f37),
        warning: hex(0x9a6700),
        danger: hex(0xd1242f),
    };
    let lanes = [
        hex(0x0969da),
        hex(0xcf222e),
        hex(0x1a7f37),
        hex(0x9a6700),
        hex(0x8250df),
        hex(0x1b7c83),
        hex(0xbc4c00),
        hex(0xbf3989),
    ];
    let flow = FlowColors {
        production: hex(0x1a7f37),
        hotfix: hex(0x1b7c83),
        release: hex(0x0969da),
        development: hex(0x8250df),
        unknown: hex(0x6e7781),
        features: FeatureColors::four([hex(0xbf3989), hex(0xcf222e), hex(0xbc4c00), hex(0x9a6700)]),
    };
    ThemePalette::derive(main, ThemeKind::Light).with_graph(lanes, flow)
}

/// Solarized Light.
fn solarized_light() -> ThemePalette {
    let main = MainColors {
        background: hex(0xfdf6e3),
        surface: hex(0xeee8d5),
        chrome: hex(0xe4ddc8),
        text: hex(0x073642),
        text_muted: hex(0x4a5b61),
        accent: hex(0x1b659c),
        border: hex(0xd6cdb5),
        success: hex(0x4f5c00),
        warning: hex(0x7a5c00),
        danger: hex(0xb02522),
    };
    let lanes = [
        hex(0x268bd2),
        hex(0xdc322f),
        hex(0x6a7a00),
        hex(0x8f6c00),
        hex(0x6c71c4),
        hex(0x1f7f78),
        hex(0xcb4b16),
        hex(0xd33682),
    ];
    let flow = FlowColors {
        production: hex(0x6a7a00),
        hotfix: hex(0x1f7f78),
        release: hex(0x268bd2),
        development: hex(0x6c71c4),
        unknown: hex(0x657b83),
        features: FeatureColors::four([hex(0xd33682), hex(0xdc322f), hex(0xcb4b16), hex(0x8f6c00)]),
    };
    ThemePalette {
        bg_selected: hex(0xcddae0),
        ..ThemePalette::derive(main, ThemeKind::Light).with_graph(lanes, flow)
    }
}

/// Warm Dark.
fn warm_dark() -> ThemePalette {
    let main = MainColors {
        background: hex(0x282725),
        surface: hex(0x31302d),
        chrome: hex(0x201f1d),
        text: hex(0xefebe2),
        text_muted: hex(0xaaa69c),
        accent: hex(0xd0714e),
        border: hex(0x3e3c38),
        success: hex(0x8fb573),
        warning: hex(0xd9a548),
        danger: hex(0xe0695a),
    };
    let lanes = [
        hex(0xd0714e),
        hex(0xe3a35c),
        hex(0x8fb573),
        hex(0x6fa8c7),
        hex(0xc48bb5),
        hex(0x5fb3a3),
        hex(0xd97a7a),
        hex(0xb9a56b),
    ];
    let flow = FlowColors {
        production: hex(0x8fb573),
        hotfix: hex(0x5fb3a3),
        release: hex(0x6fa8c7),
        development: hex(0xc48bb5),
        unknown: hex(0x9a958a),
        features: FeatureColors::four([hex(0xd0714e), hex(0xd97a7a), hex(0xe3a35c), hex(0xb9a56b)]),
    };
    ThemePalette::derive(main, ThemeKind::Dark).with_graph(lanes, flow)
}

/// Warm Light.
fn warm_light() -> ThemePalette {
    let main = MainColors {
        background: hex(0xfbf8f1),
        surface: hex(0xf3efe5),
        chrome: hex(0xe9e4d8),
        text: hex(0x37332b),
        text_muted: hex(0x625d52),
        accent: hex(0xa9512f),
        border: hex(0xdcd5c6),
        success: hex(0x466e33),
        warning: hex(0x7e5900),
        danger: hex(0xa8362b),
    };
    let lanes = [
        hex(0xb85a38),
        hex(0x9c6a12),
        hex(0x4f7a3a),
        hex(0x2f6f99),
        hex(0x8e4f84),
        hex(0x2f7d72),
        hex(0xb03f4a),
        hex(0x6f6a2a),
    ];
    let flow = FlowColors {
        production: hex(0x4f7a3a),
        hotfix: hex(0x2f7d72),
        release: hex(0x2f6f99),
        development: hex(0x8e4f84),
        unknown: hex(0x7a7468),
        features: FeatureColors::four([hex(0xb85a38), hex(0xb03f4a), hex(0x9c6a12), hex(0x6f6a2a)]),
    };
    ThemePalette::derive(main, ThemeKind::Light).with_graph(lanes, flow)
}

struct Registry {
    themes: Vec<Arc<ThemeDef>>,
    /// The theme editor's preview (M17-04): reachable by its id only, not
    /// listed.
    preview: Option<Arc<ThemeDef>>,
}

/// Id of the theme editor's preview theme (M17-04).
pub const PREVIEW_ID: &str = "preview:theme-editor";

static REGISTRY: LazyLock<RwLock<Registry>> = LazyLock::new(|| {
    RwLock::new(Registry {
        themes: builtins(),
        preview: None,
    })
});

/// Runs `f` on the registry. A panic while the lock was held cannot leave
/// the list half-changed (writers replace whole entries), so a poisoned
/// lock is used as it is.
fn read<T>(f: impl FnOnce(&Registry) -> T) -> T {
    let guard = REGISTRY.read().unwrap_or_else(PoisonError::into_inner);
    f(&guard)
}

/// The theme with `id`.
pub fn get(id: &str) -> Option<Arc<ThemeDef>> {
    read(|r| r.themes.iter().find(|t| t.id == id).cloned())
}

/// The theme with `id`, Dark when there is none.
#[cfg(test)]
pub fn resolve(id: &str) -> Arc<ThemeDef> {
    get(id).unwrap_or_else(|| default_of(ThemeKind::Dark))
}

/// The built-in default theme of a base (`dark` or `light`).
pub fn default_of(base: ThemeKind) -> Arc<ThemeDef> {
    let id = match base {
        ThemeKind::Dark => DARK_ID,
        ThemeKind::Light => LIGHT_ID,
    };
    read(|r| r.themes.iter().find(|t| t.id == id).cloned()).unwrap_or_else(|| {
        // Built-ins are never removed; this only keeps the function total.
        let (palette, syntax) = match base {
            ThemeKind::Dark => (DARK, CodeTheme::DARK),
            ThemeKind::Light => (LIGHT, CodeTheme::LIGHT),
        };
        Arc::new(ThemeDef::builtin(id, id, base, palette, syntax))
    })
}

/// Every theme in list order.
pub fn all() -> Vec<Arc<ThemeDef>> {
    read(|r| r.themes.clone())
}

/// The themes of one base in list order.
pub fn of_base(base: ThemeKind) -> Vec<Arc<ThemeDef>> {
    read(|r| {
        r.themes
            .iter()
            .filter(|t| t.base == base)
            .cloned()
            .collect()
    })
}

/// Replaces the user themes (ADR-059) with `themes`; built-ins stay
/// first. Old entries are dropped, not leaked.
pub fn set_user_themes(themes: Vec<ThemeDef>) {
    let mut guard = REGISTRY.write().unwrap_or_else(PoisonError::into_inner);
    guard
        .themes
        .retain(|t| !t.id.starts_with(crate::theme_file::ID_PREFIX));
    guard.themes.extend(
        themes
            .into_iter()
            .filter(|t| t.id.starts_with(crate::theme_file::ID_PREFIX))
            .map(Arc::new),
    );
}

/// Serializes tests that change the user themes of the process-wide
/// registry.
#[cfg(test)]
pub static TEST_USER_THEMES: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Palette of the theme with `id` (the iced theme name).
pub fn palette_of(id: &str) -> Option<Arc<ThemePalette>> {
    read(|r| {
        r.themes
            .iter()
            .chain(r.preview.iter())
            .find(|t| t.id == id)
            .map(|t| Arc::clone(&t.palette))
    })
}

/// Sets (or clears) the preview theme of the theme editor; it gets the id
/// [`PREVIEW_ID`]. The old one is dropped.
pub fn set_preview(def: Option<ThemeDef>) {
    let mut guard = REGISTRY.write().unwrap_or_else(PoisonError::into_inner);
    guard.preview = def.map(|def| {
        Arc::new(ThemeDef {
            id: PREVIEW_ID.to_owned(),
            ..def
        })
    });
}

/// Contrast of a syntax theme on `background` (ADR-058): its default
/// foreground, and the worst color of the tokens of a fixed Rust snippet
/// without comments (keywords, strings, numbers, types and the rest).
#[cfg(test)]
pub fn syntax_contrast(syntax: CodeTheme, background: iced::Color) -> (f32, f32) {
    use crate::theme::contrast;
    const SNIPPET: &str = "use std::fmt;\n\n#[derive(Debug)]\npub struct Point<T> {\n    x: T,\n    y: i64,\n}\n\nimpl fmt::Display for Point<u8> {\n    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {\n        let name = \"point\";\n        if self.y > 42 && name.len() != 0 {\n            return write!(f, \"{} {}\", name, 3.5);\n        }\n        Ok(())\n    }\n}\n";
    let rgba = |c: [u8; 4]| iced::Color::from_rgba8(c[0], c[1], c[2], f32::from(c[3]) / 255.0);
    let theme = crate::highlight::theme(syntax);
    let foreground = theme
        .settings
        .foreground
        .map(|c| rgba([c.r, c.g, c.b, c.a]))
        .unwrap_or(iced::Color::BLACK);
    let outcome = crate::highlight::highlight("snippet.rs", SNIPPET, syntax);
    let mut worst = f32::INFINITY;
    for (index, line) in SNIPPET.lines().enumerate() {
        for span in outcome.line(index).unwrap_or(&[]) {
            if line[span.range.clone()].trim().is_empty() {
                continue;
            }
            worst = worst.min(contrast(rgba(span.style.color), background));
        }
    }
    (contrast(foreground, background), worst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_by_id() {
        let dark = get(DARK_ID).expect("dark");
        assert_eq!(dark.name, "Dark");
        assert_eq!(dark.base, ThemeKind::Dark);
        assert_eq!(*dark.palette, DARK);
        let light = get(LIGHT_ID).expect("light");
        assert_eq!(light.base, ThemeKind::Light);
        assert_eq!(*light.palette, LIGHT);
        assert_eq!(*palette_of(LIGHT_ID).expect("light"), LIGHT);
    }

    #[test]
    fn unknown_ids_mean_dark() {
        assert!(get("no-such-theme").is_none());
        assert!(palette_of("no-such-theme").is_none());
        assert_eq!(resolve("no-such-theme").id, DARK_ID);
        assert_eq!(resolve("").id, DARK_ID);
    }

    #[test]
    fn list_by_base() {
        let dark = of_base(ThemeKind::Dark);
        assert!(dark.iter().all(|t| t.base == ThemeKind::Dark));
        assert_eq!(dark.first().map(|t| t.id.as_str()), Some(DARK_ID));
        let light = of_base(ThemeKind::Light);
        assert!(light.iter().all(|t| t.base == ThemeKind::Light));
        assert_eq!(light.first().map(|t| t.id.as_str()), Some(LIGHT_ID));
        assert_eq!(dark.len() + light.len(), all().len());
    }

    #[test]
    fn ids_are_unique() {
        let themes = all();
        for (i, t) in themes.iter().enumerate() {
            assert!(
                themes[i + 1..].iter().all(|o| o.id != t.id),
                "duplicate {}",
                t.id
            );
        }
    }

    /// Every built-in passes the contrast checks (ADR-058): text and muted
    /// text on every background, start buttons, file frames, lanes and Git
    /// flow colors, status colors, and its syntax theme.
    #[test]
    fn every_theme_passes_the_contrast_checks() {
        use crate::theme::{contrast, mix};
        for def in builtins() {
            let p = &*def.palette;
            let id = &def.id;
            for (bg_name, bg) in [
                ("graph", p.bg_graph),
                ("panel", p.bg_panel),
                ("chrome", p.bg_chrome),
                ("hover", p.bg_hover),
                ("selected", p.bg_selected),
                ("inset", p.bg_inset),
                ("header band", p.header_band),
            ] {
                let ratio = contrast(p.text, bg);
                assert!(ratio >= 4.5, "{id}: text on {bg_name} {ratio:.2}");
            }
            for (bg_name, bg) in [
                ("graph", p.bg_graph),
                ("panel", p.bg_panel),
                ("chrome", p.bg_chrome),
                ("inset", p.bg_inset),
            ] {
                let ratio = contrast(p.text_muted, bg);
                assert!(ratio >= 4.5, "{id}: muted text on {bg_name} {ratio:.2}");
            }
            // Start buttons (M15-07): the accent at 25 / 35 / 45 %.
            for alpha in [0.25, 0.35, 0.45] {
                let fill = mix(p.bg_graph, p.accent, alpha);
                let ratio = contrast(p.text, fill);
                assert!(ratio >= 4.5, "{id}: start button {alpha} {ratio:.2}");
            }
            // Rows in file frames stand out (finding #47).
            for (bg_name, bg) in [("hover", p.bg_hover), ("selected", p.bg_selected)] {
                let ratio = contrast(bg, p.bg_inset);
                assert!(ratio >= 1.1, "{id}: {bg_name} on inset {ratio:.3}");
            }
            // Status colors are icons and text: 3:1 on background and
            // surface; light themes use them for messages on the panel
            // (4.5:1, like Light).
            let status = [
                ("success", p.success),
                ("warning", p.warning),
                ("danger", p.danger),
                ("accent", p.accent),
            ];
            for (name, color) in status {
                for (bg_name, bg) in [("graph", p.bg_graph), ("panel", p.bg_panel)] {
                    let ratio = contrast(color, bg);
                    assert!(ratio >= 3.0, "{id}: {name} on {bg_name} {ratio:.2}");
                }
                if def.base == ThemeKind::Light {
                    let ratio = contrast(color, p.bg_panel);
                    assert!(ratio >= 4.5, "{id}: {name} text on panel {ratio:.2}");
                }
            }
            // Lanes (WCAG 1.4.11: 3:1) and distinct.
            for (i, lane) in p.lanes.iter().enumerate() {
                let ratio = contrast(*lane, p.bg_graph);
                assert!(ratio >= 3.0, "{id}: lane {i} {ratio:.2}");
                assert!(!p.lanes[i + 1..].contains(lane), "{id}: lane {i} twice");
            }
            let flow = &p.flow;
            let types = [
                flow.production,
                flow.hotfix,
                flow.release,
                flow.development,
                flow.unknown,
            ];
            for (i, color) in types.iter().chain(flow.features.as_slice()).enumerate() {
                let ratio = contrast(*color, p.bg_graph);
                assert!(ratio >= 3.0, "{id}: flow color {i} {ratio:.2}");
            }
            // Time marker pills (M14-06): text on the hover color, checked
            // above. Syntax colors on the code background.
            let (foreground, worst) = syntax_contrast(def.syntax, p.bg_graph);
            assert!(foreground >= 4.5, "{id}: syntax foreground {foreground:.2}");
            assert!(worst >= 3.0, "{id}: syntax token {worst:.2}");
        }
    }

    #[test]
    fn builtins_have_their_ids_names_and_bases() {
        let expected = [
            ("dark", "Dark", ThemeKind::Dark),
            ("light", "Light", ThemeKind::Light),
            ("dark-plus", "Dark+", ThemeKind::Dark),
            ("light-plus", "Light+", ThemeKind::Light),
            ("monokai", "Monokai", ThemeKind::Dark),
            ("github-dark", "GitHub Dark", ThemeKind::Dark),
            ("github-light", "GitHub Light", ThemeKind::Light),
            ("solarized-light", "Solarized Light", ThemeKind::Light),
            ("warm-dark", "Warm Dark", ThemeKind::Dark),
            ("warm-light", "Warm Light", ThemeKind::Light),
        ];
        assert_eq!(builtins().len(), expected.len(), "ten built-ins");
        for (id, name, base) in expected {
            let def = get(id).unwrap_or_else(|| panic!("{id} missing"));
            assert_eq!((def.name.as_str(), def.base), (name, base), "{id}");
            // The iced theme is dark or light by the base.
            let theme = crate::theme::iced_theme(&def);
            assert_eq!(theme.extended_palette().is_dark, base == ThemeKind::Dark);
            assert_eq!(*crate::theme::palette(&theme), *def.palette);
        }
    }

    /// User themes join after the built-ins and are replaced without
    /// leaking the old palettes (ADR-059).
    #[test]
    fn user_themes_are_replaced_not_leaked() {
        let _lock = TEST_USER_THEMES
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let user = |id: &str, name: &str| ThemeDef {
            id: id.into(),
            name: name.into(),
            base: ThemeKind::Light,
            palette: Arc::new(LIGHT),
            syntax: CodeTheme::LIGHT,
        };
        set_user_themes(vec![user("user:a", "A"), user("user:b", "B")]);
        let a = get("user:a").expect("a");
        let weak = Arc::downgrade(&a.palette);
        drop(a);
        assert_eq!(all().last().map(|t| t.id.clone()), Some("user:b".into()));
        assert_eq!(
            all().iter().filter(|t| t.id.starts_with("user:")).count(),
            2
        );
        // Built-in ids cannot be taken over.
        set_user_themes(vec![user("user:a", "A2"), user("dark", "Fake")]);
        assert_eq!(get("user:a").map(|t| t.name.clone()), Some("A2".into()));
        assert!(get("user:b").is_none());
        assert_eq!(get("dark").map(|t| t.name.clone()), Some("Dark".into()));
        assert!(weak.upgrade().is_none(), "old palette freed");
        set_user_themes(Vec::new());
        assert!(get("user:a").is_none());
    }
}
