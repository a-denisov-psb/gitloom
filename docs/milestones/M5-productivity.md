# M5 – Productivity parity

Target version: **0.6.0**

## Goal

The remaining everyday GitKraken features work: command palette, commit search, file history and blame, hide and solo branches, a settings view, light and dark theme, zoom, a real icon set, opt-in avatars, external diff/merge tools and opening the repository in a terminal or file manager. Every shortcut marked M5 in `docs/UX-PARITY.md` works.

## Scope

In scope:

- Action registry as single source for shortcuts, palette and shortcut list
- Settings file and settings view
- Light and dark theme, zoom, fullscreen
- SVG icon set
- Command palette (actions, open repository, checkout branch, file list)
- Commit search
- Hide and solo branches
- File history and blame
- Avatars (opt-in)
- External diff and merge tools
- Open repository in terminal and file manager

Out of scope (later):

- Diff view modes inline/split, column resizing and column menu in the graph → backlog
- Git identity / profiles in settings → backlog
- Integrated terminal (Gitloom opens the system terminal) → backlog
- M4 known limitations (HANDOVER open question 9) → backlog
- Git flow graph layout → backlog (after 1.0)
- Installers, update check, accessibility pass → M6

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Action registry | One table of all user actions in `app` (id, label, category, context, default shortcut, enabled rule with reason). Shortcuts, command palette, shortcut list and toolbar use it. No user-defined key bindings in M5. |
| Settings | Separate `settings.json` next to `session.json` (preferences vs. state), versioned, written atomically; `auto_fetch_minutes` moves there with migration (ADR-023). The settings view replaces the main area like GitKraken's preferences: category list left, form right. Opened with Ctrl+, , a gear button in the toolbar and the palette. Changes apply immediately. |
| Theme | Dark (default) and Light; "System" only if iced 0.14 can detect the OS theme (ADR-027). All colors come from a runtime palette; views use no color constants. |
| Zoom | Window scale factor 70 %–200 % in 10 % steps, Ctrl+= / Ctrl+- / Ctrl+0, persisted; F11 toggles fullscreen (not persisted). |
| Icons | Lucide (ISC) as vendored SVG files, rendered with iced's `svg` feature and tinted by the theme (ADR-025). Only icons in use are vendored; the license file is included. |
| Command palette | Ctrl+P overlay at top center: fuzzy filter over all registered actions with their shortcuts. Sub-lists: "Open repository" (Ctrl+Shift+O: recent repositories + Browse…), "Checkout branch", "File history" (Ctrl+Shift+H: tracked files). Disabled actions are listed with their reason and cannot run. Own fuzzy scorer, no dependency. |
| Commit search | Ctrl+F (or the magnifier above the graph) opens a search field above the graph. Matches summary/message, author name/email and SHA prefix in the loaded history; non-matching rows are dimmed, "n of m" counter, Enter / Shift+Enter jump to the next/previous match, Esc closes. Runs off the UI thread with a 150 ms debounce. |
| Hide and solo | Local and remote branches can be hidden (eye on hover in the sidebar, context menus) or soloed (context menu). The graph shows only commits reachable from visible branches and HEAD; tags and stashes appear when their commit is visible. Remote headers offer "Hide all" / "Show all". Stored per repository in the session file. Pure reachability filter in `git-graph`, then the normal layout. |
| File history and blame | Read through the Git CLI: `git log --follow` and `git blame --porcelain` (ADR-024). The view replaces the graph like the diff view: history list left, the selected commit's diff of that file right; History / Blame toggle in the header. |
| Avatars | Opt-in, off by default (ADR-009): Gravatar with SHA-256 email hash, disk cache with 7-day TTL including misses, at most 4 parallel requests, only for visible rows and the details panel. Off = no request at all. Initials remain the fallback (ADR-026). |
| External tools | Delegate to Git: `git difftool` / `git mergetool` with `--no-prompt`; settings choose "Use Git config" (default), an installed tool from `--tool-help`, or none. Runs in the background, refreshes when the tool exits. |
| Terminal and file manager | Open the system terminal / file manager in the repository root (Alt+T, Alt+O, toolbar Terminal button). Terminal auto-detected per platform, overridable by a command template with `{path}`. Deviation: GitKraken's button opens its integrated terminal (UX-PARITY entry). |

## Dependencies to add

| Dependency | Used in | Note |
|---|---|---|
| iced features `svg`, `image` | M5-05, M5-11 | Same iced version |
| `ureq` (rustls, proxy from env) | M5-11 | ADR-026; latest stable ≥ 14 days old |
| `sha2` | M5-11 | Gravatar hash |
| Lucide SVG icons (vendored, ISC) | M5-05 | ADR-025; no crate |

