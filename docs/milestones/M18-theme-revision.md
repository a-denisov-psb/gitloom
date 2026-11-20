# M18 – Theme revision

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after M17. Requested by the maintainer (2026-11-20), findings #50 and #51; planning update 14 adds finding #52 as M18-06. Last milestone before the final check.

## Goal

The built-in themes are revised: a new default dark theme "Dusk", a "Studio Dark" with a real Dark+ syntax look, and no theme name that belongs to another product. Themes stop being a category of their own in Preferences: they are a sub-page of Appearance.

## Scope

In scope:

- New theme set, names, ids and the migration of old ids in `settings.json` (ADR-060)
- Two embedded syntax themes of our own (`.tmTheme`): Dusk and Studio Dark
- Palettes: Dusk replaces Dark; Studio Dark keeps the UI colors of today's Dark+
- Preferences: "Manage themes…" in Appearance opens the theme list and editor as a sub-page; the category "Themes" is removed
- Documentation, third-party notices, checklist section 7q

Out of scope:

- Changes to the editor, the file format (ADR-059) or the preview
- Custom syntax token colors, `.tmTheme` import by users
- Semantic highlighting (language server); only grammar scopes are colored
- New light themes

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Theme set | Dark group: Dusk, Studio Dark, Monokai, Midnight, Warm Dark. Light group: Light, Studio Light, Sepia, Daylight, Warm Light. This is also the list order. |
| Renames | `dark` → **Dusk** (`dusk`, new palette), `dark-plus` → **Studio Dark** (`studio-dark`), `github-dark` → **Midnight** (`midnight`), `light-plus` → **Studio Light** (`studio-light`), `solarized-light` → **Sepia** (`sepia`), `github-light` → **Daylight** (`daylight`). Monokai, Warm Dark, Light and Warm Light keep name and id. Palettes of Studio Dark, Midnight, Studio Light, Sepia and Daylight stay unchanged. |
| Names | No theme name of another product or brand in the UI, the user guide, the CHANGELOG or UX-PARITY. Provenance stays where a license needs it: third-party notices, ADRs, source comments. Names are checked once more against ADR-033. |
| Old ids | `appearance.theme`, `appearance.system_dark` and `appearance.system_light` are read through an alias table: `dark` → `dusk`, `dark-plus` → `studio-dark`, `github-dark` → `midnight`, `light-plus` → `studio-light`, `solarized-light` → `sepia`, `github-light` → `daylight` (`classic-dark` already reads as `dark`, so it ends at `dusk`). Only new ids are written. The askpass window uses the same reader. |
| Default | Dusk is the default dark theme: first start, System slot "dark", fallback for an unknown or broken id ("Theme '<id>' not found, using Dusk"). Light stays the default light theme. `themes::default_of(Dark)` returns Dusk. Missing `lanes`, `flow` and `syntax` in a user theme file of base dark come from Dusk (ADR-059 said `dark`). |
| Dusk UI | Main colors from the maintainer's screenshot of the Ayu Mirage theme in the Zed editor, measured in pixels and checked against Zed's theme file: background `#242835`, surface `#353944`, chrome `#464a52`, text `#cccac2`, text muted `#9a9a98`, accent `#72cffe`, border `#53565d`, success `#d5fe80`, warning `#fecf72`, danger `#f18779`. Secondary colors by `derive`; override hover with `#43464f` and selected with `#53565d` where that is closer to the model. Muted text is lightened as far as needed for the contrast checks (`#9a9a98` on `#353944` is about 4.1:1). Lanes (suggested, from the model's player colors): `#72cffe`, `#f18779`, `#d5fe80`, `#fecf72`, `#debffe`, `#95e5cb`, `#fead66`, `#5bcde5`. Git flow (suggested): production `#d5fe80`, hotfix `#f18779`, release `#fecf72`, development `#72cffe`, unknown `#7b7d7f`, features `#fead66`, `#debffe`, `#95e5cb`, `#5bcde5`. Adjust by eye; the contrast tests decide. |
| Studio Dark UI | The palette of today's Dark+ stays as it is (it matches the default dark look of VS Code: `#1e1e1e`, `#252526`, `#333333`, `#3c3c3c`). Only name, id and syntax change. |
| Syntax themes | Two embedded `.tmTheme` files in `crates/app/assets/`, loaded with syntect at start-up: "Dusk" and "Studio Dark" (names as shown in the editor's syntax list and as the `syntax` value of theme files). `two-face` has no Visual Studio Dark+ any more and the Nord pairing is weak, so Studio Dark brings its own. `highlight::CodeTheme` becomes either a two-face theme or one of ours (still `Copy`, still part of the cache key and of every job). Dusk uses Dusk, Studio Dark uses Studio Dark; the other pairings of ADR-058 stay (Light → GitHub, Studio Light → Monokai Extended Light, Monokai → Monokai Extended, Midnight → Coldark-Dark, Daylight and Sepia → GitHub, Warm Dark → gruvbox-dark, Warm Light → gruvbox-light). OneHalfDark stays selectable as a two-face theme in the editor's list. |
| Studio Dark syntax | Translated one to one from the VS Code themes `dark_vs.json` and `dark_plus.json` (microsoft/vscode, `extensions/theme-defaults/themes`, MIT); both use TextMate scopes, so selectors carry over. Main colors: keyword, storage and modifier `#569cd6`; control flow (`keyword.control`, `keyword.other.using`) `#c586c0`; types, namespaces, classes `#4ec9b0`; functions `#dcdcaa`; variables and parameters `#9cdcfe`; constants and enum members `#4fc1ff`; strings `#ce9178`; numbers `#b5cea8`; comments `#6a9955`; escapes and CSS classes `#d7ba7d`; regex `#d16969`; tag punctuation `#808080`; invalid `#f44747`; default text and operators `#d4d4d4`. |
| Dusk syntax | The scope structure comes from the MIT-licensed Ayu color scheme (`dempfi/ayu`, `ayu-mirage.sublime-color-scheme`); the hex values are the ones of the Zed theme in the screenshot: keyword and preproc `#ffad65`; string `#d4fe7f`; number, constant, boolean and parameter `#dfbfff`; function `#ffd173`; type `#73cfff`; property, attribute, tag and constructor `#72cffe`; operator `#f29e74`; comment `#5c6773`; doc comment and escape `#9b9b99`; regex `#95e6cb`; variable and default text `#cccac2`; punctuation `#b4b3ae`. Zed's captures are Tree-sitter names, so the mapping to scopes is ours. |
| Limits | VS Code and Zed also color by meaning (semantic highlighting through a language server); Gitloom colors by grammar only. The result is very close but not always identical (for example C# properties and class names); this is documented in the user guide and accepted. |
| Sources | Hex values and scope structure only; no files are copied. Licenses (VS Code color themes, Ayu) go into `scripts/third-party/theme-palettes.md` and from there into the third-party notices; the unit test that checks the notices is extended. Zed's `ayu.json` has no license header, so the notices name the MIT Ayu sources and the values are verified against them. |
| Preferences | Appearance → Theme gets a button "Manage themes…" next to the theme list. It replaces the content of the Preferences window with the theme list and editor (same window, no second modal). Header: "← Appearance" (or "Appearance › Themes"); inside the editor the existing back to the list stays; Esc goes up one level, from the list back to Appearance. The sub-page is not remembered: opening Preferences shows Appearance itself. Leaving with unsaved edits asks "Discard changes?" as before (back button, Esc, closing Preferences, another category). The category "Themes" disappears from the sidebar. |
| Entry points | The palette action "Edit themes…" and the settings search result "Edit themes" open Preferences directly on the sub-page. The shortcut registry keeps its action. |
| Docs | The user guide page "Themes" and checklist sections 7o and 7p name the new path (Preferences → Appearance → Manage themes) and the new theme names. |

## Maintainer answers and GitKraken verification items

Answered (2026-11-20): the list above; Dusk replaces Dark and is the default; Studio Dark = screenshot colors and syntax like VS Code Dark+ (compare with the VS Code sources, they are open source); names without brands (Midnight, Daylight, Studio Light, Sepia, Dusk); themes as a sub-page of Appearance in the same window.

Open: none (Gitloom extension, no GitKraken item). For the maintainer's check: Dusk against Zed's Ayu Mirage and Studio Dark against VS Code Dark+ side by side (checklist 7q).

## Milestone acceptance

- [ ] The ten built-ins show the new names in the order of the decision table in Preferences, the palette and everywhere else; Dusk is the default dark theme.
- [ ] Old `settings.json` values (`dark`, `dark-plus`, `github-dark`, `light-plus`, `solarized-light`, `github-light`, `classic-dark`) keep working and are rewritten with the new ids.
- [ ] Studio Dark and Dusk highlight with their own syntax themes in the diff, blame, file history and the conflict view; both pass the contrast checks.
- [ ] The category "Themes" is gone; "Manage themes…" opens the same list and editor inside Appearance with a way back; unsaved edits are protected on every way out.
- [ ] No product or brand name of another theme in UI, user guide, CHANGELOG and UX-PARITY.
- [ ] Switching the active tab closes diff, blame and file history of the tab that is left; returning shows the graph (M18-06).
- [ ] User guide, UX-PARITY, release checklist (section 7q), third-party notices, ADR-060 and CHANGELOG updated; `./scripts/check.sh` green; CI green.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M18-01 | Names, ids and settings migration | M17-05 |
| M18-02 | Embedded syntax themes Dusk and Studio Dark | M17-05 |
| M18-03 | Palettes: Dusk replaces Dark, Studio Dark, renames | M18-01, M18-02 |
| M18-04 | Themes as a sub-page of Appearance | M17-05 |
| M18-05 | Documentation and close | M18-01 … M18-04 |
| M18-06 | Tab switch closes center views (planning update 14, finding #52) | M18-05 (order only) |

---

## Tasks

### [x] M18-01 Names, ids and settings migration

- Rename the built-ins and their ids as in the decision table; set the list order; `themes::default_of(Dark)`, the default id constant, the fallback note ("Theme '<id>' not found, using Dusk") and the System defaults point to `dusk`.
- Alias table for old ids when reading `appearance.theme`, `appearance.system_dark`, `appearance.system_light`; only new ids are written; the askpass window reads through the same code.
- Replace the old names in every user-visible string (Preferences, palette "Set: Theme ›", tooltips, notes, About/notices text where it lists themes).
- User theme files of base dark take missing `lanes`, `flow` and `syntax` from Dusk (`theme_file`).

Acceptance: unit tests: every old id maps to its new id in all three settings fields and is written back as the new id; `classic-dark` ends at `dusk`; an unknown id falls back to Dusk with the note; askpass reads an old settings file; the list order of the two groups; a user theme file of base dark without `lanes`, `flow` and `syntax` loads with Dusk's values. A search (`rg`) for the old names in string literals of `crates/*/src` finds nothing in user-visible text.

Note (done): `themes::canonical_id` holds the alias table (including `dark` and `classic-dark`); `ThemeChoice::parse` and a `deserialize_with` on `system_dark` / `system_light` read through it, so the app and the askpass window share it. Dusk still uses the old Dark palette and the OneHalfDark syntax until M18-03 and M18-02. Messages that set an id at runtime (Preferences, palette) only offer registry ids and do not go through the table.

### [x] M18-02 Embedded syntax themes Dusk and Studio Dark

- `highlight::CodeTheme` can hold one of our embedded themes besides a two-face theme (`Copy`, hashable, part of the cache key and of every job); the two `.tmTheme` files live in `crates/app/assets/` and are loaded with syntect at start-up; a broken embedded file is a test failure, never a runtime panic.
- Studio Dark and Dusk as described in the decision table (scope translation of the VS Code themes; Ayu scope structure with the Zed hex values). Both names appear in the editor's syntax list and are valid `syntax` values in theme files (ADR-059 note); the two-face names stay valid.
- Sources and licenses into `scripts/third-party/theme-palettes.md` and the notices; extend the notices test.

Acceptance: unit tests: both themes load; sampled rule colors match the tables of the decision table (keyword, control keyword, type, function, variable, constant, string, number, comment for each); `themes::syntax_contrast` passes for both on their palettes (a rule that fails is lightened or darkened and the change is noted in ADR-060); highlighting a small C# file (usings, class, attribute, property, string, number, doc comment) gives distinct colors for keyword, control keyword, type, string, number and comment; the cache distinguishes the two themes and a two-face theme; the editor's syntax list contains both names; a theme file with `"syntax": "Dusk"` round-trips.

Note for the implementation: the C# file of the maintainer's comparison screenshots is the reference case. Differences caused by missing semantic highlighting are expected and go into the user guide, not into extra rules.

Note (done): Studio Dark is paired with its syntax theme here; Dusk keeps OneHalfDark until its palette changes in M18-03. Four extra Studio Dark rules cover scopes the Sublime grammars name differently from VS Code's (function calls, C# built-in types and `var`, attributes, `using`); Dusk gives keywords and control keywords the same color like Ayu Mirage, so the C# test checks a separate control keyword color for Studio Dark only. syntect needs its `plist-load` feature (new crate `plist`, pinned to 1.10.3). Details in the ADR-060 notes.

### [x] M18-03 Palettes: Dusk replaces Dark, Studio Dark, renames

- Dusk: the palette of the decision table replaces the Dark palette and the old Dark constants (the OneHalfDark pairing is dropped as a pairing); lanes, Git flow colors and secondary colors as decided, adjusted until the existing contrast tests pass. Light is unchanged.
- Studio Dark: today's Dark+ palette, new name, id and syntax. Midnight, Studio Light, Sepia and Daylight: palettes and syntax pairings unchanged, new names and ids.
- Tests and snapshots that name the old default (`dark`) use Dusk; the M16 rule "Dark looks as before" no longer applies to Dark, only to Light.

Acceptance: contrast tests pass for all ten themes; a snapshot of Dusk (window with graph, diff, Preferences → Appearance) replaces the old Dark snapshot; the Light snapshot is unchanged; the snapshots of Studio Dark, Midnight, Studio Light, Sepia and Daylight differ from the old ones only in names and, for Studio Dark, in syntax colors; the check for hard-coded colors outside the allow list stays green.

Note (done): `theme::DARK` holds Dusk and `CodeTheme::DARK` is the Dusk syntax theme. Contrast corrections: muted text `#b9b9b8` (4.5:1 on the chrome `#464a52`), hover `#464a53` (stands out on the inset), selected `#51545b` (4.6:1 text); diff word emphasis 20 / 26 % like Monokai. The light accent with light text cannot reach 4.5:1 at the start buttons' 35 / 45 %: instead of darkening the accent, the start button caps its accent alpha per palette at the highest value that keeps 4.5:1 (Dusk: 29 % on hover and press; the other themes keep 25 / 35 / 45 %, checked by the contrast test). The ui_check snapshots are written to `target/ui-snapshots/` and not stored; `theme-dusk-*` and `theme-list-dusk` replace the Dark ones, checked by eye.

### [x] M18-04 Themes as a sub-page of Appearance

- Remove the category "Themes" from the Preferences sidebar. Appearance → Theme gets the button "Manage themes…" that opens the existing list and editor as a sub-page in the same window, with the header "← Appearance" (or "Appearance › Themes"); Esc goes up one level.
- The sub-page is not remembered; opening Preferences shows Appearance. The unsaved-changes dialog covers every way out (back, Esc, closing Preferences, another category).
- Palette action "Edit themes…" and the settings search result "Edit themes" open Preferences on the sub-page; the editor, the file actions and the preview do not change.

Acceptance: app tests: "Manage themes…" shows the list; back returns to Appearance; Esc returns one level and closes Preferences from Appearance as before; an edited, unsaved theme asks "Discard changes?" on back, Esc and closing; the palette action opens the sub-page from a closed Preferences window; the sidebar has no "Themes" entry; reopening Preferences shows Appearance. Layout check at the minimum window size: the editor with preview stays usable (no clipped controls).

Note (done): `Category::Themes` stays as the sub-page's state but is no longer in the sidebar; `Category::sidebar` highlights Appearance for it and `Category::parent` drives the "← Appearance" link above the title and Esc. Esc goes editor → list → Appearance → closed and asks "Discard changes?" on unsaved edits (before, Esc closed Preferences without asking). Leaving the sub-page closes a saved editor. "Opening Preferences shows Appearance itself" is read as "never the sub-page": Preferences open on General as before (GitKraken opens on its first page too); reopening after the sub-page shows General, not Themes. The "Manage themes…" button sits under the theme list. Checked by hand in the ui_check snapshots `themes-sub-page`, `themes-appearance-button` and `themes-editor-min-size` (window minimum size: all controls usable; a long preview diff line already ran over the preview frame before M18).

### [ ] M18-05 Documentation and close

- New ADR-060 (text below) in `docs/DECISIONS.md`; implementation notes after the work (contrast corrections, mapping choices).
- `docs/user/themes.md` and `docs/user/settings.md`: new names, the path Preferences → Appearance → Manage themes, the limit about semantic highlighting; the "Theme editor" and "Themes" rows of `docs/UX-PARITY.md` updated; CHANGELOG `[Unreleased]`: the paragraphs about the ten themes and the theme editor are corrected in place (new names, new path), plus a line for the Studio Dark and Dusk syntax themes.
- `docs/RELEASE-CHECKLIST.md`: sections 7o and 7p updated (path and names); new section "7q. Theme revision (M18) (Windows required)": compare Dusk with Zed's Ayu Mirage and Studio Dark with VS Code Dark+ in a C# file (colors of keywords, types, strings, comments, current line, panels); every built-in shows its new name in Preferences and the palette; an old `settings.json` with each old id still works and is rewritten; "Manage themes…" opens the sub-page, back and Esc behave, unsaved edits ask before leaving; no old product names anywhere in the UI.
- Third-party notices regenerated; `./scripts/check.sh` green.

Acceptance: docs reviewed against the implemented behavior; checklist 7q matches the acceptance list.

### [ ] M18-06 Tab switch closes center views

- When the active repository tab changes (click, keyboard shortcut, command palette, switch to the start screen), the tab that is left returns its center view to the graph if that view replaces the graph: diff (hunk, inline, split), blame and file history. It behaves exactly like the Close button in the diff header, including what happens to the file selection in the right panel. Returning to a tab always shows the graph.
- All ways of changing the active tab go through one place (no per-path special cases).
- The conflict view is checked first: if closing it would lose unsaved resolution work, it stays open (ADR-044, no data loss) and the exception is documented in the task note; otherwise it closes like the other views.
- Closing a view is view state only, not a write action: no undo entry (ADR-043). Staged hunks and lines live in the index and are unaffected.

Acceptance: app tests: open a diff in tab A, switch to tab B and back: the graph is shown and the diff is closed; the same for inline, split, blame and file history; switching to the start screen and back; two tabs that each have an open view are both closed when left; a line staged before the switch is still staged afterwards; the conflict view behaves as decided (test matches the documented outcome). Docs: UX-PARITY row "Diff" gets "switching tabs closes it (Gitloom decision; GitKraken behavior to be checked)"; the user doc that describes the diff view; `docs/RELEASE-CHECKLIST.md` section 7q gets one item (open a diff, switch tab and back, the graph is shown; same for blame and file history); CHANGELOG `[Unreleased]`; implementation note in ADR-060 (no new ADR); `docs/HANDOVER.md` GitKraken verification list: "Does GitKraken close the diff when switching tabs?".

## ADR-060 (to append to docs/DECISIONS.md)

## ADR-060 Theme set revision and themes under Appearance

- Status: accepted (M18 plan, 2026-11-20)
- Context: ADR-058 introduced ten built-in themes with names of other products (Dark+, Light+, GitHub Dark, GitHub Light, Solarized Light). Dark and Dark+ were close to each other, and the Dark+ syntax pairing (Nord) was weak because `two-face` no longer ships a Visual Studio Dark+ theme. ADR-059 put the theme list and editor in a category of their own although themes are a cosmetic setting.
- Decision:
  - The set becomes Dusk, Studio Dark, Monokai, Midnight, Warm Dark (dark) and Light, Studio Light, Sepia, Daylight, Warm Light (light). Dusk replaces Dark and is the default dark theme; its colors follow the Ayu Mirage theme as the Zed editor shows it. Studio Dark keeps the UI colors of Dark+; Midnight, Studio Light, Sepia and Daylight are renames. Theme names carry no other product's name; provenance is kept in the notices.
  - Ids change with the names (`dusk`, `studio-dark`, `midnight`, `studio-light`, `sepia`, `daylight`). Old ids are read through an alias table in `settings.json` and never written again.
  - Two syntax themes of our own (`.tmTheme`, embedded): Studio Dark (translated from the VS Code Dark+ themes, MIT) and Dusk (Ayu scope structure, Zed hex values). Custom syntax token colors by users stay out of scope. Semantic highlighting is not available; grammar scopes only.
  - Themes are a sub-page of Preferences → Appearance ("Manage themes…") in the same window; the category "Themes" is removed. Palette action and settings search open the sub-page directly.
- Why: fewer near-duplicates, no brand names, a real Dark+ syntax look, and a settings sidebar that keeps cosmetic options in one place without a second modal.
- Consequences: ids and names in `settings.json` need the alias table for good; ADR-058's list of themes and ADR-059's "Themes category" are superseded in these points; the hex sources and licenses of the new syntax themes belong in the notices.

## After M18

These closing steps follow M18-06 (planning update 14), not M18-05. All planned milestones are done. Do not ask for planning. Instead:

1. `docs/ROADMAP.md`: M18 done; "Current milestone" → "Final check and release 1.0.0 (M6-13, M6-15; ADR-042)" with the checklist sections 7 … 7q.
2. `docs/HANDOVER.md`: current state; the maintainer to-do "Final check before 1.0.0" lists all checklist sections 7 … 7q and the open GitKraken verification items.
3. `docs/PLANNING-HANDOVER.md`: milestone history line for M18 (version "1.0.0 (unreleased)"), section 8 "Next: final check by the maintainer, then 1.0.0".
4. Stop with the milestone report and the line "Bereit für den finalen Test (Checkliste Abschnitte 7 … 7q); 1.0.0 auf Zuruf."
