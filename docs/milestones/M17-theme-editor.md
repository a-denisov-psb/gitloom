# M17 – Theme editor

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after M16. Requested by the maintainer (2026-11-19). Last milestone before the final check.

## Goal

Users make their own themes: duplicate any theme, change its colors in Preferences, check the result in a preview, save it as a file and use it like a built-in theme. Themes can be exported and imported as files. A theme with unreadable colors can never lock the user out of the settings.

## Scope

In scope:

- Preferences → Themes: list, New theme (duplicate), Rename, Delete, Import, Export, Open folder
- Editor: main colors, more colors, lane colors, Git flow colors with the feature color list, diff emphasis colors, syntax theme
- Preview panel and contrast warnings
- Theme files (JSON) and their registry integration (ADR-059)

Out of scope:

- Editing built-in themes in place
- Custom syntax token colors, `.tmTheme` import
- Sharing hub, themes per repository, theme by time of day
- Fonts and density

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Where | New global category Preferences → Themes below Appearance. Appearance has a link "Edit themes…"; the palette has the action "Edit themes…". |
| List | Built-in themes (read only, lock icon) and "My themes". Buttons: New theme (a duplicate of the selected theme, named "<name> copy"), Rename, Delete (confirmation), Import…, Export…, Open folder. Built-in themes offer only New theme (duplicate) and Export. |
| Editor | Header with name field and base switch (Dark or Light; decides the System slot and the derived defaults). Sections: Main colors (the ten of ADR-058), More colors (inset, hover, selected, header band, backdrop, diff emphasis added and removed), Graph (eight lane colors), Git flow (production, hotfix, release, development, unknown, and the feature colors as a list of 1–4 entries with add and remove; with one entry the note "All feature branches use this color"), Syntax (a list of the embedded syntect themes). Node text is automatic (black or white by contrast) and has no field. |
| Color row | Label, swatch, hex field (`#RRGGBB`; `#RRGGBBAA` where alpha is meaningful: backdrop and the emphasis colors) and a picker popover (hue and saturation/value). If the picker needs a new dependency, three RGB sliders replace it; record the choice as a milestone note. An invalid hex value marks the field and is not applied. |
| Derive | Button "Derive the other colors from the main colors" recomputes inset, hover, selected, header band, backdrop and the diff emphasis colors (`derive`, ADR-058). It never touches lanes, Git flow colors or the syntax theme. After a main color changed, a hint offers it. |
| Preview | A panel in the editor drawn with the edited theme only: sample graph rows with lane and Git flow colors and ref pills, a diff with added, removed and emphasized lines and syntax colors of the chosen syntax theme, panel, hover and selected rows, buttons, text, muted text, success, warning and danger. The rest of the window, including the editor controls, keeps the active theme, so no color choice can make Preferences unreadable (ADR-044). The sample is static text labelled "Preview", not repository data. |
| Contrast | Warnings under the preview: text on background, surface, chrome, hover and selected below 4.5:1; muted text on background and surface below 3:1; accent, success, warning and danger on background below 3:1; the syntax theme's default color on background below 4.5:1. Warnings never block saving. |
| Save | "Save" writes the file; "Save and use" also selects the theme; "Revert" restores the saved file; closing the editor with unsaved changes asks "Discard changes?". The window repaints immediately when the active theme was saved. |
| Files | One JSON file per theme in `themes/` inside the config directory (ADR-059); id `user:<file stem>`. Import copies a chosen file there after validating it; a name collision asks "Replace" or "Keep both" (the copy gets a numbered name). Export writes the theme file to a chosen path. |
| Failure | A file that cannot be read stays untouched and is listed as "<file> (invalid: <reason>)" with "Open folder". A deleted or invalid theme that was chosen or mapped falls back to `dark` or `light` with a visible note. |
| Undo | Themes are settings, not undo entries (ADR-043). The way back is Revert, Delete or choosing another theme (ADR-044). |

## Maintainer answers and GitKraken verification items

Answered (2026-11-19): new themes are created as a copy of an existing one and then edited; the whole app palette is editable; Git flow colors are editable, features as a list.

Open: none (GitKraken has no theme editor; Gitloom extension).

## Milestone acceptance

- [ ] A built-in theme can be duplicated; every color group can be edited; the result is saved as a file and used.
- [ ] The preview shows the edited theme while the controls keep the active theme; contrast warnings appear and never block saving.
- [ ] A one-entry feature list colors all feature branches the same in the Gitflow layout.
- [ ] Import and export round-trip; invalid files are listed with the reason and never crash the app.
- [ ] Deleting or breaking the active theme falls back with a visible note.
- [ ] User guide, UX-PARITY, release checklist (section 7p) and CHANGELOG updated; `./scripts/check.sh` green; CI green.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M17-01 | Theme files: format, load, save, validation, registry | M16-02, M16-03 |
| M17-02 | Themes category: list and file actions | M17-01 |
| M17-03 | Editor sections, color rows, save and derive | M17-02 |
| M17-04 | Preview panel and contrast warnings | M17-03 |
| M17-05 | Documentation | M17-01 … M17-04 |

