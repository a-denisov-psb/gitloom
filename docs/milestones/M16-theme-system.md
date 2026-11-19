# M16 – Theme system

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after the fix of finding #49 (language mode, ADR-057). Requested by the maintainer (2026-11-19). First of two milestones before the final check; M17 builds the theme editor on it.

## Goal

Gitloom offers ten built-in themes instead of only Dark and Light. Window, graph, diffs and syntax colors follow the chosen theme. "System" uses one dark and one light theme of the user's choice, depending on the operating system. The palette becomes runtime data so that M17 can add themes made by the user.

## Scope

In scope:

- Palette as runtime data, theme registry, theme ids in `settings.json` (ADR-058)
- Derivation of the secondary colors from ten main colors
- Feature branch colors of the Git flow layout as a list of 1–4 colors per theme
- A syntax theme per theme
- Ten built-in themes
- Preferences → Appearance: theme list and System mapping; palette "Set: Theme ›"
- A check that views take their colors only from the palette

Out of scope:

- Themes made by the user, theme files, the editor (M17)
- The graph style tokens of the removed Classic Dark (ADR-054): row tint, filled pills, connector lines, ringed nodes
- Custom syntax token colors and `.tmTheme` files
- Fonts, density, icon sets, themes per repository, theme by time of day

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Palette data | A theme is data: id, name, base (Dark or Light), `ThemePalette` and the name of an embedded syntect theme, kept in a registry. Style functions keep their signature `fn(&Theme)`; how they reach the palette of the active theme is chosen during M16-01 and recorded in ADR-058 (no leaking per change, because M17 replaces user themes at runtime). Dark and Light keep their values exactly. |
| Base | Every theme is Dark or Light. The base decides iced's palette (`is_dark`), icon tint, backdrop and which System slot lists the theme. |
| Ids | Kebab-case: `dark`, `light`, `dark-plus`, `light-plus`, `monokai`, `github-dark`, `github-light`, `solarized-light`, `warm-dark`, `warm-light`. M17 adds `user:<file stem>`. |
| Settings | `appearance.theme` holds an id or `system`. The old values `dark`, `light`, `system` stay valid; `classic-dark` is read as `dark` (ADR-054). An unknown id means Dark; Preferences → Appearance then says "Theme '<id>' not found, using Dark" (ADR-044). |
| System | `appearance.system_dark` and `appearance.system_light` hold the ids used when the operating system reports dark (or nothing, as before) or light. Defaults `dark` and `light`. Each list offers only themes of the matching base. |
| Main colors | Ten colors define a theme: background (`bg_graph`), surface (`bg_panel`), chrome (`bg_chrome`), text, text muted, accent, border, success, warning, danger. `ThemePalette::derive` computes the rest: inset, hover, selected, header band, backdrop, node text (black or white by contrast), diff emphasis colors. Dark and Light stay explicit. The new built-ins are main colors, lanes, Git flow colors and overrides where a derived value looks wrong. |
| Lane colors | Eight colors per theme, cycled as today. |
| Git flow colors | One color each for production, hotfix, release, development and unknown. Features: a list of 1–4 colors. With one entry every feature branch has that color (fixed); with several they are cycled as today. The mapping from the four feature color ids of `git-graph` to the list lives in the palette (`lane_color`: `features[i % len]`); `git-graph` does not change. Dark and Light keep four colors; so do the new built-ins (the one-color path is covered by tests and by M17). |
| Syntax theme | Each theme names one embedded `two-face` theme; `highlight::CodeTheme` becomes that name (cache key, jobs and stale-result checks include it as they include the theme today). Gitloom keeps its own diff backgrounds (ADR-047). Pairings: Dark → OneHalfDark and Light → GitHub (unchanged); Monokai → the Monokai theme of the set; Solarized Light → the Solarized light theme; GitHub Light → GitHub; for Dark+, Light+, GitHub Dark and the two Warm themes the closest embedded theme (the set has no GitHub Dark and, per ADR-047, no Visual Studio Dark+). The result is recorded in ADR-058. |
| Built-ins | Dark and Light: today's. Dark+ and Light+: the neutral gray and blue look of VS Code's classic default themes. Monokai: dark olive-gray with pink, green, yellow, purple and orange. GitHub Dark and GitHub Light: the blue-gray and white looks of GitHub's own themes. Solarized Light: cream background, blue accent. Warm Dark: warm charcoal background, off-white text, terracotta accent. Warm Light: cream background, dark brown-gray text, terracotta accent. Names in the UI: "Dark", "Light", "Dark+", "Light+", "Monokai", "GitHub Dark", "GitHub Light", "Solarized Light", "Warm Dark", "Warm Light". |
| Sources | Hex values come from published palettes only where the license allows (VS Code themes, GitHub Primer and Solarized are MIT); otherwise own values by eye. Every source goes into the third-party notices like the syntect themes (ADR-047). The theme names are added to the ADR-033 brand check for the maintainer. |
| Contrast | Every built-in passes the checks `theme.rs` runs for Dark and Light today. Added for all themes: success, warning, danger and accent against background and surface at least 3:1; the default foreground of the syntax theme against `bg_graph` at least 4.5:1 and its colors for keywords, strings, numbers and types at least 3:1 (sampled from a fixed Rust snippet, comments excluded). A pairing that fails gets another embedded theme. |
| Preferences | Appearance → Theme: one list grouped Dark and Light plus "System"; every entry shows a small swatch strip (background, surface, accent, text, three lane colors); the current one is marked. With System selected, two lists "When the system is dark" and "When the system is light" appear. Changes apply immediately like all Preferences. The palette list "Set: Theme ›" shows the same names and "System". |
| Askpass | The askpass window reads the chosen theme from `settings.json` (read only) and falls back to the operating system's preference when the file or the id is unusable. |
| Colors in views | Views and style functions take colors only from `ThemePalette`. A test scans the view code for hard-coded colors outside an allow list (M16-07). |

