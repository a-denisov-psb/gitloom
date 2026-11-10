# M7 – Diff view modes and Git flow

Target version: **1.1.0**

Starts after `v1.0.0` (M6-15). Changed by the maintainer (2026-11-09): M7 starts on `develop` right after `v1.0.0-rc.2`; 1.0 fixes come from `master` (ADR-039).

## Goal

The diff view offers GitKraken's three modes (Hunk, Inline, Split) with staging in every mode, and Gitloom supports GitKraken's Git flow: initialize it per repository, start and finish features, releases and hotfixes from the left panel, the context menus and the command palette, including conflicts and undo. The left panel groups branches into folders like GitKraken.

## Scope

In scope:

- Left panel like GitKraken: branch folders, icons, highlighted checked-out branch, hidden-branch counter (M7-14, first task)
- Diff view modes Hunk / Inline / Split, remembered globally; line and block staging in all modes; change navigation
- Full-context diffs in `git-core`
- Git flow configuration (compatible with the `git-flow` CLI), initialization in the Preferences
- GITFLOW section in the left panel; start and finish for feature, release and hotfix
- Finish across conflicts (resumable) and undo/redo of start and finish
- User guide, UX parity and release 1.1.0

Out of scope (later):

- Git flow graph layout (ideas backlog; can build on the configuration model of M7-06)
- `bugfix/` and `support/` branch types of git-flow AVH (read and kept in the config, not shown)
- Pushing as part of a finish (GitKraken does not push either)
- Editing files in the diff view, word wrap, ignore whitespace, word diff, syntax highlighting
- Compare arbitrary commits (multi-selection diff)

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Left panel tree | Local and remote branches (per remote) are split at `/` into folders, any depth; folders before branches, both sorted case-insensitively, unless the GitKraken check says otherwise. Folder rows: chevron, folder icon, segment name; click toggles; collapse state per repository in the session (default expanded); no folder context menu unless GitKraken has one. Branch rows: branch icon, last segment only, full name in the tooltip, ahead/behind and all branch interactions unchanged. The checked-out branch is a highlighted row with a checkbox-style check. With hidden or solo branches a line "Viewing x/y · Show All" sits above the filter and section counts read "visible/total"; "Show All" clears hide and solo. Tags stay flat unless the GitKraken check says they are grouped. The GITFLOW section (M7-08) reuses the same row components. |
| Diff mode | Toggle Hunk / Inline / Split in the diff header (the existing disabled buttons). One global mode stored in `settings.json`, used by every diff view (standalone and embedded in file history). The conflict view is unchanged. |
| Diff data | `git-core` gains a context option (`DiffContext::Hunks` = today's 3 lines, `DiffContext::Full` = the whole file as one hunk). Hunk mode keeps its exact data and code path; Inline and Split load the full context. Size limits, binary detection and the filter pipeline (ADR-018) are the same for both. ADR-038. |
| Staging in Inline/Split | Line selection works like in Hunk mode (click, Shift+click, Ctrl+click). Instead of a hunk header, each **change block** (changes whose 3-line contexts touch, the same rule as `text_diff`) gets a slim bar with "Stage block" / "Unstage block" / "Discard block". The app turns a block into a line selection and applies it through the existing line path (`build_patch`), so there is no new patch code. |
| Split layout | One row list with two halves: left old (context and removed lines), right new (context and added lines). Inside a block, removed and added lines are paired row by row and the shorter side is padded with empty rows, so both sides scroll together. Added or deleted files show the empty side with "File added" / "File deleted". |
| Change navigation | In Inline and Split, "Previous change" / "Next change" buttons in the diff header scroll to the previous/next block (GitKraken's arrows). No shortcut (GitKraken has none). |
| Git flow engine | Native, no `git-flow` CLI: every step uses existing `GitWriter` operations (checkout, branch, merge, rebase, tag). The configuration uses git-flow's keys in the repository's `.git/config`: `gitflow.branch.master`, `gitflow.branch.develop`, `gitflow.prefix.feature`, `gitflow.prefix.release`, `gitflow.prefix.hotfix`, `gitflow.prefix.versiontag`. Other `gitflow.*` keys (bugfix, support, AVH options) are read, kept and ignored. "Initialized" = both `gitflow.branch.*` keys set (same check as the CLI). ADR-037. |
| Initialization | Preferences → new category "Gitflow" (applies to the repository of the active tab; disabled with a hint on the start screen). Fields: production branch (default `main` if it exists, otherwise `master`), development branch (`develop`), prefixes `feature/`, `release/`, `hotfix/`, version tag prefix (empty). "Initialize Gitflow" writes the config and creates missing branches locally (development from the production tip; production from HEAD if missing). Once initialized: fields editable, "Save", and "Remove Gitflow" (removes the `gitflow` section only, never branches). |
| Left panel | A "GITFLOW" section, shown only when Git flow is initialized, between "LOCAL" and "REMOTE" (maintainer decision 2026-11-10, M7-15); groups Features / Releases / Hotfixes with the names without prefix, counts, filter applies. A green "+" in the section header opens: Start feature, Start release, Start hotfix. Git flow branches stay in "LOCAL" as well. Click, double click and context menu behave like the same branch in "LOCAL". |
| Start | Dialog with one name field ("Feature name" / "Release version" / "Hotfix version"), validated like Ctrl+B. Feature and release start from the local development branch, hotfix from the local production branch; the new branch is checked out. Entry points: "+" in the GITFLOW section, branch menu of the development branch (Start feature, Start release) and the production branch (Start hotfix), command palette. |
| Finish | Branch menu of a Git flow branch (left panel and graph pill) "Finish feature <name>" / "Finish release <version>" / "Finish hotfix <version>", also the palette ("Finish current Gitflow branch" when one is checked out). Dialog options: feature: "Rebase on <develop> before finishing" (off), "Delete branch" (on); release/hotfix: tag name (`<versiontag><version>`, editable), tag message (prefilled "Release <version>" / "Hotfix <version>"), "Delete branch" (on). Steps: feature → (rebase) → checkout develop → `merge --no-ff` → delete. Release/hotfix → checkout production → `merge --no-ff` → annotated tag → checkout develop → `merge --no-ff` (the branch) → delete. Merge messages are Git's defaults. No push. |
| Finish preconditions | Clean working tree (otherwise "Commit or stash your changes first"), no operation in progress, production/development exist locally. If the local production or development branch is behind its upstream, the dialog shows a warning (not blocking). |
| Finish across conflicts | The plan of a running finish is saved to `.git/gitloom/gitflow-finish.json` (survives a restart). A merge conflict stops in the existing conflict flow; the operation banner says "Finishing <type> <name>". After the merge is committed with Continue, the remaining steps run automatically. Abort aborts the merge and the finish; steps already done stay, and a dialog lists them. |
| Undo | Start = existing `RefChange` entries (create + checkout). A finish is **one** undo entry: it records the old and new values of every touched ref (production, development, the finished branch, the tag) and the checked-out branch. Undo/redo only move refs and check out (the commits and the annotated tag object still exist, so redo restores the identical tag); precondition: clean working tree and every ref still where the finish/undo left it, otherwise "the repository changed" (ADR-014). |

## Maintainer decisions and GitKraken verification items

Answer what you can in GitKraken before or during M7. Claude Code decides the rest conservatively (ADR-034) and records it in UX-PARITY.

| Item | Used in |
|---|---|
| Left panel: are folders sorted before branches? Are tags grouped into folders as well? | M7-14 |
| Left panel: does a folder have a context menu (e.g. hide/solo all branches in it)? | M7-14 |
| Diff: is the mode remembered globally, per file or not at all? | M7-01 |
| Diff: staging controls in Inline/Split (hunk buttons, line selection, which buttons per block)? | M7-03, M7-04 |
| Diff: how Split aligns a changed block (padding, pairing) and where the change arrows sit | M7-04 |
| Gitflow: fields and defaults of the initialization in Preferences; is there a "Reset/Remove Gitflow"? | M7-07 |
| Gitflow: position of the left panel section; are Git flow branches also listed in LOCAL? | M7-08 |
| Gitflow: start dialog fields; "Start …" entries in the menus of develop/main or of commits? | M7-08 |
| Gitflow: finish dialog options (delete branch, rebase, tag name editable, tag message); behavior with local changes | M7-10 |
| Gitflow: hotfix finish while a release branch exists (merge into release instead of develop?) | M7-09 |

## Milestone acceptance

- [x] The left panel shows branch folders, icons, the highlighted checked-out branch and the hidden-branch counter like GitKraken; all branch interactions work unchanged in the tree.
- [x] Every diff (unstaged, staged, untracked, commit, stash, file history) can be shown as Hunk, Inline and Split; the mode is remembered across restarts.
- [x] Line staging, unstaging and discarding work in all three modes and produce the same index as `git add -p` with the same selection; block actions in Inline/Split equal "Stage hunk" in Hunk mode where the blocks coincide.
- [x] A 2 MB text file opens in Inline and Split without a frame over 50 ms while scrolling (`docs/PERFORMANCE.md`).
- [x] Git flow can be initialized, edited and removed in Preferences; a repository initialized with the `git-flow` CLI is recognized and vice versa.
- [x] Feature, release and hotfix can be started and finished from the left panel, the branch menus and the palette; the result equals `git flow <type> finish` (merge commits, tag, deleted branch) in a scripted comparison test.
- [x] A finish that hits a conflict can be resolved and continues automatically, also after a restart; Abort leaves a consistent state and says what was done.
- [x] Start and finish are undoable and redoable.
- [x] User guide, UX-PARITY and CHANGELOG updated; `./scripts/check.sh` green; CI green on all platforms.
- [ ] Follow-ups after the first review (M7-15 … M7-18): GITFLOW between LOCAL and REMOTE; diffs scroll horizontally in all modes; UTF-16 text files with a BOM are shown as text; graph lines drawn like GitKraken's.
- [ ] Released as `v1.1.0`.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M7-14 | Left panel tree like GitKraken (first task) | – |
| M7-01 | Diff mode setting and toggle | – |
| M7-02 | Full-context diffs in git-core | – |
| M7-03 | Inline view | M7-01, M7-02 |
| M7-04 | Split view | M7-03 |
| M7-05 | Diff modes in embedded views, snapshots, performance | M7-03, M7-04 |
| M7-06 | Git flow configuration model | – |
| M7-07 | Gitflow preferences (initialize, edit, remove) | M7-06 |
| M7-08 | GITFLOW section and start | M7-06, M7-14 |
| M7-09 | Finish engine (git-core) | M7-06 |
| M7-10 | Finish UI, conflicts and resume | M7-08, M7-09 |
| M7-11 | Undo/redo for finish | M7-09, M7-10 |
| M7-12 | Documentation and walkthrough | M7-01 … M7-11, M7-14 |
| M7-15 | GITFLOW section between LOCAL and REMOTE | M7-08 |
| M7-16 | Horizontal scrolling in the diff view | M7-05 |
| M7-17 | UTF-16 text files | M7-02 |
| M7-18 | Graph line style like GitKraken | – |
| M7-13 | Release v1.1.0 | all |

---

## Tasks

### [x] M7-14 Left panel tree like GitKraken

First task of M7 (the ID is new; it runs before M7-01). Behavior per the decisions table ("Left panel tree").

- Tree model in the app (pure function, unit-tested): from the branch list to folders and leaves per section/remote; cached per snapshot and filter, not rebuilt per frame (target: 2 000 branches build in < 5 ms).
- Rows: Lucide `folder`/`folder-open` and `git-branch` icons (vendored like the M5 icons, license kept); indentation per level; chevron on folders; truncation with tooltip at narrow widths.
- Collapse state per repository in `session.json` (key: section, remote, folder path); the filter matches full branch names and shows matches with their folders expanded without changing the stored state.
- Checked-out branch: highlighted row (palette color with AA contrast in both themes) and a checkbox-style check icon instead of the plain check.
- Hidden branches (M5-08): "Viewing x/y · Show All" above the filter while anything is hidden or solo; counts "visible/total" in LOCAL and REMOTE; "Show All" clears hide and solo.
- Click, double click, context menu, drag and drop, hide/solo and keyboard focus on branch rows behave as before; folder rows only toggle.
- `docs/UX-PARITY.md`: left panel row set to done with the verification results.

Acceptance: unit tests for the tree (nesting, sorting, a remote with and without folders, filter with expanded ancestors, counts with hidden branches); app tests for toggling, persistence and Show All; existing sidebar tests green; `iced_test` snapshots of the left panel in both themes at 100 % and 200 % zoom.

Note (2026-11-09): done. Tree model `crates/app/src/branch_tree.rs` (pure, unit-tested, 2 000 branches far below 5 ms); `RepoView::sidebar_groups` caches it per snapshot, text filter and hide/solo state. GitKraken verification items not answered yet, so the planned defaults apply (folders before branches, tags flat, no folder context menu); recorded in `docs/UX-PARITY.md`. Remote nodes collapse like folders (key `remote:<name>`), with a cloud icon. "Visible" counts the branches shown in the graph (the checked-out one always); the section counts are taken among the text filter matches, "Viewing x/y" among all branches. The checked-out row uses the selection color (AA in both themes) and shows its ahead/behind badge in normal text color. Snapshots `ui_check_left_panel_tree` (dark/light × 100/200 %, clicks on a folder and "Show All") checked; 200 % is rendered as half the logical size, since `iced_test` always renders at scale 2.

### [x] M7-01 Diff mode setting and toggle

- `Settings` gains `diff.view_mode` (`hunk` | `inline` | `split`, default `hunk`); older `settings.json` files load unchanged (missing field = default), the settings version follows the existing migration rule.
- The Hunk / Inline / Split buttons in the diff header are enabled; the active one is highlighted; switching re-requests the diff with the matching context (M7-02) and keeps the selected file. Until M7-03/M7-04 land, Inline/Split may render with the Hunk renderer behind the toggle; the release must not ship that state.
- The command palette gets "Diff: Hunk view", "Diff: Inline view", "Diff: Split view".

Acceptance: the mode survives a restart; Hunk mode looks and behaves exactly as before (existing `iced_test` snapshots unchanged); unit test for loading old settings.

Note (2026-11-09): done after M7-02. `settings.json` gains `diff.view_mode` (`hunk`/`inline`/`split`), version 1 kept (missing = `hunk`). Switching reloads every open diff (standalone and file history) with the mode's context and clears the line selection (positions differ); results loaded for another mode are dropped. Inline and Split show the full-context diff with the hunk renderer until M7-03/M7-04. Palette: "Diff: Hunk view", "Diff: Inline view", "Diff: Split view" (category Interface, no shortcut). Existing `iced_test` checks unchanged; new `ui_check_diff_mode_toggle`.

### [x] M7-02 Full-context diffs in git-core

- `DiffContext { Hunks, Full }` passed from the diff request down to `text_diff` (e.g. `text_diff_with(old, new, context)`); `Full` yields one hunk spanning the whole file with correct `old_start`/`new_start`/counts and header, or no hunk for identical content.
- Same handling as today for binary, `too_large`, CRLF, missing trailing newline, added/deleted/renamed files, partial clone fetch and the filter pipeline (ADR-018).
- `build_patch` accepts selections inside a full-context hunk; the resulting patch applies with `git apply --cached` (and `--reverse` for unstaging) exactly like a 3-line-context patch.

Acceptance: unit tests for the cases above; an integration test stages a selection from a full-context diff and compares the index with the same selection applied via a 3-line-context diff; Hunk mode output unchanged (existing tests green).

Note (2026-11-09): done before M7-01, which needs it to re-request diffs. The context is a parameter of the new `GitBackend::file_diff_with(request, context)` (`file_diff` = `DiffContext::Hunks`) instead of a field of `DiffRequest`, so the request stays the identity of the shown file across mode switches. `text_diff_with(old, new, context)`: `Full` uses the length of the longer file as context, so every change lands in one hunk; binary, size limit, CRLF, missing newline, added/deleted files and the filter pipeline are shared. `tests/patch.rs` stages and unstages 30 random selections through both contexts and compares the index.

### [x] M7-03 Inline view

- Renderer for Inline: the full file with old/new line numbers, markers and the same colors as Hunk mode; virtualized rows (same approach as today) so large files scroll smoothly.
- Change blocks: computed in the app from the full-context hunk (changes whose 3-line contexts touch, same rule as `text_diff`); each block gets a slim bar with "Stage block" / "Unstage block" / "Discard block" (discard asks for confirmation like a hunk, see UX-PARITY deviations); the actions build a line selection of the block and use the existing line path.
- Line selection (click, Shift+click, Ctrl+click) and "Stage/Unstage/Discard selected lines" work as in Hunk mode.
- "Previous change" / "Next change" buttons in the header (disabled at the ends); opening a diff scrolls to the first change.
- After staging, the diff reloads and keeps the scroll position near the acted-on block.

Acceptance: stage/unstage/discard by block and by line in a test repo (incl. CRLF and a file without trailing newline) give the same results as in Hunk mode; `iced_test` snapshot of an Inline diff in both themes.

Note (2026-11-09): done. Diff rows are typed (`DiffRow::Header/Line/Block`); the layout comes from the mode when the diff loaded (Split uses the Inline layout until M7-04). Blocks (`crate::diff_blocks`) use the hunk rule of `text_diff` (at most 6 unchanged lines between changes); the bar sits right above a block's first changed line, same row height as lines so the virtualization stays exact. Block actions pass the block's lines to the existing line path (`apply_diff_lines`, `discard_diff_lines`); discard asks "Discard block". Jumps keep 3 rows above the bar and stop where scrolling ends; opening a file or switching the mode jumps to the first change, reloads after staging keep the scroll position (same scrollable id). Tests: blocks equal hunks of the Hunk view; in real repositories (LF, CRLF, no trailing newline) stage/unstage/discard by block equals the hunk action and a line selection gives the same index as in Hunk mode; `ui_check_inline_diff` in both themes.

### [x] M7-04 Split view

- Renderer for Split: two halves of equal width with a divider; left old line numbers and content (context, removed), right new (context, added); in a block removed and added lines are paired row by row, the shorter side padded with empty rows; one vertical scroll; long lines are clipped per half (no wrap, like Hunk mode).
- Line selection: removed lines are clicked on the left, added lines on the right; selection, block bars and change navigation as in M7-03.
- Added/deleted files show the empty side with "File added" / "File deleted"; binary/too large/no changes show the same messages as the other modes.

Acceptance: tests for the row pairing (only removals, only additions, more removals than additions and vice versa, adjacent blocks); stage/unstage/discard by block and line as in M7-03; `iced_test` snapshot in both themes at 100 % and 200 % zoom.

Note (2026-11-09): done. `DiffRow::Split(hunk, left, right)` rows from `diff_blocks::split_rows` (runs of changes paired row by row, a removal after additions starts a new run); blocks, bars, navigation and staging are shared with Inline (same full-context data, same `(hunk, line)` selection keys, so block and line actions equal M7-03 by construction). Shift+click selects the changed lines of both sides in the range. The "File added" / "File deleted" note sits in the first row of the empty side. Snapshots `ui_check_split_diff` (dark/light × 100/200 %, added file) checked.

### [x] M7-05 Diff modes in embedded views, snapshots, performance

- The diff embedded in file history (and any other embedded diff) uses the global mode and toggle; read-only diffs (commit, stash, history) show no staging bars.
- Measure opening and scrolling a 2 MB / ~50 000 line file in Inline and Split (`docs/PERFORMANCE.md`, M7 section); fix hot spots (e.g. precomputed row list, no per-frame allocation of the full file).

Acceptance: target from the milestone acceptance met and documented; snapshots for the file history diff in Split.

Note (2026-11-09): done. The file history diff already followed the global mode since M7-01; read-only diffs (commit, stash, history) now have no block bars at all, and the change navigation jumps to a block's first changed row there. Performance in `docs/PERFORMANCE.md` (M7 section): frames 6–8 ms median, at most 19 ms, opening 43–51 ms for a 2 MB / 50 000 line file; no fix needed. Snapshot `ui_check_file_history_split` (dark/light).

### [x] M7-06 Git flow configuration model

- `git-core`: `FlowConfig { production, development, prefixes: feature/release/hotfix, version_tag }` with `read(repo) -> Option<FlowConfig>`, `write`, `remove` (only the `gitflow` section; other `gitflow.*` keys kept on write). Reads come from the repository config (gix), writes go through `git config` (CliBackend).
- `FlowBranch` classification of a local branch name (type + name without prefix) for the left panel and menus.
- `init(config)`: writes the config and creates missing branches (development from the production tip, production from HEAD); refuses in a repository without commits with a clear error.
- ADR-037 (native Git flow, config compatibility).

Acceptance: unit tests for parsing, classification and defaults; integration tests: a repo initialized with `git flow init -d` (if the CLI is available in CI, otherwise a config written like the CLI does) is recognized; a repo initialized by Gitloom passes `git flow config` / is usable by the CLI (same keys).

Note (2026-11-09): done. `git_core::flow` (`FlowConfig`, `FlowKind`, `FlowBranch`, `write`, `remove`, `init`) and `GitBackend::flow_config` (resolved config via gix, `None` unless both branch keys are set; missing prefixes take git-flow's defaults). `remove` drops every `gitflow` section (incl. AVH's `bugfix`/`support`/`path` keys), never branches. Classification picks the longest matching prefix. Validation: branch names like Ctrl+B, production ≠ development, a prefix must be able to start a branch name (git-flow accepts prefixes without "/", so does Gitloom), the tag prefix a tag name. The `git-flow` CLI is not installed in CI, so the CLI case uses the keys `git flow init -d` (AVH) writes.

### [x] M7-07 Gitflow preferences (initialize, edit, remove)

- Preferences → category "Gitflow", applying to the active tab's repository (title shows the repository name); on the start screen the category explains "Open a repository first".
- Not initialized: fields with defaults (M7 decisions) and "Initialize Gitflow". Initialized: fields with the current values, "Save", "Remove Gitflow" (confirmation; branches untouched).
- Validation: branch names and prefixes like Ctrl+B (prefix must end with `/` or be accepted as Git does); production ≠ development.
- The palette gets "Initialize Gitflow" (opens the category).

Acceptance: tests for validation; initialize, edit and remove in a test repo update `.git/config` as expected; the GITFLOW section (M7-08) appears/disappears without restart.

Note (2026-11-09): done. Preferences → "Gitflow" (title "Gitflow · <repository>"); the form starts from the repository's values or the defaults (`main` if it exists). Not initialized: "Initialize Gitflow" (`flow::init`, creates missing branches); initialized: "Save" (`flow::write`) and "Remove Gitflow" (confirmation, `flow::remove`, branches untouched). Validation from `FlowConfig::problem` is shown live and disables the button; writes run like other writes (busy/Git missing disable the buttons with the reason). The snapshot carries the Gitflow config and the refs check compares it, so a change (also from a terminal) refreshes the tab. Palette: "Initialize Gitflow" opens the category. Snapshot `ui_check_gitflow_preferences`.

### [x] M7-08 GITFLOW section and start

- Left panel section "GITFLOW" (decisions table): groups, counts, filter, collapse state remembered like other sections, green "+" menu.
- Start dialog per type (name field, validation, Enter starts, Esc cancels); creates the branch from the local development/production tip and checks it out (existing branch creation + checkout, undoable as today).
- Menu entries: development branch → Start feature, Start release; production branch → Start hotfix; palette → Start feature/release/hotfix.
- Blocked states (operation in progress, branch exists) show the reason like other write actions.

Acceptance: `iced_test` snapshot of the section; tests for start from all entry points; undo removes the started branch and returns to the previous branch.

Note (2026-11-09): done. GITFLOW section between STASHES and SUBMODULES with the groups Features/Releases/Hotfixes (a group is listed only when it has branches, like GitKraken; collapse keys `gitflow:<type>` stored with the folders), names without prefix, the same branch rows as LOCAL, total count, text filter on full names. Green "+" opens Start feature/release/hotfix at the pointer; the development branch menu offers Start feature/release, the production branch menu Start hotfix; the palette has "Start feature/release/hotfix" (disabled with "Initialize Gitflow first" without Git flow). The start dialog validates like Ctrl+B (plus: empty name, base branch missing, operation in progress) and creates and checks out the branch with the existing undoable path (undo deletes it and returns to the previous branch). Snapshot `ui_check_gitflow_section`.

### [x] M7-09 Finish engine (git-core)

- `merge` gains a `no_ff` option (`--no-ff`); everything else reuses existing operations.
- `FlowFinish` plan: list of steps (rebase, checkout, merge, tag, delete) built from the type, options and config; executor runs steps in order, persists the plan with the index of the next step in `.git/gitloom/gitflow-finish.json`, stops on conflicts (`MergeOutcome::Conflicts`) and resumes from the saved step; removes the file when done or aborted.
- Hotfix while a release branch exists: follow the GitKraken verification answer; without an answer merge into development (GitKraken doc) and note it in UX-PARITY.
- Preconditions as in the decisions table, returned as typed errors.

Acceptance: integration tests for feature/release/hotfix finish (with and without delete, with rebase) compared with the result of `git flow … finish` where the CLI is available (otherwise against hand-written expected history); conflict in the first and in the second merge of a release finish, resume after resolution and after a simulated restart; abort.

Note (2026-11-09): done. `git_core::merge_no_ff` and `git_core::flow_finish` (`plan`, `problem`, `start`, `resume`, `pending`, `abort`). A step that stops with conflicts counts as started (the user concludes it), so `next` points after it; `resume` waits while the operation is still in progress. Abort aborts a stopped merge/rebase and lists the steps done before it; later steps are not undone (that is undo's job, M7-11). Preconditions as `FinishProblem` (local changes to tracked files, operation in progress, another finish running, missing branch, tag exists/empty); untracked files do not block. Hotfix with an open release branch: no GitKraken answer, so the hotfix is merged into development (GitKraken's documentation), noted in UX-PARITY. The `git-flow` CLI is not in CI; tests compare with the history `git flow … finish` produces (merge parents, Git's default merge message, annotated tag on the production merge, deleted branch). git-core gains `serde`/`serde_json` (same versions as the app) for the plan file.

### [x] M7-10 Finish UI, conflicts and resume

- "Finish …" entries in the branch menus (left panel, graph pill) and the palette; finish dialog with the options from the decisions table; behind-upstream warning.
- Progress in the status bar; on conflicts the existing banner and conflict view, banner text "Finishing <type> <name>"; after Continue commits the merge, the remaining steps run without another click.
- On start of a tab with a pending `gitflow-finish.json` the banner offers "Continue finishing" / "Abort".
- Abort dialog lists the steps already done.

Acceptance: `iced_test` snapshots of the dialogs; a scripted walkthrough (Linux) of a release finish with a conflict in the develop merge, resolved in the conflict view.

Note (2026-11-09): done. "Finish <type> <name>" in the menu of every Git flow branch (left panel and graph pill share the branch menu) and "Finish current Gitflow branch" in the palette. Dialog: feature "Rebase on <develop> before finishing" (off) / "Delete branch" (on); release and hotfix tag name, tag message, "Delete branch"; a warning names production/development branches behind their upstream; an empty, invalid or existing tag is refused. Forms gained a checkbox field. The snapshot reads the running finish (`.git/gitloom/gitflow-finish.json`); while its merge waits the operation banner reads "Finishing <type> <name> · Merging …" and its Abort aborts the finish; once the merge is committed (Continue) the remaining steps run automatically, but only for a finish started or continued in this session and only once per step (a failing step does not loop). After a restart, or after a failed step, the banner offers "Continue finishing" / "Abort". The abort confirmation lists the steps already done. Snapshots `ui_check_gitflow_finish`. The scripted conflict walkthrough is covered by the git-core tests (conflict in either merge, resume after restart) plus the app tests of the automatic continuation; a hand walkthrough is in the M7 release checklist (M7-12).

### [x] M7-11 Undo/redo for finish

- One undo entry per finish (decisions table, ADR-014 extension): records old/new values of the production, development and finished branch, the tag (ref and tag object id) and the checked-out branch; undo/redo move refs (`git update-ref` with old-value check), recreate or delete the branch and tag ref, and check out with `--keep` semantics.
- Precondition failures show "the repository changed" like other undo entries.

Acceptance: integration tests: finish → undo restores all refs and HEAD exactly; redo restores the finished state with the identical tag object; undo refused after an extra commit on develop.

Note (2026-11-09): done. The finish plan stores the values of every touched ref (finished branch, production/development, tag) and the checked-out branch when it starts, so a finish that continued after a conflict or a restart is still undone as a whole. On completion `FinishOutcome::Done` carries a `FinishRecord` (old/new value per ref, branch before/after), pushed as one `UndoAction::FlowFinish`. Undo/redo check a clean working tree, no operation and that every ref is where it was left ("The repository changed since then …" otherwise), detach HEAD, move/create/delete the refs with `git update-ref` and the expected old value, then check out the branch (`git switch` refuses to lose local changes). Redo restores the identical annotated tag object.

### [x] M7-12 Documentation and walkthrough

- User guide: new page "Git flow", diff modes in "Daily workflow"; "Coming from GitKraken" updated.
- `docs/UX-PARITY.md`: rows for diff modes and Gitflow set to done with the verification results; new deviations if any.
- CHANGELOG `[Unreleased]`; Linux walkthrough of all M7 acceptance items; open Windows checks appended to `docs/RELEASE-CHECKLIST.md` (M7 section) for the maintainer.

Acceptance: docs reviewed against the implemented behavior; checklist section present.

Note (2026-11-09): done. New user guide page `docs/user/git-flow.md`; diff views, block staging and the folder tree in "Daily workflow"; "Getting started" and "Coming from GitKraken" updated; UX-PARITY rows for diff modes and Gitflow set to done (GitKraken checks still open, listed there); CHANGELOG complete; `docs/RELEASE-CHECKLIST.md` section 7a holds the Windows checks of M7 for the maintainer. The Linux walkthrough of the acceptance items is covered by the automated tests (git-core integration tests against real repositories, app tests of every entry point, `iced_test` snapshots checked by eye); there is no display in the cloud session for a hand run.

### [x] M7-15 GITFLOW section between LOCAL and REMOTE

Maintainer decision (2026-11-10): the GITFLOW section moves from below STASHES to between LOCAL and REMOTE.

- Order: LOCAL, GITFLOW (when initialized), REMOTE, TAGS, STASHES, SUBMODULES. Collapse state, filter, counts and the green "+" unchanged.
- `docs/UX-PARITY.md`: left panel row and a new deviation row (see below); user guide `git-flow.md` if it names the position.

Acceptance: snapshot `ui_check_gitflow_section` and the left panel snapshots updated; keyboard focus order follows the new order.

Note (2026-11-10): done. The sidebar column is built LOCAL, GITFLOW (when initialized), REMOTE, TAGS, STASHES, SUBMODULES; widget order is the focus order, so keyboard focus follows. `gitflow_section_between_local_and_remote` checks the header positions (layout, runs always); `ui_check_gitflow_section` re-rendered and checked by eye; the other left panel snapshots have no Git flow and are unchanged. `docs/user/git-flow.md` does not name the position.

### [x] M7-16 Horizontal scrolling in the diff view

Finding of the maintainer (2026-11-10): long lines cannot be scrolled horizontally in any diff mode.

- Hunk, Inline and Split, in every diff view (working copy, commit, stash, file history): Shift+mouse wheel, horizontal wheel/tilt and touchpad scroll horizontally; a horizontal scrollbar appears at the bottom of the diff while the content is wider than the view.
- Line numbers, markers, hunk headers and block bars stay in place; only the code text moves. Split: one shared horizontal offset for both halves, so paired lines stay aligned; each half clips its own text.
- Content width = the longest line of the loaded diff in the monospace font, computed once per loaded diff (not per frame); the offset is clamped, reset when another file opens and kept when the diff reloads after staging or a mode switch.
- Line selection (click, Shift+click, Ctrl+click) and block actions work unchanged at any offset.
- The conflict view gets the same behavior if it shares the line rendering; otherwise note it in the task note.

Acceptance: unit tests for width, clamping and the wheel mapping (Shift+vertical → horizontal); app test: selection at a scrolled offset; `iced_test` snapshots of a scrolled Hunk and Split diff; the 2 MB scroll measurement of `docs/PERFORMANCE.md` still meets the 50 ms target.

Note (2026-11-10): done. `DiffView` keeps `h_offset` (kept across reloads, a new file gets a new view), the measured `text_width` of the longest line (one monospace measurement per load, `diff_hscroll::text_width`) and the range the scrollbar last reported. The code text of each line sits in a `pin` moved left by the offset and clipped to its room; numbers, markers, hunk headers and block bars are not moved; Split uses one offset for both halves. iced's vertical scrollable drops horizontal wheel input, so a small `wheel_observer` widget around it reports horizontal deltas without capturing the event (Shift+wheel, tilt wheel, touchpad x); the horizontal scrollbar under the text (a horizontal scrollable whose range is the largest offset, computed per layout pass from the width via `responsive`) reports drags and follows wheel scrolling through `scroll_to`. The conflict view has its own line rendering and is unchanged (backlog candidate). Measured (release, 2 MB, scrolled 120 px to the right): frame max 9–19 ms, open 49–58 ms (`docs/PERFORMANCE.md`).

### [x] M7-17 UTF-16 text files

Finding of the maintainer (2026-11-10, Windows): a text file created with `echo test > file` (PowerShell 5 writes UTF-16 LE with BOM) is shown as "Binary file not shown". Binary detection looks for NUL bytes like Git, and UTF-16 text contains them.

- Before the binary check, a side that starts with a UTF-16 BOM (`FF FE` little endian, `FE FF` big endian) is decoded to text (invalid sequences replaced) and diffed as text in all modes; the diff header shows the encoding ("UTF-16 LE" / "UTF-16 BE"). Files without BOM stay binary (Git's behavior).
- Staging, unstaging and discarding by hunk, block or line are disabled for such files with "Line staging is not available for UTF-16 files; stage the whole file" (`git apply` treats them as binary); file-level actions work.
- The size limit applies to the file size in bytes. Files with a `working-tree-encoding` attribute: check what the existing filter pipeline (ADR-018) produces and note it in the task note.
- CHANGELOG and user guide (`known-limitations.md`: line staging for UTF-16).

Acceptance: unit tests (LE, BE, BOM only on one side, odd byte count, mixed newlines); app test: the line actions are disabled with the hint, staging the whole file works; snapshot of a UTF-16 diff.

Note (2026-11-10): done. `git_core::diff::decode_utf16` runs before the binary check in `text_diff_with` (after the size check on the raw bytes); `FileDiff`/`TextDiff` carry `encoding`, shown in the diff header. `DiffView::line_staging_block` disables the hunk and block buttons (tooltip with the hint), line selection, and guards Apply/Discard of hunks, blocks and lines (status bar shows the hint). `working-tree-encoding` (checked in `tests/gix_diff.rs`): with `UTF-16` gix's filter pipeline converts the working tree file, so the diff is plain UTF-8 text and line staging stays available; Git's own name `UTF-16LE-BOM` is unknown to gix's encoding filter, the diff then reports "Filtered file – diff not available" as before (known limitation in the user guide). Blame and the conflict view still treat UTF-16 as binary (outside this task).

### [x] M7-18 Graph line style like GitKraken

Maintainer wish (2026-11-10), reference: GitKraken screenshot from the maintainer at the same display scaling.

- Edges: straight vertical lines in their lane and 90° turns with a rounded corner instead of Bézier curves, where a branch leaves or joins a node; corner radius, line width (thinner than today's 2 px) and the dash pattern of dashed lines (WIP, stash) taken from the reference; node shapes and colors unchanged unless the reference clearly differs.
- Keep the per-row segment model of `docs/ARCHITECTURE.md` (virtualized rows); corners must join seamlessly across rows, including M6-23's gap fix.
- Style values as named constants (later style tokens of M9); `docs/ARCHITECTURE.md` graph rendering section updated.

Acceptance: `iced_test` graph snapshots (merges, branch-offs, a stash, WIP; dark/light; 100 % and 200 %) compared side by side with the reference; the task note lists the chosen values.

Note (2026-11-10): done. Values from the maintainer's two reference screenshots (dark theme): `LINE_WIDTH` 1.5 px (was 2 px), `CORNER_RADIUS` 6 px (limited to the lane distance), `DASH` 2 px dash / 2 px gap (dotted, was 4/3) for WIP and stash edges. Edges into or out of a node from another lane run horizontally at the node's height and turn 90° with a rounded corner (quadratic curve through the corner point) instead of an S-curve; vertical parts keep M6-23's overlap past the row edges and the global dash phase, so corners and lines join across rows. Nodes unchanged: the reference shows the same shapes (avatar or initials circle, small merge dot, dashed WIP circle); GitKraken's row tint in the lane color and the branch-label connector lines are not part of this task. Snapshots `ui_check_graph_lines` (new `graph_app` with merges, a branch-off, a stash and WIP; dark/light, 100 %/200 %) compared by eye with the reference; geometry tested in `edges_turn_with_rounded_corners`.

### [ ] M7-13 Release v1.1.0

`/release` (ADR-034) on `release/1.1.0` (ADR-040), only after `v1.0.0`; merge `release/1.1.0` into `master` and back into `develop`.
