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
- Left panel: tree order, tag folders, checked-out marker
- Start screen buttons
- Preferences in a modal
- Panel toggles in the toolbar
- README screenshot generated from a fixture

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
| Tree order | In LOCAL, REMOTE (inside each remote), GITFLOW and TAGS every level of the tree lists its branches (or tags) first, then its folders; both groups case-insensitive alphabetical (`compare_names`). Remotes stay alphabetical. Maintainer decision 2026-11-11; replaces "folders first" (M7-14) and the mixed GITFLOW order (M9-06). UX-PARITY deviation. |
| Tag folders | TAGS is a tree at `/` like the branch sections: same row components, folder collapse remembered per repository, filter applies, tree order above. Tag rows keep the tag icon. |
| Checked-out marker | Like GitKraken: the checked-out branch shows the check **and** the branch icon. The check sits in the gutter left of the branch icon (the indent column of the parent level); icons and labels of all rows of the same depth stay aligned and nothing moves when another branch is checked out. LOCAL and GITFLOW. Highlighted row unchanged. |
| Start screen buttons | "Open a repo", "Clone a repo" and "Start a local repo" have the same fixed size (width and height, also without a shortcut line), filled with the accent color (the color of the recent repository links) instead of grey, with hover and pressed states. Label and shortcut line on the button meet WCAG AA (4.5:1) in every theme; the shortcut line ("Ctrl+O") stays readable. Maintainer request 2026-11-11 (the shortcut hint was hard to read, grey felt uninviting). |
| Preferences modal | Preferences open as a centered modal over the window instead of a full view: width min(80 % of the window, 960 px), height min(85 %, 680 px); below 720 × 520 logical px it fills the window. Category list left, content scrolls, close button "×" and Esc close it; a click on the backdrop does not close. Focus stays inside the modal (Tab order), the app behind is dimmed. Entry points unchanged; changes apply immediately as today. API `open_preferences(page)` for later deep links (M12-12). UX-PARITY deviation "GitKraken shows Preferences as a full view" (maintainer decision 2026-11-11). |
| Panel toggles | Two icon toggle buttons at the right end of the toolbar, like VS Code: left panel (Lucide `panel-left`) and commit panel (`panel-right`), vendored like the other icons. Active state highlighted, tooltip "Hide left panel (Ctrl+J)" / "Show left panel (Ctrl+J)", likewise for Ctrl+K. Only in repository tabs. Maintainer request 2026-11-11 (toggling was only possible by shortcut). |
| README screenshot | A scripted fixture repository with fictional authors and neutral names (several branches with folders, a merge, tags, a stash, Git flow initialized, a dirty working tree) is rendered headless with `iced_test` at 1600 × 900, 100 %, Dark theme, into `docs/screenshots/main-dark.png` (`scripts/screenshot.sh`, test ignored by default). The README uses it; the old `m5-main-dark-100.png` is removed. `.claude/commands/next-task.md` step 4 (milestone close) gets "run `scripts/screenshot.sh` and commit the updated screenshot". |

## Maintainer answers and GitKraken verification items

Answered (GitKraken 7.5.2 screenshots, 2026-11-11): header gear at the right end; UI Customization has "Show commit author/date/sha in graph"; Editor page: Font, Font Size, Tab Size, End of Line Character, Syntax Highlighting, Show Line Numbers, Word Wrap; the stash node is a dashed square with an icon, labelled "WIP on develop"; rows in the graph are tinted in the lane color and filled pills connect to their node. Order of folders and branches: maintainer decision (2026-11-11) branches first, then folders, alphabetical, in every tree section; GitKraken's order is not followed. Checked-out branch: GitKraken shows a check and the branch icon.

Open:

| Item | Used in |
|---|---|
| GitKraken's column menu: entries and wording | M11-03 |
| Does GitKraken persist column widths per repository or globally? | M11-02 |
| GitKraken word wrap in the split view: are both sides kept aligned? | M11-07 |
| GitKraken: buttons to toggle the left panel and the commit panel? Where? | M11-14 |