## Maintainer answers and GitKraken verification items

Answered (2026-11-19): before 1.0.0; two stages; the ten built-ins above; "Claude" style as Warm Dark and Warm Light without the name; feature colors as a list of 1–4.

Open: none. Themes beyond Dark and Light are a Gitloom extension (UX-PARITY).

## Milestone acceptance

- [ ] Ten built-in themes can be chosen in Preferences → Appearance and in the command palette; window, graph (also with the Gitflow layout), diffs, blame, file history, conflict view, dialogs, toasts and the start screen follow the theme.
- [ ] Dark and Light look exactly as before (snapshot comparison).
- [ ] System uses the mapped dark or light theme; old `settings.json` values still work; an unknown id falls back to Dark with a visible note.
- [ ] A feature color list with one entry colors all feature branches the same.
- [ ] Every built-in passes the contrast checks; no view uses a hard-coded color.
- [ ] User guide, UX-PARITY, release checklist (section 7o) and CHANGELOG updated; `./scripts/check.sh` green; CI green.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M16-01 | Palette as runtime data and theme registry | – |
| M16-02 | Feature color list, syntax theme per theme, `derive` | M16-01 |
| M16-03 | Settings: theme id, System mapping, migration, askpass | M16-01 |
| M16-04 | Built-in themes: Dark+, Light+, Monokai | M16-02 |
| M16-05 | Built-in themes: GitHub Dark, GitHub Light, Solarized Light, Warm Dark, Warm Light | M16-04 |
| M16-06 | Appearance list, System mapping and palette | M16-03 |
| M16-07 | No hard-coded colors in views | M16-01 |
| M16-08 | Documentation | M16-01 … M16-07 |

---

## Tasks

### [x] M16-01 Palette as runtime data and theme registry

- `ThemeDef` (id, name, base, palette, syntax theme name) and a registry holding Dark and Light; lookup by id, list by base.
- Every place that reads the palette (`theme.rs`, `app.rs`, `icons.rs`, `ui/graph.rs`, `highlight.rs`, `askpass.rs`, `ui/toasts.rs`, `ui/start.rs`, `ui/split_handle.rs`, `ui/sidebar.rs`, `ui/diff.rs`) gets it from the registry through one mechanism; `ThemeKind` stays as the base. No leak per theme change. Record the mechanism in ADR-058.

Acceptance: all existing snapshots of Dark and Light are pixel-identical to before; unit tests for lookup, list by base and unknown id → Dark; `./scripts/check.sh` green.

Note: mechanism in ADR-058 (registry keyed by the iced theme name, `palette()` returns `Arc<ThemePalette>`); all 359 `ui_check` snapshots compared pixel by pixel with `develop` before the change: identical.

### [x] M16-02 Feature color list, syntax theme per theme, derive

- Replace `flow_colors: [Color; 9]` by named Git flow colors plus a feature list of 1–4 colors; `lane_color` maps the four feature ids to `features[i % len]`.
- `highlight::CodeTheme` becomes the name of an embedded `two-face` theme taken from the theme; the cache key, `Progressive` jobs and the stale-result checks use it.
- `ThemePalette::derive(main colors, base)` as decided above; deterministic, no randomness.