## Maintainer verification items

| Item | Used in |
|---|---|
| Commit search: does GitKraken dim non-matching commits or filter them out? | M5-07 |
| Hide/solo: can the checked-out branch be hidden; where exactly are the controls? | M5-08 |
| File history and blame: entry points and layout (list position, History/Blame toggle) | M5-09, M5-10 |
| Command palette: which sub-lists does GitKraken offer (open repository, checkout, file history)? | M5-06 |

Until answered, implement the decisions above; adjust afterwards.

## Milestone acceptance

- [ ] Every shortcut marked M5 in UX-PARITY works (Windows/Linux; macOS mapping covered by tests); Ctrl+/ lists all registered shortcuts.
- [ ] Every registered action is reachable from the command palette.
- [ ] On this repository and a generated 100 000-commit repository, search, hide/solo, palette, file history and blame meet the targets of M5-14.
- [ ] All views are readable in Light and Dark at 70 %, 100 % and 200 % zoom (screenshots in `docs/screenshots/`).
- [ ] With avatars off, Gitloom opens no network connection besides Git's own fetch/pull/push.
- [ ] `./scripts/check.sh` green on all CI platforms, docs and CHANGELOG up to date.
- [ ] Released as `v0.6.0` (M5-15).

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M5-01 | Action registry and shortcut list | – |
| M5-02 | Settings file and settings view | – |
| M5-03 | Theme system: dark and light | M5-02 |
| M5-04 | Zoom and fullscreen | M5-01, M5-02 |
| M5-05 | SVG icon set | M5-03 |
| M5-06 | Command palette | M5-01 |
| M5-07 | Commit search | M5-01 |
| M5-08 | Hide and solo branches | M5-05 |
| M5-09 | File history | M5-06 |
| M5-10 | Blame | M5-09 |
| M5-11 | Avatars (opt-in) | M5-02 |
| M5-12 | External diff and merge tools | M5-01, M5-02 |
| M5-13 | Terminal and file manager | M5-02, M5-05 |
| M5-14 | End-to-end check, docs, performance | all others |
| M5-15 | Release v0.6.0 | all |

---

## Tasks

### [x] M5-01 Action registry and shortcut list

> Done: `actions.rs` holds `ActionId` (the former `Shortcut` enum, kept as an alias) and the registry with label, category, context (global, graph, staging, text field, plus "rebase panel" for P/R/S/F/D and Alt+↑/↓), chords and availability; `shortcuts.rs` maps keys through it. Chords match Shift exactly (Shift+F5, Shift+Ctrl+Home and Ctrl+Alt+Shift+F no longer trigger; nothing in UX-PARITY uses them). Punctuation that needs Shift on the layout is matched by the shifted key (Ctrl+/ on a German keyboard, Ctrl++). Ctrl+/ opens "Keyboard shortcuts" (grouped, filter, Esc closes even from the focused filter); M5 actions are listed and refused with "Not available yet" in the status bar. Menu and tooltip shortcut texts come from the registry. Tests: no duplicate chord per context (both platforms), every chord dispatches to its action (both platforms), macOS labels (⌘, ⇧, ⌥, ⌃), shifted punctuation, list filter, app (open, filter, Esc, disabled action). Checked by hand: Ctrl+/ → list, typing "zoom"/"tab" filters, Esc closes.

- `actions.rs`: `ActionId` plus a table with label, category (the UX-PARITY sections: Repo actions, Navigation, Command palette and search, Interface, Tabs), context (global, graph, staging, text field), default shortcut (Ctrl → ⌘ on macOS) and an enabled rule returning the reason when disabled.
- Existing shortcut handling (`shortcuts.rs`) dispatches through the registry; behavior unchanged.
- Ctrl+/ opens the overlay "Keyboard shortcuts", grouped by category, with a filter field; Esc closes.
- Add every M5 action to the registry when its task lands; until then it is listed disabled with "Not available yet".

Acceptance: unit tests: no duplicate shortcut per context, macOS mapping, dispatch of all existing shortcuts (existing shortcut tests stay green); overlay checked by hand.

### [x] M5-02 Settings file and settings view

