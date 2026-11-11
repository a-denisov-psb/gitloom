# M11 – Graph columns, themes and syntax highlighting

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after M10.

## Goal

The graph table behaves like GitKraken's (resizable columns, a column menu), a "Classic Dark" theme brings back the look of older GitKraken versions, stashes look like GitKraken's, and code in diffs, blame, file history and the conflict view is syntax highlighted with the editor settings GitKraken offers.

## Scope

In scope:

- Style tokens in the palette; theme "Classic Dark"; Dark and Light keep their look
- Graph columns: resize, optimal width by double click, column menu, persisted widths and visibility; no automatic hiding
- Classic Dark graph look: row tint, filled ref pills with a connector line, header band, column separators, panel depth
- Stash node like GitKraken and the label "WIP on <branch>"
- Preferences → Editor: font, font size, tab size, line numbers, word wrap, syntax highlighting
- Syntax highlighting in all diff modes, blame, file history and the conflict view

Out of scope:

- An in-app file editor (GitKraken's "End of Line Character" setting belongs to it; UX-PARITY deviation)
- User-defined themes or theme files (ideas backlog)

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Style tokens | `ThemePalette` gains tokens for what differs between themes: `row_tint_alpha` (0 = off), `ref_pill` (outline / filled), `ref_connector` (bool), `header_band` color, `column_separator` color (transparent = off), `panel_shadow` (bool), `node_style`. Dark and Light set values that reproduce today's look exactly (snapshot comparison). ADR-048. |
| Classic Dark | New `ThemeChoice::ClassicDark` ("Classic Dark") next to Dark, Light, System; colors after GitKraken 7.5 (colors are free, no assets): graph background `#1e2128`, panels `#262a31`, chrome `#1b1d22`, header band `#2a2f37`, separators `#323842`, text `#d8dde4`, muted `#8b94a3`, accent `#1aa3c8`, lanes `#1aa3c8`, `#e0247a`, `#8b5cc9`, `#2ebf91`, `#f2a93b`, `#e04b4b`, `#3d7be0`, `#c9b42c`. Row tint on, filled pills with connector, header band, separators, panel shadow on. Fine-tuning by eye is allowed and noted in the task. |
| Row tint (Classic Dark) | Each commit row is tinted in its lane color at low alpha (about 15 %) from the node's lane to the right edge of the graph column; selection and hover stay on top. |
| Ref pills (Classic Dark) | Pills filled with the lane color of their commit, white text, the checked-out pill with the existing check mark; a horizontal connector line in the lane color from the pill's right edge to the node. Dark and Light keep outline pills without connector. |
| Columns | Branch/Tag, Graph, Commit message, Author, Date, SHA. Message fills the rest (minimum 120 px). Borders between header cells can be dragged (cursor ↔); a double click on a border sets the optimal width of the column on its left (Branch/Tag: widest visible pill group; Graph: lanes of the loaded layout, capped at 50 % of the table; Author, Date, SHA: widest visible content). Minimum widths: 60 px, Graph 1 lane. A Graph column narrower than the lanes clips them on the right with a short fade. Widths and visibility are global settings (`settings.json`), restored on start. |
| Column menu | Gear button at the right end of the header (like GitKraken) opens a menu with checkboxes Author, Date, SHA (Branch/Tag, Graph and Message are always shown) and "Reset column widths". The same three options in Preferences → Appearance ("Show commit author / date and time / SHA in graph", GitKraken's UI Customization). The automatic hiding of columns by window width is removed; a narrow window clips the table at the right. |
| Stash node | Dashed square outline in the lane color with a small stash icon (Lucide `archive`, vendored like the other icons) inside; dotted edge to the base commit (as today). Label: a message that is Git's default (`WIP on <branch>: <sha> <summary>`) or empty (`On <branch>:`) shows as "WIP on <branch>"; otherwise the text after `On <branch>: `. Same label in the STASHES section and the stash commit details title. "Stash" without a message no longer passes an empty `-m`, so Git writes its default message. |
| Editor settings | New global category Preferences → Editor (GitKraken "Editor"): Font (Default = the platform monospace; list of installed monospace fonts from the font database iced already loads, fallback to Default when missing), Font size (8–24, default 12), Tab size (1–8, default 4; tabs are expanded for display only), Show line numbers (on), Word wrap (off), Syntax highlighting (on). Applies to all diff modes, blame, file history and the conflict view; the commit message fields are not affected. |
| Word wrap | Wrapped lines take several visual rows; with a monospace font the row count is computed from the character count and the column width, so virtualization keeps working (prefix sums of row counts, recomputed on resize). Line numbers and block bars stay on the first visual row; line selection selects whole logical lines. Split view wraps each side independently and keeps the pairing aligned by the larger row count. Horizontal scrolling (M7-16) is off while wrapping. If wrap turns out to break the 50 ms frame budget of `docs/PERFORMANCE.md`, it may move to the ideas backlog (ADR-034, scope reduction with a note). |
| Highlighting engine | `syntect` with the `two-face` syntax set (languages like bat/VS Code built-ins) in a pure app module `highlight` without iced types: input file content and path, output style spans per line. Whole old and new files are highlighted (M7-02 full context), then mapped to diff lines, so multi-line comments and strings are right in every mode. Background task, cache keyed by blob id + theme, files over 1 MB or 20,000 lines are not highlighted (plain, with a muted note "Too large for syntax highlighting"). Unknown file types stay plain. Plain text is shown first and replaced when highlighting is ready. ADR-047. |
| Highlighting colors | One syntect theme per Gitloom theme from `two-face`'s embedded themes, chosen for contrast with the diff backgrounds: Dark and Classic Dark a dark theme (e.g. "Visual Studio Dark+"), Light a light theme (e.g. "GitHub"); added/removed backgrounds stay Gitloom's, syntect only colors the text. Exact choice recorded in ADR-047. |
| Conflict view | Ours and theirs columns highlighted like diffs; the editable output uses iced's text editor highlighter interface with the same engine. |

## Maintainer answers and GitKraken verification items

Answered (GitKraken 7.5.2 screenshots, 2026-11-11): header gear at the right end; UI Customization has "Show commit author/date/sha in graph"; Editor page: Font, Font Size, Tab Size, End of Line Character, Syntax Highlighting, Show Line Numbers, Word Wrap; the stash node is a dashed square with an icon, labelled "WIP on develop"; rows in the graph are tinted in the lane color and filled pills connect to their node.

Open:

| Item | Used in |
|---|---|
| GitKraken's column menu: entries and wording | M11-03 |
| Does GitKraken persist column widths per repository or globally? | M11-02 |
| GitKraken word wrap in the split view: are both sides kept aligned? | M11-07 |

## Milestone acceptance

- [ ] Dark and Light look exactly as before (snapshot comparison); Classic Dark has row tint, filled pills with connector, header band, separators and panel depth.
- [ ] Columns can be resized, set to optimal width by double click, shown/hidden from the gear menu and Preferences; settings survive a restart; no automatic hiding.
- [ ] Stashes show as dashed squares with an icon and "WIP on <branch>"; a new stash without message gets Git's default message.
- [ ] Preferences → Editor settings apply to diffs, blame, file history and the conflict view.
- [ ] Syntax highlighting in all diff modes, blame, file history and the conflict view, correct across multi-line constructs; large files fall back to plain with a note; no frame over 50 ms while scrolling a 2 MB file (`docs/PERFORMANCE.md`).
- [ ] New dependencies pass cargo-deny and appear in the third-party notices; user guide, UX-PARITY, release checklist and CHANGELOG updated; `./scripts/check.sh` green; CI green.

No release (ADR-042): when all tasks are done, continue with M12.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M11-01 | Style tokens and the Classic Dark theme | – |
| M11-02 | Resizable graph columns | – |
| M11-03 | Column menu and visibility | M11-02 |
| M11-04 | Classic Dark graph look | M11-01 |
| M11-05 | Stash node and label | M11-01 |
| M11-06 | Preferences → Editor | – |
| M11-07 | Word wrap in diff views | M11-06 |
| M11-08 | Highlighting engine | – |
| M11-09 | Highlighting in all views | M11-06, M11-08 |
| M11-10 | Documentation and performance | M11-01 … M11-09 |

---

## Tasks

### [ ] M11-01 Style tokens and the Classic Dark theme

- Tokens per the decisions table; Dark and Light values reproduce today's look; `ThemeChoice::ClassicDark` with its palette; theme choice in Preferences → Appearance and the palette's theme switch.
- ADR-048.

Acceptance: snapshots of main window, Preferences and dialogs in Dark and Light unchanged (pixel comparison against the current snapshots); Classic Dark snapshots checked by eye; settings migration test.

### [ ] M11-02 Resizable graph columns

- Draggable header borders, double click for optimal width, minimum widths, Graph column clipping with fade, persistence.

Acceptance: unit tests for the width model (drag, minimums, optimal width per column, message fill); app tests for drag messages and restore after restart; snapshots with a narrow Graph column.

### [ ] M11-03 Column menu and visibility

- Gear menu with Author, Date, SHA and "Reset column widths"; the same options in Preferences → Appearance; automatic hiding removed.

Acceptance: app tests for toggles from both places and reset; snapshot of the menu; narrow-window snapshot clips instead of hiding.

### [ ] M11-04 Classic Dark graph look

- Row tint, filled pills with connector line, header band, column separators, panel depth, all driven by tokens.

Acceptance: snapshots in Classic Dark (selected row, hover, several lanes, checked-out branch, tags, remote pills) checked by eye; Dark/Light unchanged.

### [ ] M11-05 Stash node and label

- Node drawing and label rule per the decisions table in graph, STASHES section and details; stash push without empty `-m`.

Acceptance: unit tests for the label rule (default message, empty, custom, other branch names with `:`); integration test: "Stash" without message → Git's default message; snapshots in all three themes.

### [ ] M11-06 Preferences → Editor

- Category and settings per the decisions table; font list from the font database; font, size, tab size and line numbers applied to diffs, blame, file history and conflict view.

Acceptance: settings migration test; app tests that the views use the settings (row height follows font size, tabs expanded, numbers hidden); snapshots with a different font size and without line numbers.

### [ ] M11-07 Word wrap in diff views

- Wrapping per the decisions table in Hunk, Inline and Split; selection and staging unchanged.

Acceptance: unit tests for the row-count model (prefix sums, resize, split alignment); app tests: line selection on a wrapped line stages the logical line; performance check on the 2 MB file; snapshots wrapped in all modes.

### [ ] M11-08 Highlighting engine

- Module `highlight` (pure), syntect + two-face, theme mapping, size limits, cache, background task API; dependencies per CLAUDE.md rules, ADR-047, cargo-deny, notices.

Acceptance: unit tests: Rust and Python samples get keyword/string/comment spans; a multi-line comment spanning a hunk boundary keeps its style on every line; unknown extension → plain; size limit → plain; cache hit on the same blob.

### [ ] M11-09 Highlighting in all views

- Diff modes (Hunk, Inline, Split), blame, file history, conflict view columns and output editor; plain first, highlighted when ready; the Editor setting switches it off.

Acceptance: app tests: a diff gets spans after the background result, switching the setting off removes them, a stale result for another file is ignored; snapshots of a Rust diff in all modes and themes; conflict view snapshot.

### [ ] M11-10 Documentation and performance

- `docs/PERFORMANCE.md`: highlighting and wrap measurements (2 MB file, scrolling, first paint).
- User guide `settings.md` (Appearance, Editor, columns), `daily-workflow.md` (columns, stashes), `coming-from-gitkraken.md`.
- UX-PARITY rows and the deviation "no End of Line Character setting (no in-app file editor)".
- `docs/RELEASE-CHECKLIST.md` section "7e. New in M11" (Classic Dark, columns, stash look, editor settings, highlighting on Windows fonts such as Consolas).
- CHANGELOG `[Unreleased]`.

Acceptance: docs reviewed; measurements recorded.