Acceptance: unit tests: `lane_color` with 1, 2, 3 and 4 feature colors for the ids base+4 … base+7; cache keys differ per syntax theme; `derive` for sample inputs of both bases keeps text readable (4.5:1) on derived inset, hover, selected and header band; Dark and Light snapshots unchanged.

### [x] M16-03 Settings: theme id, System mapping, migration, askpass

- `ThemeChoice` becomes an id or System; new `appearance.system_dark` and `appearance.system_light`; `App::theme_kind` becomes `active_theme`; `Message::ThemeChosen` carries the choice.
- Old values and `classic-dark` still read; an unknown id falls back to Dark and sets a note for Appearance.
- Askpass reads the choice (read only) with the OS preference as fallback.

Acceptance: unit tests: old `settings.json` files with `"theme": "light"`, `"system"` and `"classic-dark"` read as before; unknown id → Dark with the note; round trip of the new keys; app tests: choosing a theme applies immediately and is saved; System follows `system_theme` (Light, Dark, None) with the mapped ids; askpass picks the saved theme and falls back without a settings file.

Note: `App::theme_kind` became `active_theme` (plus `theme_note`); the mapping is set by `Message::SystemThemeMapped(base, id)`. An unknown mapped id falls back to the default of its slot (Dark for the dark slot, Light for the light slot) with the same note. The askpass window returns no theme to iced while the file, the id or the System mode is unusable or unknown, so iced follows the operating system; it asks iced for the mode at start and then uses the mapped theme.

### [x] M16-04 Built-in themes: Dark+, Light+, Monokai

- Palettes (main colors, lanes, Git flow colors, overrides), syntax pairings and sources per the decisions table; third-party notices.
- Contrast test over all registry themes (shared by M16-05).

Acceptance: the contrast test passes for each theme; a snapshot of the fixture window (graph with the Gitflow layout, a diff with word diff and syntax colors, staging, status bar) per theme; `scripts/third-party-licenses.sh` output lists the sources.

Note: the fixture is two snapshots per theme (`ui_check_themes`: `theme-<id>-graph` with the Gitflow layout, `theme-<id>-diff` with a Split word diff, staging and the status bar), since a diff replaces the graph. Light themes also keep status colors at 4.5:1 on the panel like Light.

### [x] M16-05 Built-in themes: GitHub Dark, GitHub Light, Solarized Light, Warm Dark, Warm Light

- Same as M16-04 for these five; Warm Dark and Warm Light use a warm neutral palette with a terracotta accent and own values.

Acceptance: as M16-04 for each theme; the theme names are listed for the ADR-033 check in `docs/HANDOVER.md` (Maintainer to-dos).

### [ ] M16-06 Appearance list, System mapping and palette

- Theme list with swatches grouped by base, System entry, mapping lists, "not found" note; palette "Set: Theme ›" and its current-value label from the registry.

Acceptance: app tests: choosing each theme in Preferences and in the palette; the mapping lists appear only with System and offer only themes of the matching base; the note appears for an unknown id; snapshots of Preferences → Appearance in Dark and Light, with and without System.

### [ ] M16-07 No hard-coded colors in views

- A test scans the app sources for `Color::from_rgb`, `Color::from_rgba`, `color!`, `Color::WHITE` and `Color::BLACK` outside an allow list (`theme.rs`, the syntect span conversion in `ui/diff.rs` and `ui/code_highlighter.rs`, tests), with a reason per entry. Colors found elsewhere move into the palette with values for every theme. `CLAUDE.md` hard rules get the line "UI colors come only from `ThemePalette`."

Acceptance: the test passes; the allow list is short and commented.

### [ ] M16-08 Documentation

- User guide (`docs/user/settings.md` Appearance row, `coming-from-gitkraken.md`), UX-PARITY row "Themes" (Gitloom extension: ten themes and System mapping; GitKraken has few), `docs/ARCHITECTURE.md` (registry), ADR-027 and ADR-054 status lines, ADR-058 implementation notes (mechanism, syntax pairings, sources).
- `docs/RELEASE-CHECKLIST.md` section "7o. Themes (M16) (Windows required, Linux checked in CI)": choose each of the ten themes and look at the window, graph with and without the Gitflow layout, diff, blame, conflict view, dialogs, toasts, start screen and Preferences; syntax colors readable in each theme; System follows the OS while the app runs (switch the Windows theme); old `settings.json` values; unknown id note; askpass window follows the theme.
- CHANGELOG `[Unreleased]`.

Acceptance: docs reviewed against the implemented behavior.

## After M16

Continue with M17: `docs/ROADMAP.md` → M16 done, "Current milestone" → M17.