> Done: `settings.rs` (versioned `Settings` with general/appearance/tools, `#[serde(default)]` everywhere so missing and unknown fields fall back to defaults; a file without version counts as version 1, a newer version or a broken value resets to defaults and keeps `settings.json.bak`; atomic writes through `session::save`, debounced 300 ms). Path: `settings.json` next to `session.json`. Migration: without a settings file, `auto_fetch_minutes` is read from the session file and the new file is written once; the session no longer has the field (old files keep it until the next session save, it is not read again). Settings view replaces the tab content (tabs stay visible): General with the auto-fetch input (applies per valid keystroke, invalid input shows the reason, timer restarts), Appearance and External tools with disabled controls ("Not available yet"). Opens with Ctrl+, and the gear button (toolbar right, "Settings"); Esc (also from the focused input) or Close returns; other shortcuts wait while it is open. Tests: defaults, round trip, missing/unknown fields, broken values, newer version, migration (once), corrupted file, input parsing, app (open, apply, invalid, Esc, timer restart, loaded settings). Checked by hand: old session with 9 minutes → settings.json with 9; Ctrl+, → General shows 9; "1x" → red hint; 15 → written to settings.json; Appearance list; Esc back to the graph.

- `settings.rs`: versioned `Settings` (general: auto-fetch interval; appearance: theme, zoom, avatars; external tools: diff tool, merge tool, terminal command), `settings.json` in the config dir, atomic writes, missing/unknown fields → defaults.
- Migration: `auto_fetch_minutes` is read from the session file once and moved to settings.
- Corrupted file: defaults, status message, broken file kept as `settings.json.bak`.
- Settings view in the main area: categories General, Appearance, External tools; this task builds General (auto-fetch interval, 0 = off; timer restarts on change). Controls of later tasks are shown disabled with "Not available yet" until they land.
- Open with Ctrl+, , the gear button (toolbar right) and the palette; Esc or Close returns. UX-PARITY row for Ctrl+, (GitKraken: Preferences).
- ADR-023.

Acceptance: unit tests for defaults, roundtrip, unknown fields, migration, corrupted file; view checked by hand.

### [x] M5-03 Theme system: dark and light