---

## Tasks

### [x] M17-01 Theme files: format, load, save, validation, registry

- Pure module for the format of ADR-059: parse, serialize, validate, fill gaps (`derive`, built-in defaults), id from the file name.
- User themes join the registry at start and are replaced at runtime when saved, imported, renamed or deleted (no leaks); unreadable files are kept as invalid entries with their reason.

Acceptance: unit tests: a minimal file (name, base, main colors) loads and derives the rest; a complete file round-trips (`load(save(theme)) == theme`); invalid hex, missing base, unknown keys (ignored), zero and five feature colors (error), BOM and CRLF, a name collision, an unreadable file (kept, listed); no panics on random bytes.

Note: the name collision test covers `unique_stem` ("<stem> 2", "<stem> 3", compared without regard to case) and `stem_for_name`, used by import and New theme (M17-02). User themes load in the startup task with the settings; tests that change the process-wide registry hold `themes::TEST_USER_THEMES`, and the contrast test checks the built-ins only.

### [ ] M17-02 Themes category: list and file actions

- The category, the list with lock icons, New theme (duplicate), Rename (file and id; settings follow when the theme was chosen or mapped), Delete with confirmation and fallback note, Import…, Export… (system file dialogs), Open folder; link and palette action "Edit themes…".

Acceptance: app tests: duplicating a built-in creates `<name> copy.json` and selects it; rename updates file, id and settings; deleting the active theme falls back to Dark with the note; import collision asks Replace or Keep both; importing an invalid file shows the reason and changes nothing; an exported file imports again; built-ins have no Rename or Delete; snapshots of the list in Dark and Light.

### [ ] M17-03 Editor sections, color rows, save and derive

- Sections and color rows as decided; Save, Save and use, Revert, unsaved-changes dialog; the Derive button and its hint; the feature color list with 1–4 entries; the syntax theme list.

Acceptance: unit tests: hex parsing (`#RRGGBB`, `#RRGGBBAA`, without `#`; everything else rejected); Derive changes only the derived fields; the feature list keeps 1 to 4 entries. App tests: editing a main color and saving changes the file and the active window; an invalid hex value is marked and not applied; Revert; closing with changes asks; Save and use selects the theme; a one-entry feature list colors all feature lanes the same (snapshot of the graph fixture with the Gitflow layout). Snapshots of the editor for a duplicated built-in in Dark and Light.

### [ ] M17-04 Preview panel and contrast warnings

- The preview panel drawn with the edited palette only; the warning rules of the decisions table.

Acceptance: app test: an edit changes the preview and not the editor controls; unit tests per warning rule at its threshold; a snapshot with a deliberately bad theme (text equal to background) shows readable controls, an unreadable preview and the warnings.

### [ ] M17-05 Documentation

- New `docs/user/themes.md`: duplicate, edit, preview, save, import and export, the file format as a table, a hand-written minimal file. `docs/user/settings.md` (category Themes, "Files on disk" row for `themes/`), UX-PARITY row "Theme editor" (Gitloom extension), `docs/ARCHITECTURE.md`, ADR-059 implementation notes.
- `docs/RELEASE-CHECKLIST.md` section "7p. Theme editor (M17) (Windows required, Linux checked in CI)": duplicate a built-in and edit each color group; preview and warnings; Save, Save and use, Revert, discard dialog; Derive; a one-entry feature list in a repository with the Gitflow layout; export, then import on a second machine or after deleting; a hand-written minimal file; an invalid file in the folder; delete the active theme; System mapping with a user theme; a theme with text equal to background (Preferences stay readable).
- CHANGELOG `[Unreleased]`.

Acceptance: docs reviewed against the implemented behavior.

## After M17

All planned milestones are done. Do not ask for planning. Instead:

1. `docs/ROADMAP.md`: M16 and M17 done; "Current milestone" → "Final check and release 1.0.0 (M6-13, M6-15; ADR-042)" with the checklist sections 7 … 7p.
2. `docs/HANDOVER.md`: current state; the maintainer to-do "Final check before 1.0.0" lists all checklist sections 7 … 7p and the open GitKraken verification items.
3. `docs/PLANNING-HANDOVER.md`: milestone history lines for M16 and M17 (version "1.0.0 (unreleased)"), section 8 "Next: final check by the maintainer, then 1.0.0".
4. Stop with the milestone report and the line "Bereit für den finalen Test (Checkliste Abschnitte 7 … 7p); 1.0.0 auf Zuruf."