## Milestone acceptance

- [ ] Dark and Light look exactly as before (snapshot comparison); Classic Dark has row tint, filled pills with connector, header band, separators and panel depth.
- [ ] Columns can be resized, set to optimal width by double click, shown/hidden from the gear menu and Preferences; settings survive a restart; no automatic hiding.
- [ ] Stashes show as dashed squares with an icon and "WIP on <branch>"; a new stash without message gets Git's default message.
- [ ] Preferences → Editor settings apply to diffs, blame, file history and the conflict view.
- [ ] Syntax highlighting in all diff modes, blame, file history and the conflict view, correct across multi-line constructs; large files fall back to plain with a note; no frame over 50 ms while scrolling a 2 MB file (`docs/PERFORMANCE.md`).
- [ ] Every tree section lists branches/tags before folders, alphabetical at every level; TAGS shows folders; the checked-out branch shows the check and the branch icon with aligned rows.
- [ ] Start screen buttons are equal in size, use the accent color and pass AA contrast in all themes.
- [ ] Preferences open as a modal; Esc closes it.
- [ ] Toolbar toggles hide and show the left and the commit panel.
- [ ] The README screenshot is generated by `scripts/screenshot.sh`.
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
| M11-10 | Documentation and performance | M11-01 … M11-09, M11-11 … M11-15 |
| M11-11 | Tree order, tag folders, checked-out marker | – |
| M11-12 | Start screen buttons | M11-01 |
| M11-13 | Preferences in a modal | M11-06 |
| M11-14 | Panel toggles in the toolbar | – |
| M11-15 | README screenshot from a fixture | M11-04, M11-11 … M11-14 |

---

## Tasks

### [x] M11-01 Style tokens and the Classic Dark theme

- Tokens per the decisions table; Dark and Light values reproduce today's look; `ThemeChoice::ClassicDark` with its palette; theme choice in Preferences → Appearance and the palette's theme switch.
- ADR-048.

Acceptance: snapshots of main window, Preferences and dialogs in Dark and Light unchanged (pixel comparison against the current snapshots); Classic Dark snapshots checked by eye; settings migration test.

Note (2026-11-11): done. `ThemePalette` tokens `row_tint_alpha`, `ref_pill` (`PillStyle`), `ref_connector`, `header_band`, `column_separator`, `panel_shadow`, `node_style` (`NodeStyle`); `ThemeKind` and `app_theme(kind)`; `palette(theme)` tells Classic Dark from Dark by its background color (no allocation in style functions). Only `header_band` is drawn yet (the column header, equal to `bg_panel` in Dark and Light); the graph tokens are used by M11-04. Classic Dark colors as planned; derived values: hover `#2c313a`, selection `#1d4a5c`, border `#323842`; all text and lane contrast tests pass for the three palettes. There is no palette theme switch yet (it comes with M12-12 "Set: Theme ›"); the theme is chosen in Preferences → Appearance. Pixel comparison: 50 existing snapshots rendered before and after, 0 differ. Snapshots `ui_check_classic_dark` (main window, Preferences, dialog) checked.

### [x] M11-02 Resizable graph columns

- Draggable header borders, double click for optimal width, minimum widths, Graph column clipping with fade, persistence.

Acceptance: unit tests for the width model (drag, minimums, optimal width per column, message fill); app tests for drag messages and restore after restart; snapshots with a narrow Graph column.

