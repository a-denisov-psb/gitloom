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

### [ ] M5-02 Settings file and settings view

- `settings.rs`: versioned `Settings` (general: auto-fetch interval; appearance: theme, zoom, avatars; external tools: diff tool, merge tool, terminal command), `settings.json` in the config dir, atomic writes, missing/unknown fields → defaults.
- Migration: `auto_fetch_minutes` is read from the session file once and moved to settings.
- Corrupted file: defaults, status message, broken file kept as `settings.json.bak`.
- Settings view in the main area: categories General, Appearance, External tools; this task builds General (auto-fetch interval, 0 = off; timer restarts on change). Controls of later tasks are shown disabled with "Not available yet" until they land.
- Open with Ctrl+, , the gear button (toolbar right) and the palette; Esc or Close returns. UX-PARITY row for Ctrl+, (GitKraken: Preferences).
- ADR-023.

Acceptance: unit tests for defaults, roundtrip, unknown fields, migration, corrupted file; view checked by hand.

### [ ] M5-03 Theme system: dark and light

- `theme.rs`: constants become a `ThemePalette` (all current colors and lane colors) chosen at runtime; Dark keeps today's values, Light is new.
- Text, muted text and selected text reach WCAG AA (4.5:1) on their backgrounds in both palettes; lane colors stay distinguishable on the light background.
- Appearance → Theme: Dark / Light, plus System if iced 0.14 can detect the OS theme (check the iced source; record the result in ADR-027).
- Switching applies without restart, including canvas-drawn parts (graph, conflict view).

Acceptance: unit tests for the contrast ratios of both palettes; screenshots of start screen, graph, staging, diff, conflict view and interactive rebase in both themes.

### [ ] M5-04 Zoom and fullscreen

- Ctrl+= (also Ctrl++ and numpad +), Ctrl+-, Ctrl+0: 70 %–200 % in 10 % steps via the iced scale factor; persisted in settings, shown in Appearance and briefly in the status bar ("Zoom 120 %").
- F11 toggles fullscreen.
- Virtualization, hit testing, context menu positions and drag and drop stay correct at every zoom level.

Acceptance: unit tests for steps and bounds; by hand at 70 % and 200 %: graph scrolling, context menu position in a scrolled graph, drag and drop of a branch.

### [ ] M5-05 SVG icon set

- iced feature `svg`; Lucide SVGs in `crates/app/assets/icons/` with the license file; `icons.rs` embeds them (`include_bytes!`) and tints them with theme colors.
- Replace the Unicode placeholders: toolbar (undo, redo, pull, push, branch, stash, pop, terminal, settings), sidebar sections, file status, banners, remote marker on pills (cloud icon, remote name as tooltip), eye/solo icons for M5-08.
- Every icon-only button has a tooltip.
- ADR-025.

Acceptance: no Unicode placeholder icons left (list the replaced ones in the task note); screenshots in both themes.

### [ ] M5-06 Command palette

- Pure module `fuzzy.rs`: subsequence match with bonuses for word starts, consecutive characters and case; stable order for ties.
- Overlay at top center: query field, results with the shortcut right-aligned, disabled entries with their reason; ↑/↓, Enter, Esc, mouse. Empty query lists recently used actions first (in memory).
- Sub-lists: "Open repository" (Ctrl+Shift+O; recent repositories + Browse…), "Checkout branch" (local and remote; existing checkout flow incl. "Stash and checkout"), file list mode for M5-09. Backspace on an empty query returns to the action list.

Acceptance: unit tests for scorer ordering and palette state (filter, selection wrap, disabled entries not executable, sub-list navigation); filtering 100 000 file paths < 30 ms (release build).

### [ ] M5-07 Commit search

- Search field above the graph per planning decision; magnifier button and Ctrl+F open it, also from the palette.
- Pure match function over the loaded snapshot: case-insensitive, summary/message, author name and email, SHA prefix (≥ 4 hex characters). If the snapshot holds summaries only, search summaries and note it.
- Matches re-run after a refresh; jumping selects the commit and scrolls it into view; Esc closes and keeps the current selection.

Acceptance: unit tests for the match rules; app tests for next/previous with wrap-around and refresh; 100 000 commits < 50 ms (release build).

### [ ] M5-08 Hide and solo branches

