//! Theme registry (ADR-058): every theme is data (id, name, base and
//! palette) kept in one process-wide list. Style functions only receive the
//! iced [`Theme`](iced::Theme); its name is the theme id, so
//! [`crate::theme::palette`] finds the palette here. Entries are shared
//! (`Arc`) and replaced, never leaked: a theme that is replaced or removed
//! is freed once the last frame that used it is gone.

use std::sync::{Arc, LazyLock, PoisonError, RwLock};

use crate::highlight::CodeTheme;
use crate::theme::{ThemeKind, ThemePalette, DARK, LIGHT};

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

/// The built-in themes in list order.
fn builtins() -> Vec<Arc<ThemeDef>> {
    [
        ThemeDef::builtin(DARK_ID, "Dark", ThemeKind::Dark, DARK, CodeTheme::DARK),
        ThemeDef::builtin(LIGHT_ID, "Light", ThemeKind::Light, LIGHT, CodeTheme::LIGHT),
    ]
    .into_iter()
    .map(Arc::new)
    .collect()
}

struct Registry {
    themes: Vec<Arc<ThemeDef>>,
}

static REGISTRY: LazyLock<RwLock<Registry>> =
    LazyLock::new(|| RwLock::new(Registry { themes: builtins() }));

/// Runs `f` on the registry. A panic while the lock was held cannot leave
/// the list half-changed (writers replace whole entries), so a poisoned
/// lock is used as it is.
fn read<T>(f: impl FnOnce(&Registry) -> T) -> T {
    let guard = REGISTRY.read().unwrap_or_else(PoisonError::into_inner);
    f(&guard)
}

/// The theme with `id`.
#[allow(dead_code)] // Used from M16-03 / M16-06 on.
pub fn get(id: &str) -> Option<Arc<ThemeDef>> {
    read(|r| r.themes.iter().find(|t| t.id == id).cloned())
}

/// The theme with `id`, Dark when there is none.
#[allow(dead_code)] // Used from M16-03 / M16-06 on.
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
#[allow(dead_code)] // Used from M16-03 / M16-06 on.
pub fn all() -> Vec<Arc<ThemeDef>> {
    read(|r| r.themes.clone())
}

/// The themes of one base in list order.
#[allow(dead_code)] // Used from M16-03 / M16-06 on.
pub fn of_base(base: ThemeKind) -> Vec<Arc<ThemeDef>> {
    read(|r| {
        r.themes
            .iter()
            .filter(|t| t.base == base)
            .cloned()
            .collect()
    })
}

/// Palette of the theme with `id` (the iced theme name).
pub fn palette_of(id: &str) -> Option<Arc<ThemePalette>> {
    read(|r| {
        r.themes
            .iter()
            .find(|t| t.id == id)
            .map(|t| Arc::clone(&t.palette))
    })
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
}