Note: a border belongs to the column on its left; for Author, Date and SHA (right of the message, which fills) dragging right also widens them and narrows the message. The Graph column follows the lanes (2 … 12) until it is first resized (`graph: null` in `settings.json`). The fade goes to the row background (graph background or the selected row's tint); canvas initials of a node cut off by the column are hidden, since canvas text ignores the clip. The automatic hiding stays until M11-03; it uses the current widths. Persistence: global, as planned (open item answered by the plan).

### [x] M11-03 Column menu and visibility

- Gear menu with Author, Date, SHA and "Reset column widths"; the same options in Preferences → Appearance; automatic hiding removed.

Acceptance: app tests for toggles from both places and reset; snapshot of the menu; narrow-window snapshot clips instead of hiding.

Note: menu wording as planned (open GitKraken item stays for the maintainer); the menu closes after each choice like the other menus (checked entries are a new menu entry kind). The search and gear buttons lie over the right end of the header (header background behind them) instead of taking room, so the header cells line up with the rows; before, AUTHOR, DATE and SHA sat 25 px left of their columns. Reset restores the widths, not the visibility. Below 120 px for the message the table keeps that minimum and is clipped at the right.

### [x] M11-04 Classic Dark graph look

- Row tint, filled pills with connector line, header band, column separators, panel depth, all driven by tokens.

Acceptance: snapshots in Classic Dark (selected row, hover, several lanes, checked-out branch, tags, remote pills) checked by eye; Dark/Light unchanged.

Note: tint drawn in the graph canvas from the node to the right edge of the Graph column, not on the selected row (its own tint stays). The connector runs from the pill to the refs cell edge and on in the canvas, which now starts at the column edge (no left cell padding; `lane_x` adds it). `NodeStyle::Ringed` draws a ring with a gap around initials and avatars. Separators only in the header (line at the right edge of every header cell); panel depth as 6 px gradients at both graph edges. Pills of a row use the lane color of its commit (also tags and remote branches); the Preferences "Default" badge keeps the kind color. Pixel comparison: all Dark/Light snapshots equal except one anti-aliased pixel at 150 % (`sharp-icons-150`) from the moved canvas origin; `stash-template-1-preset` shows the current time. Snapshot `ui_check_classic_dark_graph` checked.

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
- User guide: left panel order and tag folders, Preferences modal, panel toggles; `coming-from-gitkraken.md`.
- `docs/RELEASE-CHECKLIST.md` section "7e. New in M11" (Classic Dark, columns, stash look, editor settings, highlighting on Windows fonts such as Consolas, tree order, tag folders, check plus branch icon, start screen colors, Preferences modal, panel toggles).
- CHANGELOG `[Unreleased]`.

Acceptance: docs reviewed; measurements recorded.

### [ ] M11-11 Tree order, tag folders, checked-out marker

- `branch_tree` order: leaves before folders, both by `compare_names`, recursive; one rule for LOCAL, REMOTE, GITFLOW and TAGS. TAGS built as a tree. `sidebar::ref_item`: check in the gutter, branch icon kept.
- UX-PARITY: left panel row and the new deviation; M7-14 and M9-06 notes point here.

Acceptance: unit tests for the order (leaves first, case-insensitive, nested folders, ties); tree tests for tags with and without `/`; snapshots of LOCAL with the checked-out branch at depth 0 and inside a folder (both themes) with aligned icons and labels; `ui_check_gitflow_section` updated.

### [ ] M11-12 Start screen buttons

- Equal size, accent fill, states and contrast per the decisions table, in all three themes.

Acceptance: contrast unit test (WCAG ratio) for label and shortcut line in Light, Dark and Classic Dark; snapshots of the start screen in all three themes at 100 and 125 %.

### [ ] M11-13 Preferences in a modal

- Modal per the decisions table; all categories, the repository group and the start-screen case work as before.

Acceptance: app tests: open from every entry point, Esc and "×" close, backdrop click does not close, focus stays inside, `open_preferences(page)` selects the page; layout test for the size rule incl. a small window; snapshots in both themes.

### [ ] M11-14 Panel toggles in the toolbar

- Two toggle buttons per the decisions table, synchronized with Ctrl+J/Ctrl+K and the palette.

Acceptance: app tests (click and shortcut give the same state, tooltips follow the state, not on the start screen); toolbar snapshots with both panels shown and hidden.

### [ ] M11-15 README screenshot from a fixture

- Fixture script, ignored snapshot test, `scripts/screenshot.sh`, README, `next-task.md` step 4.

Acceptance: the script produces the PNG on Linux CI tooling without a display; README renders it; no real names or e-mail addresses in the fixture.