- `git-graph`: pure `visible_commits(tips, parents)`; the layout runs on the visible subset; the WIP row stays.
- Sidebar: eye on hover toggles hide; hidden entries dimmed with an eye-off icon; context menu Hide/Show and Solo/Unsolo; soloed entries marked; while any branch is soloed only soloed branches and HEAD are visible. Remote header menu "Hide all" / "Show all". Graph pill menu "Hide".
- The checked-out branch cannot be hidden (disabled, tooltip "Checked-out branch") until the maintainer verification says otherwise.
- A selected commit that becomes hidden moves the selection to the WIP row.
- Stored per repository in the session file by full ref name; refs that no longer exist are dropped.

Acceptance: unit tests for reachability (shared history, merges, solo plus hide, nothing visible but HEAD) and persistence; toggling on 100 000 commits < 200 ms including layout.

### [ ] M5-09 File history

- `git_core::file_history(path, start)`: `git --no-optional-locks log --follow -z` with a parseable format (id, author, date, summary, path at that commit); cancellable (the process is killed when the view closes or a new request starts). ADR-024.
- Entry points: right click a file in commit details or staging → "File history"; "History" button in the diff view header; Ctrl+Shift+H → palette file list (tracked files at HEAD).
- The view replaces the graph: list left (initials/avatar, summary, author, date, rename marker), right the existing read-only diff of that file for the selected commit; "Show in graph" (and double click) closes the view and selects the commit; Esc closes. Loading indicator while running.

Acceptance: integration tests: linear history, rename followed, file deleted and re-added, merge commits, path with spaces and non-ASCII characters, cancellation; checked by hand.

### [ ] M5-10 Blame

- `git_core::blame(path, rev)`: `git --no-optional-locks blame --porcelain` (respects `.mailmap` and `blame.ignoreRevsFile`), parsed into line groups per commit.
- Blame mode in the file history view (History / Blame toggle) and "Blame at this commit" in the history list; for the working copy, uncommitted lines are marked "Not committed yet".
- Gutter per group: short SHA, author, relative date, alternating shading; tooltip with the full summary; clicking the gutter selects that commit in the history list.
- Virtualized rendering like the graph; binary files show a message instead.

Acceptance: parser unit tests (repeated commits, boundary commits, uncommitted lines, CRLF); integration test compared against `git blame`; a 10 000-line file scrolls smoothly (checked by hand).

### [ ] M5-11 Avatars (opt-in)

- Appearance → "Load avatars from Gravatar" (off), with the note "Sends a hash of each author's email address to gravatar.com".
- `https://gravatar.com/avatar/<sha256 of trimmed lowercase email>?s=<px>&d=404` via `ureq` (rustls, proxy from env); cache in `<cache dir>/gitloom/avatars/`, TTL 7 days, misses cached; at most 4 parallel requests; only visible rows and the details panel. ADR-026.
- Round image in the graph node and the commit details; initials as fallback and while loading.
- Setting off: no HTTP client is created and no request is made.

Acceptance: unit tests for the hash, cache TTL and misses, request limit, and "off means no request" (fetcher behind a trait, fake in tests); checked by hand against Gravatar.

### [ ] M5-12 External diff and merge tools

- External tools → Diff tool / Merge tool: "Use Git config" (default; shows the configured value or "not configured"), installed tools from `git difftool --tool-help` / `git mergetool --tool-help`, "None".
- Ctrl+D and the diff header button "Open in external diff tool": commit file → `git difftool --no-prompt [--tool=X] <parent> <commit> -- <path>`; unstaged → `git difftool --no-prompt -- <path>`; staged → with `--cached`.
- Conflict view and the conflicted file's context menu: "Open in merge tool" → `git mergetool --no-prompt [--tool=X] -- <path>`; refresh afterwards, the conflict view stays if the file is still conflicted.
- stdin closed, Gitloom's editor mode stays active, nothing waits for a terminal; status bar "Waiting for <tool>…".
- No tool configured → dialog with an explanation and "Open settings".

Acceptance: integration tests with a fake tool (`difftool.<name>.cmd` / `mergetool.<name>.cmd` script recording its arguments) for commit, unstaged, staged and merge; no hang with stdin closed.

### [ ] M5-13 Terminal and file manager

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