> Done: `theme.rs` has `ThemePalette` with `DARK` (today's values) and `LIGHT`; every style function takes the palette from the iced theme it gets (`palette(theme)`, by `extended_palette().is_dark`), text colors use `.style(theme::text_*)`, the graph canvas draws with the palette of its theme, and no view uses a color constant any more. Light warning text is darker (#875a00) to reach 4.5:1 on panels. iced 0.14 detects the OS theme (`iced::system::theme()` at start, `system::theme_changes()` subscription), so Appearance offers Dark / Light / System; an unknown mode (`Mode::None`, e.g. WSLg) falls back to Dark and the view says so. The askpass window reads the choice from `settings.json` (System counts as Dark there). Conflict and rebase views got the graph background (they showed the divider color, visible in Light). Tests: contrast of text (all backgrounds) and muted text (graph, panel, chrome) ≥ 4.5 in both palettes, light status colors ≥ 4.5 on panels, lanes ≥ 3:1 and distinct, palette follows the theme, app (choice, system mode, fallback). Screenshots `docs/screenshots/m5-{start,graph,diff,conflict,rebase}-{dark,light}.png`; switching Dark → Light in Preferences applies at once (graph included) and is saved.

- `theme.rs`: constants become a `ThemePalette` (all current colors and lane colors) chosen at runtime; Dark keeps today's values, Light is new.
- Text, muted text and selected text reach WCAG AA (4.5:1) on their backgrounds in both palettes; lane colors stay distinguishable on the light background.
- Appearance → Theme: Dark / Light, plus System if iced 0.14 can detect the OS theme (check the iced source; record the result in ADR-027).
- Switching applies without restart, including canvas-drawn parts (graph, conflict view).

Acceptance: unit tests for the contrast ratios of both palettes; screenshots of start screen, graph, staging, diff, conflict view and interactive rebase in both themes.

### [x] M5-04 Zoom and fullscreen

> Done: `settings::{zoom_step, clamp_zoom}` (70–200 %, 10 % grid, off-grid values from an edited file snap, out-of-range values are clamped on load); `App::scale_factor` feeds iced's `scale_factor`; Ctrl+= / Ctrl++ (also Ctrl+Shift+= and numpad +) / Ctrl+- / Ctrl+0 work also while an overlay or the settings view is open; the level is saved, shown in Appearance (−, value, +, Reset) and flashed in the status bar for 2 s ("Zoom 120 %"). F11 toggles fullscreen via `window::mode`/`set_mode` (not persisted). `App::window_size()` converts the last reported window size to the current zoom (iced sends no resize when only the scale changes), so menus are clamped correctly. Found by hand: at 200 % a menu that fits neither below nor above the cursor jumped to the top; `ContextMenu::origin` now keeps it as low as the window allows. Tests: steps and bounds, app (shortcuts, persistence, flash expiry, zoom while settings open, window size), menu origin without room. Checked by hand at 200 % and 70 % on this repository and a small one: wheel-scrolled graph, right click → menu at the cursor; branch pill dragged onto another → merge/rebase menu at the drop point; F11 → 2560×1440 and back; Ctrl+= → 110 % saved.

- Ctrl+= (also Ctrl++ and numpad +), Ctrl+-, Ctrl+0: 70 %–200 % in 10 % steps via the iced scale factor; persisted in settings, shown in Appearance and briefly in the status bar ("Zoom 120 %").
- F11 toggles fullscreen.
- Virtualization, hit testing, context menu positions and drag and drop stay correct at every zoom level.

Acceptance: unit tests for steps and bounds; by hand at 70 % and 200 %: graph scrolling, context menu position in a scrolled graph, drag and drop of a branch.

### [x] M5-05 SVG icon set

> Done: iced feature `svg`; 35 Lucide 1.55.0 icons (newest release ≥ 14 days old) plus the ISC `LICENSE` in `crates/app/assets/icons/`; `icons.rs` embeds them with `include_bytes!`, caches one `svg::Handle` per icon and tints them from the palette (`tinted`, `button_icon` dims icons of disabled buttons, `change_icon` uses the change colors). Replaced: toolbar ↶ ↷ ↓ ↑ ⑂ ⊕ ⊖ ›_ ⚙ and the ▾ of pickers and the Pull dropdown; tab × and +; diff ×; sidebar ▾/▸, section icons (monitor, cloud, tag, archive, package), ✓ of the checked-out branch and the "+" of REMOTE; graph WIP ✎/✓, ✓ in the HEAD pill and the " · origin" remote text (now a cloud icon with the remote names as tooltip; the "+N" tooltip lists "main (origin)"); file status letters A/M/D/R/?/! (square-plus/pen/minus/arrow, dashed square, triangle); banner icons (info, merge, alert); ⋮⋮ grip in the rebase panel; ⟳ in the status bar; −/+ zoom buttons. Eye, eye-off and solo icons are vendored for M5-08, search for M5-07. Kept as text on purpose: ↑/↓ ahead/behind counts (numbers with arrows, like GitKraken), → in rename labels, · separators, and the ✓ in front of the current Pull mode inside the menu (menus have text labels only). Icon-only buttons have tooltips (tab close "Close tab", "New tab", diff close "Close diff (Esc)", "Add remote"). Tests: icon table order, every icon is a Lucide SVG with `currentColor`, license vendored; pill labels carry remotes separately. Screenshots `docs/screenshots/m5-*-{dark,light}.png` retaken with icons.

- iced feature `svg`; Lucide SVGs in `crates/app/assets/icons/` with the license file; `icons.rs` embeds them (`include_bytes!`) and tints them with theme colors.
- Replace the Unicode placeholders: toolbar (undo, redo, pull, push, branch, stash, pop, terminal, settings), sidebar sections, file status, banners, remote marker on pills (cloud icon, remote name as tooltip), eye/solo icons for M5-08.
- Every icon-only button has a tooltip.
- ADR-025.

Acceptance: no Unicode placeholder icons left (list the replaced ones in the task note); screenshots in both themes.

### [x] M5-06 Command palette

> Done: `fuzzy.rs` (subsequence; bonuses for word starts incl. camelCase humps and path separators, consecutive characters, same case, first character; gap penalty; a mid-word match is skipped when the character starts a word later; stable ties). `palette.rs` (mode, entries, query, top 200 matches, wrap-around selection, disabled entries not runnable, Backspace on an empty query leaves a sub-list without the Backspace that emptied it, recent actions first, loading state). App: Ctrl+P lists every registered action except the palette itself, with the shortcut right-aligned and the reason for disabled ones (later task, "Open a repository first", "Only in the interactive rebase panel"); arrows work while the query field has focus, Enter / click runs, Esc closes (also from the field). Sub-lists: "Open recent repository…" (Ctrl+Shift+O: Browse… + recent repositories with path), "Checkout branch…" (new registry action, local and remote branches, the checked-out one disabled, runs the existing checkout flow incl. "Stash and checkout"). The file list mode (loading state, entries) is prepared for M5-09. Registry: `Requires::Repository` for actions that need an open repository. Tests: scorer ordering, palette state, app (run + recent, disabled with reason, sub-lists, checkout). Filtering 100 000 paths: **≤ 8.1 ms** (release, `palette_filter_speed`). Checked by hand: Ctrl+P → list with shortcuts and disabled reasons, typing "chb" → "Checkout branch…" first. Enter, arrows and Backspace could not be injected (WSLg keyboard stopped reaching the window) → maintainer to-do; covered by app tests.

- Pure module `fuzzy.rs`: subsequence match with bonuses for word starts, consecutive characters and case; stable order for ties.
- Overlay at top center: query field, results with the shortcut right-aligned, disabled entries with their reason; ↑/↓, Enter, Esc, mouse. Empty query lists recently used actions first (in memory).
- Sub-lists: "Open repository" (Ctrl+Shift+O; recent repositories + Browse…), "Checkout branch" (local and remote; existing checkout flow incl. "Stash and checkout"), file list mode for M5-09. Backspace on an empty query returns to the action list.

Acceptance: unit tests for scorer ordering and palette state (filter, selection wrap, disabled entries not executable, sub-list navigation); filtering 100 000 file paths < 30 ms (release build).

### [x] M5-07 Commit search

> Done: `search.rs` — `matches` (case-insensitive substring in summary, body, author, email; ASCII queries without allocation per commit, others via `to_lowercase`; SHA prefix from 4 hex digits, prefixes only) and `SearchState` (query, ascending commit indices, current match, request number, jump flag, "n of m", wrap-around steps). The snapshot holds full messages, so bodies are searched. App: Ctrl+F (also from the palette) or the magnifier at the right end of the column header opens the bar above the graph (field, counter, ↑/↓ buttons, ×); typing searches in `spawn_blocking` after 150 ms and jumps to the first match unless the selected commit matches; Enter / Shift+Enter step with wrap-around and scroll the match into view; stale results are dropped; a new snapshot runs the search again without moving the selection; Esc (also from the focused field) closes and keeps the selection. Non-matching rows: faint summary and a faint graph node (lines stay). Tests: match rules (fields, case, trim, SHA length/prefix, non-ASCII, ASCII search edges), state (counter, dimming, steps, kept current), app (jump, steps, wrap, Shift+Enter, kept selection, stale results, cleared query, refresh re-run, Esc). 100 000 commits with bodies: **≤ 12.4 ms** (release, `search_speed`). Checked by hand: magnifier → bar with field, buttons and tooltips. Typing could not be injected (WSLg keyboard) → maintainer to-do (typing, dimming, Enter/Shift+Enter, Esc).

- Search field above the graph per planning decision; magnifier button and Ctrl+F open it, also from the palette.
- Pure match function over the loaded snapshot: case-insensitive, summary/message, author name and email, SHA prefix (≥ 4 hex characters). If the snapshot holds summaries only, search summaries and note it.
- Matches re-run after a refresh; jumping selects the commit and scrolls it into view; Esc closes and keeps the current selection.

Acceptance: unit tests for the match rules; app tests for next/previous with wrap-around and refresh; 100 000 commits < 50 ms (release build).

### [x] M5-08 Hide and solo branches

> Done: `git_graph::visible_commits(commits, tips)` (iterative reachability, ignores missing parents and unknown tips). App: `ref_filter.rs` (`RefFilter` with full ref names; shown = soloed-if-any and not hidden; remote "Hide all"/"Show all"; prune), `RepoSnapshot::with_filter` (tips = shown branches + HEAD, stash rows follow their base, tags stay in `refs` for the sidebar and appear on visible commits, `unfiltered` keeps the whole history, `applied_filter` records the filter) and `App::ensure_filter` (runs in `spawn_blocking`, stale results dropped; also after a refresh, which already filters in its loader). Sidebar: eye on hover (open eye = shown, click hides; eye-off on hidden entries shows again), hidden/not-soloed entries faint with eye-off, soloed ones with the solo icon; the checked-out branch is never dimmed and has no eye. Menus: Hide/Show and Solo/Unsolo on local and remote branches (sidebar and graph pills), Hide disabled with "Checked-out branch" on HEAD; remote menu "Hide all"/"Show all". Stored in the session (`ref_filters`, per repository root). Found by hand: restoring the session opened the tabs before taking over the loaded session, so stored filters were not applied at start — fixed (session first); covered by the manual restart check, the async loader capture is not reachable from app tests. Tests: reachability (shared history, merges, solo plus hide, nothing but HEAD, missing parents, 200 000-commit chain), filter rules and persistence, filtered snapshots (tags, stashes, working tree), menus, app (hide → graph and WIP selection, session entry, show again, HEAD not hideable, solo keeps HEAD, stale result, prune). Toggle on 100 000 commits incl. layout: **66 ms** (release, `filter_speed`). Checked by hand: hover eye → hide topic (still shown through origin/topic), origin → Hide all → only main's history; Solo/Show from the menu; restart restores the filter and drops a vanished ref.

- `git-graph`: pure `visible_commits(tips, parents)`; the layout runs on the visible subset; the WIP row stays.
- Sidebar: eye on hover toggles hide; hidden entries dimmed with an eye-off icon; context menu Hide/Show and Solo/Unsolo; soloed entries marked; while any branch is soloed only soloed branches and HEAD are visible. Remote header menu "Hide all" / "Show all". Graph pill menu "Hide".
- The checked-out branch cannot be hidden (disabled, tooltip "Checked-out branch") until the maintainer verification says otherwise.
- A selected commit that becomes hidden moves the selection to the WIP row.
- Stored per repository in the session file by full ref name; refs that no longer exist are dropped.

Acceptance: unit tests for reachability (shared history, merges, solo plus hide, nothing visible but HEAD) and persistence; toggling on 100 000 commits < 200 ms including layout.

### [x] M5-09 File history

> Done: `git_core::file_history(git, path, start, cancel)` runs `git --no-optional-locks log --follow -z --name-status --format=…` through `run_network` (process tree killed on cancel) and parses id, author (mailmap), email, time, summary, status, path and the previous path of renames/copies; `git_core::tracked_files` lists HEAD with `ls-tree -r -z`. App: `CenterView::FileHistory` (`file_history.rs`; dropping the view cancels its `git log`, so closing or replacing it stops the process), list left (initials badge, summary, author · date, rename icon with "Renamed from …"), the existing diff (without close/History buttons) of the file under its path in that commit right; ↑/↓ move in the list; "Show in graph" and double click close the view and select the commit (a commit outside the graph, e.g. on a hidden branch, gets a status message). Entry points: file context menu "File history" (commit files start at that commit, staging files at HEAD; disabled for untracked files), "History" button in the diff header, Ctrl+Shift+H → palette list of tracked files. Tests: parser (renames, copies, commits without diff, empty), integration with the real binary (linear, rename followed, deleted and re-added, merged branch, spaces + non-ASCII path and `ls-tree`, cancellation, unknown path), view state (selection, preselect, no wrap, drop cancels), app (load, select, ↓, stale result, Show in graph, Esc, error, palette file list, menu rules). Checked by hand: commit details → right click `actions.rs` → File history: five commits, diff of the newest; select another, Show in graph → selected and scrolled into view.

- `git_core::file_history(path, start)`: `git --no-optional-locks log --follow -z` with a parseable format (id, author, date, summary, path at that commit); cancellable (the process is killed when the view closes or a new request starts). ADR-024.
- Entry points: right click a file in commit details or staging → "File history"; "History" button in the diff view header; Ctrl+Shift+H → palette file list (tracked files at HEAD).
- The view replaces the graph: list left (initials/avatar, summary, author, date, rename marker), right the existing read-only diff of that file for the selected commit; "Show in graph" (and double click) closes the view and selects the commit; Esc closes. Loading indicator while running.

Acceptance: integration tests: linear history, rename followed, file deleted and re-added, merge commits, path with spaces and non-ASCII characters, cancellation; checked by hand.

### [x] M5-10 Blame

> Done: `git_core::blame(git, path, rev, cancel)` runs `git --no-optional-locks blame --porcelain [rev] -- path` through `run_network` and `parse_porcelain` builds commits (author from the mailmap, email, time, summary, boundary), lines (CR stripped) and groups of consecutive lines; NUL bytes mark the file binary; `UNCOMMITTED` marks working-copy lines. App: History / Blame toggle in the file history header (Blame defaults to the working copy for a history from HEAD, else the start commit), "Blame at this commit" in the entry menu (disabled on a deletion; the entry menu also offers Show in graph), a new blame replaces and cancels the running one (`BlameView` cancels on drop). Blame view: fixed 18 px rows, only visible rows built; gutter per group (short SHA, author, relative age via `format::relative`; "Not committed yet" for working-copy lines), repeated on the first fully visible row of a group; alternating group shading; tooltip with the summary; a click selects the commit in the history list (History mode, its diff); binary and empty files show a message. Found by hand: a horizontally scrollable list collapsed the `Fill` rows to nothing — the blame scrolls vertically and clips long lines. Tests: parser (repeated commits, boundary, uncommitted, CRLF + tabs, binary, empty), integration against `git blame` (HEAD and working copy) and with `.mailmap`, `format::relative`, view state (targets, deletion, gutter selection), app (toggle → working copy, stale result, Blame at commit cancels the previous blame, gutter click, entry menu). Checked by hand (release build): blame of `crates/app/src/app.rs` (10 686 lines) of the working copy; wheel scrolling 15 ms median / 19 ms max per step; gutter click → History with that commit's diff.

- `git_core::blame(path, rev)`: `git --no-optional-locks blame --porcelain` (respects `.mailmap` and `blame.ignoreRevsFile`), parsed into line groups per commit.
- Blame mode in the file history view (History / Blame toggle) and "Blame at this commit" in the history list; for the working copy, uncommitted lines are marked "Not committed yet".
- Gutter per group: short SHA, author, relative date, alternating shading; tooltip with the full summary; clicking the gutter selects that commit in the history list.
- Virtualized rendering like the graph; binary files show a message instead.

Acceptance: parser unit tests (repeated commits, boundary commits, uncommitted lines, CRLF); integration test compared against `git blame`; a 10 000-line file scrolls smoothly (checked by hand).

### [x] M5-11 Avatars (opt-in)

> Done: `avatars.rs` — `gravatar_hash` (SHA-256 of the trimmed, lower case email), `gravatar_url` (`s=64&d=404`), `Fetcher` trait with `HttpFetcher` (ureq, rustls, 10 s timeout, proxy from the environment by ureq's default config, 1 MiB body limit), `DiskCache` (`<cache dir>/gitloom/avatars/<hash>.img|.miss`, fresh for 7 days by file time, atomic writes), `load` (cache first, then network; misses cached) and `Avatars` (email → hash without allocation for known authors, queue, at most 4 in flight). App: Appearance toggle (with the privacy note); requests only for the authors of the visible graph rows, the selected commit and the file history list, after every update; the client is created lazily on the first request and dropped when the setting goes off (avatars forgotten). Rendering: round 18 px avatar on the graph node (an `image` widget on top of the canvas, because the canvas does not round images; a ring in the lane color, faint when dimmed by the search), 32 px in the commit details, 24 px in the file history; initials everywhere else and while loading. Dependencies: `ureq 3.4` locked to 3.4.2 (3.4.3/3.5.0 are younger than 14 days), `sha2 0.11`, and iced `image-without-codecs` plus `image 0.25` with only PNG and JPEG (iced's `image` feature pulls every codec incl. an AV1 encoder) — recorded in ADR-026. Tests: hash, URL, cache TTL for images and misses, miss → image replacement, request limit, app (off: no client is created and nothing requested; on: four requests, results shown; off again: client dropped). Checked by hand with a public address that has a Gravatar and one without: round avatar in the graph and the details, initials for the other; cache files `.img` and `.miss`; with avatars off the process opened no TCP socket at all (sampled from `/proc` for 8 s), with avatars on two.

- Appearance → "Load avatars from Gravatar" (off), with the note "Sends a hash of each author's email address to gravatar.com".
- `https://gravatar.com/avatar/<sha256 of trimmed lowercase email>?s=<px>&d=404` via `ureq` (rustls, proxy from env); cache in `<cache dir>/gitloom/avatars/`, TTL 7 days, misses cached; at most 4 parallel requests; only visible rows and the details panel. ADR-026.
- Round image in the graph node and the commit details; initials as fallback and while loading.
- Setting off: no HTTP client is created and no request is made.

Acceptance: unit tests for the hash, cache TTL and misses, request limit, and "off means no request" (fetcher behind a trait, fake in tests); checked by hand against Gravatar.

### [x] M5-12 External diff and merge tools

> Done: `git_core::external_tools` — `tool_info` (`--tool-help` first section incl. user-defined `<name>.cmd`, plus the configured tool: `diff.tool` falling back to `merge.tool` like Git, or `merge.tool`), `difftool_args`/`mergetool_args`, `run_difftool` (commit → `<parent> <commit>`, root commit → empty tree from `git mktree`; unstaged; staged `--cached`; a rename passes old and new path) and `run_mergetool`; both through `GitWriter::run` (stdin closed, `GIT_EDITOR=:`), waiting until the tool exits. App: Ctrl+D (registry, also from the palette) and the "External" button in the diff header (also in the file history; disabled for untracked files and stashes) — the status bar says "Waiting for <tool>…" while the app stays usable; conflict view header and the conflicted file's context menu "Open in merge tool" — the tab is busy ("Waiting for <tool>") so writes wait, afterwards the repository is refreshed and the conflict view loads the file again if it is still conflicted (otherwise it closes). Preferences → External tools: pick lists "Use Git config (<configured> | not configured)", the tools Git lists (loaded when the category opens with a repository), "None"; a chosen tool Git no longer lists stays selectable. "None", or Git config without a configured tool → dialog "No diff tool"/"No merge tool" with "Open settings" (opens External tools). Tests: `--tool-help` parsing, argument building, integration with fake tools recording their arguments (commit, root commit, unstaged, staged, stdin closed: no hang, merge resolving the conflict), configured tool fallback; app (no diff open, "None" → dialog with Open settings, waiting status, finish, merge tool busy and conflict reload flag, tool settings stored), option list. Checked by hand with fake tools in a test repository: conflict view → Open in merge tool → resolved, refreshed, Continue merge ready; staged diff → External → the diff tool got the HEAD blob and the file.

- External tools → Diff tool / Merge tool: "Use Git config" (default; shows the configured value or "not configured"), installed tools from `git difftool --tool-help` / `git mergetool --tool-help`, "None".
- Ctrl+D and the diff header button "Open in external diff tool": commit file → `git difftool --no-prompt [--tool=X] <parent> <commit> -- <path>`; unstaged → `git difftool --no-prompt -- <path>`; staged → with `--cached`.
- Conflict view and the conflicted file's context menu: "Open in merge tool" → `git mergetool --no-prompt [--tool=X] -- <path>`; refresh afterwards, the conflict view stays if the file is still conflicted.
- stdin closed, Gitloom's editor mode stays active, nothing waits for a terminal; status bar "Waiting for <tool>…".
- No tool configured → dialog with an explanation and "Open settings".

Acceptance: integration tests with a fake tool (`difftool.<name>.cmd` / `mergetool.<name>.cmd` script recording its arguments) for commit, unstaged, staged and merge; no hang with stdin closed.

### [x] M5-13 Terminal and file manager

> Done: `launch.rs` — `terminal(platform, lookup, template, dir)` (Windows `wt.exe -d <path>` → `pwsh -NoExit` → `powershell -NoExit` → `cmd /K`, the last three in a new console; macOS `open -a Terminal <path>`; Linux `$TERMINAL`, `x-terminal-emulator`, `gnome-terminal --working-directory=`, `konsole --workdir`, `xfce4-terminal --working-directory=`, `alacritty --working-directory`, `kitty --directory`, `wezterm start --cwd`, `xterm`; always started in the repository root), template expansion (`split_words`: whitespace, single/double quotes, backslash escapes; `{path}` replaced in every word, so paths with spaces stay one argument), `file_manager` (`explorer`/`open`/`xdg-open`), `reveal_file` (takes over "Open file location": `explorer /select,`, `open -R`, `xdg-open <folder>`), `spawn` (detached, stdio null). App: Alt+T, Alt+O, the toolbar Terminal button (now enabled, tooltip with the shortcut) and the palette; Preferences → External tools → Terminal command input. Failure → status bar with the reason and a hint to the settings. With this task every registered action is implemented, so `Requires::NotYet`, `static_block` and the "Not available yet" marks of the registry were removed. Tests: detection order on all three platforms with an injected lookup (incl. a `$TERMINAL` that is not installed), template expansion (spaces, quotes, escapes, unclosed quote), file manager commands, app (every action available with a repository). Checked by hand on Linux with stub programs on `PATH` and a repository path containing a space: Terminal button → `xterm` started in the repository root; Open file location → `xdg-open <repository>`; a template `/bin/sh -c "…" {path} "second arg"` → two correct arguments; without any terminal → "Could not open a terminal: No terminal found. Set a terminal command in Preferences → External tools." Windows and macOS: maintainer to-do.

- One module `launch.rs` (also takes over the existing "Open file location" code).
- Terminal (Alt+T, toolbar Terminal button, palette) in the repository root. Detection order: Windows `wt.exe -d <path>` → `pwsh` → `powershell` → `cmd`; macOS `open -a Terminal <path>`; Linux `$TERMINAL`, `x-terminal-emulator`, `gnome-terminal`, `konsole`, `xfce4-terminal`, `alacritty`, `kitty`, `wezterm`, `xterm`. Custom command template with `{path}` in settings.
- File manager (Alt+O, palette): `explorer`, `open`, `xdg-open`.
- Failure → status message with the reason and a hint to the settings.
- UX-PARITY deviation: external terminal instead of GitKraken's integrated one.

Acceptance: unit tests for the detection order (injected executable lookup) and template expansion (spaces, quoting); checked by hand on Linux; Windows and macOS → maintainer to-do.

### [ ] M5-14 End-to-end check, docs, performance

- Walkthrough of all M5 features on Linux in both themes at 70 %, 100 % and 200 % zoom; Windows → maintainer to-do.
- PERFORMANCE.md "M5" with a generated 100 000-commit repository: search < 50 ms, hide/solo toggle < 200 ms, palette filter < 30 ms; file history of a file with 2 000 commits < 2 s; blame of 10 000 lines < 2 s; idle memory with avatars on and off.
- Update UX-PARITY (all M5 rows), CHANGELOG, screenshots, README.

### [ ] M5-15 Release v0.6.0

Same procedure as before (`/release`), only when the maintainer asks.
