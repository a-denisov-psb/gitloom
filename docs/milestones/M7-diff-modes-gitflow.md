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
| Left panel | A "GITFLOW" section, shown only when Git flow is initialized, below "STASHES" (above "SUBMODULES"); groups Features / Releases / Hotfixes with the names without prefix, counts, filter applies. A green "+" in the section header opens: Start feature, Start release, Start hotfix. Git flow branches stay in "LOCAL" as well. Click, double click and context menu behave like the same branch in "LOCAL". |
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
- [ ] Every diff (unstaged, staged, untracked, commit, stash, file history) can be shown as Hunk, Inline and Split; the mode is remembered across restarts.
- [ ] Line staging, unstaging and discarding work in all three modes and produce the same index as `git add -p` with the same selection; block actions in Inline/Split equal "Stage hunk" in Hunk mode where the blocks coincide.
- [ ] A 2 MB text file opens in Inline and Split without a frame over 50 ms while scrolling (`docs/PERFORMANCE.md`).
- [ ] Git flow can be initialized, edited and removed in Preferences; a repository initialized with the `git-flow` CLI is recognized and vice versa.
- [ ] Feature, release and hotfix can be started and finished from the left panel, the branch menus and the palette; the result equals `git flow <type> finish` (merge commits, tag, deleted branch) in a scripted comparison test.
- [ ] A finish that hits a conflict can be resolved and continues automatically, also after a restart; Abort leaves a consistent state and says what was done.
- [ ] Start and finish are undoable and redoable.
- [ ] User guide, UX-PARITY and CHANGELOG updated; `./scripts/check.sh` green; CI green on all platforms.
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

### [ ] M7-01 Diff mode setting and toggle

- `Settings` gains `diff.view_mode` (`hunk` | `inline` | `split`, default `hunk`); older `settings.json` files load unchanged (missing field = default), the settings version follows the existing migration rule.
- The Hunk / Inline / Split buttons in the diff header are enabled; the active one is highlighted; switching re-requests the diff with the matching context (M7-02) and keeps the selected file. Until M7-03/M7-04 land, Inline/Split may render with the Hunk renderer behind the toggle; the release must not ship that state.
- The command palette gets "Diff: Hunk view", "Diff: Inline view", "Diff: Split view".

Acceptance: the mode survives a restart; Hunk mode looks and behaves exactly as before (existing `iced_test` snapshots unchanged); unit test for loading old settings.

### [ ] M7-02 Full-context diffs in git-core

- `DiffContext { Hunks, Full }` passed from the diff request down to `text_diff` (e.g. `text_diff_with(old, new, context)`); `Full` yields one hunk spanning the whole file with correct `old_start`/`new_start`/counts and header, or no hunk for identical content.
- Same handling as today for binary, `too_large`, CRLF, missing trailing newline, added/deleted/renamed files, partial clone fetch and the filter pipeline (ADR-018).
- `build_patch` accepts selections inside a full-context hunk; the resulting patch applies with `git apply --cached` (and `--reverse` for unstaging) exactly like a 3-line-context patch.

Acceptance: unit tests for the cases above; an integration test stages a selection from a full-context diff and compares the index with the same selection applied via a 3-line-context diff; Hunk mode output unchanged (existing tests green).

### [ ] M7-03 Inline view

- Renderer for Inline: the full file with old/new line numbers, markers and the same colors as Hunk mode; virtualized rows (same approach as today) so large files scroll smoothly.
- Change blocks: computed in the app from the full-context hunk (changes whose 3-line contexts touch, same rule as `text_diff`); each block gets a slim bar with "Stage block" / "Unstage block" / "Discard block" (discard asks for confirmation like a hunk, see UX-PARITY deviations); the actions build a line selection of the block and use the existing line path.
- Line selection (click, Shift+click, Ctrl+click) and "Stage/Unstage/Discard selected lines" work as in Hunk mode.
- "Previous change" / "Next change" buttons in the header (disabled at the ends); opening a diff scrolls to the first change.
- After staging, the diff reloads and keeps the scroll position near the acted-on block.

Acceptance: stage/unstage/discard by block and by line in a test repo (incl. CRLF and a file without trailing newline) give the same results as in Hunk mode; `iced_test` snapshot of an Inline diff in both themes.

### [ ] M7-04 Split view

- Renderer for Split: two halves of equal width with a divider; left old line numbers and content (context, removed), right new (context, added); in a block removed and added lines are paired row by row, the shorter side padded with empty rows; one vertical scroll; long lines are clipped per half (no wrap, like Hunk mode).
- Line selection: removed lines are clicked on the left, added lines on the right; selection, block bars and change navigation as in M7-03.
- Added/deleted files show the empty side with "File added" / "File deleted"; binary/too large/no changes show the same messages as the other modes.

Acceptance: tests for the row pairing (only removals, only additions, more removals than additions and vice versa, adjacent blocks); stage/unstage/discard by block and line as in M7-03; `iced_test` snapshot in both themes at 100 % and 200 % zoom.

### [ ] M7-05 Diff modes in embedded views, snapshots, performance

- The diff embedded in file history (and any other embedded diff) uses the global mode and toggle; read-only diffs (commit, stash, history) show no staging bars.
- Measure opening and scrolling a 2 MB / ~50 000 line file in Inline and Split (`docs/PERFORMANCE.md`, M7 section); fix hot spots (e.g. precomputed row list, no per-frame allocation of the full file).

Acceptance: target from the milestone acceptance met and documented; snapshots for the file history diff in Split.

### [ ] M7-06 Git flow configuration model

- `git-core`: `FlowConfig { production, development, prefixes: feature/release/hotfix, version_tag }` with `read(repo) -> Option<FlowConfig>`, `write`, `remove` (only the `gitflow` section; other `gitflow.*` keys kept on write). Reads come from the repository config (gix), writes go through `git config` (CliBackend).
- `FlowBranch` classification of a local branch name (type + name without prefix) for the left panel and menus.
- `init(config)`: writes the config and creates missing branches (development from the production tip, production from HEAD); refuses in a repository without commits with a clear error.
- ADR-037 (native Git flow, config compatibility).

Acceptance: unit tests for parsing, classification and defaults; integration tests: a repo initialized with `git flow init -d` (if the CLI is available in CI, otherwise a config written like the CLI does) is recognized; a repo initialized by Gitloom passes `git flow config` / is usable by the CLI (same keys).

### [ ] M7-07 Gitflow preferences (initialize, edit, remove)

- Preferences → category "Gitflow", applying to the active tab's repository (title shows the repository name); on the start screen the category explains "Open a repository first".
- Not initialized: fields with defaults (M7 decisions) and "Initialize Gitflow". Initialized: fields with the current values, "Save", "Remove Gitflow" (confirmation; branches untouched).
- Validation: branch names and prefixes like Ctrl+B (prefix must end with `/` or be accepted as Git does); production ≠ development.
- The palette gets "Initialize Gitflow" (opens the category).

Acceptance: tests for validation; initialize, edit and remove in a test repo update `.git/config` as expected; the GITFLOW section (M7-08) appears/disappears without restart.

### [ ] M7-08 GITFLOW section and start

- Left panel section "GITFLOW" (decisions table): groups, counts, filter, collapse state remembered like other sections, green "+" menu.
- Start dialog per type (name field, validation, Enter starts, Esc cancels); creates the branch from the local development/production tip and checks it out (existing branch creation + checkout, undoable as today).
- Menu entries: development branch → Start feature, Start release; production branch → Start hotfix; palette → Start feature/release/hotfix.
- Blocked states (operation in progress, branch exists) show the reason like other write actions.

Acceptance: `iced_test` snapshot of the section; tests for start from all entry points; undo removes the started branch and returns to the previous branch.

### [ ] M7-09 Finish engine (git-core)

- `merge` gains a `no_ff` option (`--no-ff`); everything else reuses existing operations.
- `FlowFinish` plan: list of steps (rebase, checkout, merge, tag, delete) built from the type, options and config; executor runs steps in order, persists the plan with the index of the next step in `.git/gitloom/gitflow-finish.json`, stops on conflicts (`MergeOutcome::Conflicts`) and resumes from the saved step; removes the file when done or aborted.
- Hotfix while a release branch exists: follow the GitKraken verification answer; without an answer merge into development (GitKraken doc) and note it in UX-PARITY.
- Preconditions as in the decisions table, returned as typed errors.

Acceptance: integration tests for feature/release/hotfix finish (with and without delete, with rebase) compared with the result of `git flow … finish` where the CLI is available (otherwise against hand-written expected history); conflict in the first and in the second merge of a release finish, resume after resolution and after a simulated restart; abort.

### [ ] M7-10 Finish UI, conflicts and resume

- "Finish …" entries in the branch menus (left panel, graph pill) and the palette; finish dialog with the options from the decisions table; behind-upstream warning.
- Progress in the status bar; on conflicts the existing banner and conflict view, banner text "Finishing <type> <name>"; after Continue commits the merge, the remaining steps run without another click.
- On start of a tab with a pending `gitflow-finish.json` the banner offers "Continue finishing" / "Abort".
- Abort dialog lists the steps already done.

Acceptance: `iced_test` snapshots of the dialogs; a scripted walkthrough (Linux) of a release finish with a conflict in the develop merge, resolved in the conflict view.

### [ ] M7-11 Undo/redo for finish

- One undo entry per finish (decisions table, ADR-014 extension): records old/new values of the production, development and finished branch, the tag (ref and tag object id) and the checked-out branch; undo/redo move refs (`git update-ref` with old-value check), recreate or delete the branch and tag ref, and check out with `--keep` semantics.
- Precondition failures show "the repository changed" like other undo entries.

Acceptance: integration tests: finish → undo restores all refs and HEAD exactly; redo restores the finished state with the identical tag object; undo refused after an extra commit on develop.

### [ ] M7-12 Documentation and walkthrough

- User guide: new page "Git flow", diff modes in "Daily workflow"; "Coming from GitKraken" updated.
- `docs/UX-PARITY.md`: rows for diff modes and Gitflow set to done with the verification results; new deviations if any.
- CHANGELOG `[Unreleased]`; Linux walkthrough of all M7 acceptance items; open Windows checks appended to `docs/RELEASE-CHECKLIST.md` (M7 section) for the maintainer.

Acceptance: docs reviewed against the implemented behavior; checklist section present.

### [ ] M7-13 Release v1.1.0

`/release` (ADR-034).
