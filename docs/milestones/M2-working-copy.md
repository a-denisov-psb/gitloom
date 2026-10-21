# M2 – Working copy: stage, commit, stash

Target version: **0.3.0**

## Goal

The daily loop works without a terminal: edit files, see changes appear automatically, stage files, hunks or lines, discard changes, commit or amend, stash and pop, and undo a commit or discard. All writes go through the `git` executable so hooks, signing and config behave exactly like plain Git.

## Scope

In scope:

- `CliBackend` for write operations, Git detection, one write operation at a time per repository
- UI infrastructure: modal dialogs, context menus (file rows, stashes), operation errors with Git output
- File watcher with incremental refresh
- Stage/unstage by file, hunk and line; discard by file, hunk, line and all
- Commit, amend, commit template, merge commit conclusion
- Stash, pop, apply, delete; stashes as graph rows
- Undo/redo for commit, amend and discard
- Filter-aware working tree diffs (eol/CRLF), partial clone fallback
- `LaneHints` preparation for the Git flow layout
- First CI run on Windows, macOS, Linux

Out of scope (later):

- Checkout, branches, tags, fetch/pull/push, commit/branch context menus, drag and drop → M3
- Merge/rebase/cherry-pick actions, conflict resolution UI, aborting merges → M4
- Tree view toggle for file lists, external diff tool, search → M5
- Git flow detection and layout → backlog (after M3)
- Commit signing UI (signing works through Git config, no UI)

## Scope changes against the roadmap

- Context menu and modal infrastructure moves from M3 to M2, because discard, stash and file actions need them. M3 reuses it for commits and branches.

## Milestone acceptance

- [ ] On this repository: modify files, stage a single line, commit, amend, undo the amend, stash with untracked files, pop — all from the UI, results identical to the CLI.
- [ ] External changes (editor, `cargo build`, `git` in a terminal) appear within 1 s without manual refresh; build output in ignored folders causes no refresh storm.
- [ ] CI green on Windows, macOS and Linux; a CRLF repository on Windows can be staged by hunk.
- [ ] Write actions are disabled with a clear tooltip when `git` is missing or too old.
- [ ] `./scripts/check.sh` green, `CHANGELOG.md`, `docs/UX-PARITY.md`, `docs/DECISIONS.md` up to date.
- [ ] Released as `v0.3.0` (M2-17).

## Dependencies to add

| Crate | Version | Where | Notes |
|---|---|---|---|
| `notify` | `8.2` | app | Recommended watcher, own debouncing |

Patch building is our own code; process spawning via `std::process`.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M2-01 | Platform baseline: CI and Windows check | GitHub remote |
| M2-02 | `CliBackend` foundation | – |
| M2-03 | UI infrastructure: modal, context menu, operation errors | – |
| M2-04 | Filter-aware working tree diffs | – |
| M2-05 | File watcher and incremental refresh | – |
| M2-06 | Stage and unstage files | M2-02 |
| M2-07 | Patch builder and hunk/line staging | M2-04, M2-06 |
| M2-08 | Discard changes | M2-03, M2-07 |
| M2-09 | Commit | M2-02, M2-03 |
| M2-10 | Amend | M2-09 |
| M2-11 | Stash | M2-02, M2-03 |
| M2-12 | Undo and redo | M2-08, M2-10 |
| M2-13 | Repository states (merge in progress, locks) | M2-09 |
| M2-14 | Partial clone fallback | M2-02 |
| M2-15 | `LaneHints` preparation | – |
| M2-16 | Performance and regression check | M2-05, M2-07 |
| M2-17 | Release v0.3.0 | all |

---

## Tasks

### [ ] M2-01 Platform baseline: CI and Windows check

> In progress: MSRV job added; dependencies pinned to stay compatible with Rust 1.88 (`ordered-float` 5.4, `uuid` 1.26, resolver 3). No extra Linux packages needed for `rfd` (portal via pure-Rust D-Bus, Wayland loaded at runtime). Waiting for CI read access and the Windows check (Maintainer to-dos).

Precondition: GitHub repository with `develop` pushed (done in the planning step). The maintainer pushes `master` and tags himself.

- Make CI green on all three OS (fix platform issues, add missing Linux packages for `rfd`).
- Add a CI job for MSRV 1.88 (`cargo check` only).
- Maintainer runs the app natively on Windows and checks the M1 shortcuts from `docs/UX-PARITY.md`; findings become fixes. J/K direction is confirmed (J down/older, K up/newer).
- Parts that wait for the maintainer go to "Maintainer to-dos" in `docs/HANDOVER.md`; continue with other tasks meanwhile.

Acceptance:
- CI green on `develop` for all three OS.
- Shortcut check documented in the task note (may be completed later by the maintainer).

### [ ] M2-02 `CliBackend` foundation

Goal: safe, uniform execution of `git` commands.

- New write trait (e.g. `GitWriter`) in `git-core`, implemented by `CliBackend`. Reads stay in `GixBackend`.
- Git discovery: `git` on `PATH`; parse `git --version`; minimum **2.34** (Ubuntu 22.04 LTS). Do not use newer flags without a fallback.
- Runner: arguments as a list (never via shell), working dir = repo root, stdin support, captures stdout/stderr/exit code.
- Environment: `GIT_TERMINAL_PROMPT=0`, `LC_ALL=C`, `GIT_OPTIONAL_LOCKS=0` for read-like helper calls; no interactive editor (`GIT_EDITOR` set to a no-op; commits always pass the message via `-F -`).
- Windows: spawn without a console window (`CREATE_NO_WINDOW`).
- Error type carries command, exit code and stderr for display.
- App: one write operation at a time per tab (queue or reject while busy); busy indicator in the status bar; reads keep working; refresh after each write.
- Git missing or too old: write actions disabled with tooltip naming the reason; one-time banner.

Acceptance:
- Unit tests for version parsing; integration tests run real commands on temp repos.
- No console window flashes on Windows (maintainer check, see Maintainer to-dos).

### [ ] M2-03 UI infrastructure: modal, context menu, operation errors

- Modal dialog over the whole window (`stack` + `opaque`): title, text, optional details (monospace, scrollable), buttons. Esc cancels, Enter confirms the default button. Destructive buttons in danger style.
- Context menu at the cursor position, opened with right click (`mouse_area::on_right_press`; buttons do not capture right clicks), closed by Esc, outside click or selection. Items: label, shortcut hint, enabled flag, separators.
- Operation error dialog: summary plus full Git output in the details area, "Copy" button.

Acceptance:
- Unit tests for menu state (open, close, item selection).
- Used by at least one real action by the end of the milestone (discard, stash).

### [ ] M2-04 Filter-aware working tree diffs

Goal: unstaged diffs match `git diff` in repositories with eol conversion, so patches built from them apply cleanly.

- Run worktree content through gix's `convert_to_git` filter pipeline (eol, `core.autocrlf`, `.gitattributes` `text`/`eol`, `ident`) before diffing against the index.
- External filters (LFS, custom `filter.*`): fall back to the CLI (`git diff -- <path>`) for that file, or show "Filtered file – diff not available"; document the choice in DECISIONS.

Acceptance:
- Tests with `core.autocrlf=true` and `.gitattributes eol=crlf`: diff equals `git diff` output (CI covers all OS).

### [ ] M2-05 File watcher and incremental refresh

- `notify` watcher per open repository (subscription keyed by tab id), recursive on the worktree.
- Classify events after a 300 ms debounce:
  - worktree path (not ignored) or `.git/index` → status refresh only
  - `.git/HEAD`, `.git/refs/**`, `.git/packed-refs`, `MERGE_HEAD`, `rebase-*` → refs check; full history reload only if any ref target changed
  - ignored paths (via gix excludes) and other `.git/` internals → dropped
- Own write operations suppress the watcher until their refresh completes.
- Window focus triggers only a cheap status refresh while the watcher is healthy.
- Watcher failure (e.g. inotify limit): fall back to focus refresh, one-time notice.

Acceptance:
- `cargo build` in this repo (writes to ignored `target/`) causes no refresh.
- Editing a tracked file updates the WIP row within 1 s.
- Unit tests for event classification.

### [ ] M2-06 Stage and unstage files

- Hovering a file row shows the "Stage File" / "Unstage File" button (remove the always-visible variant).
- "Stage all changes" / "Unstage all changes" buttons.
- Keyboard: S / U for the selected file, Ctrl+Shift+S / Ctrl+Shift+U for all.
- Commands: `git add -A -- <paths>` (handles deletions); unstage `git restore --staged -- <paths>`; before the first commit `git rm --cached -r -- <paths>`.
- Renames, conflicted files (stage = mark resolved), submodules work.
- After staging a file, the selection moves to the next unstaged file and its diff is shown in the center (GitKraken behavior). If no unstaged file is left, the diff closes and the graph is shown.

Acceptance:
- Integration tests for each file kind incl. repository without commits.
- Unit test for the selection rule.

### [ ] M2-07 Patch builder and hunk/line staging

- Pure patch builder in `git-core` from `FileDiff`: full hunk or a line selection.
  - Staging lines: unselected `+` lines are dropped, unselected `-` lines become context; hunk headers recomputed.
  - Unstaging: same on the staged diff, applied in reverse.
- Apply with `git apply --cached` (unstage: `--reverse`); patch via stdin.
- Diff view: per hunk "Stage hunk" / "Unstage hunk"; line selection with click, Shift+click range, Ctrl+click toggle; "Stage selected lines" / "Unstage selected lines".
- After applying: refresh, keep the diff open at the same position.

Acceptance:
- Property-style tests: for random hunks and selections, `git apply --check --cached` succeeds and the resulting index matches the expected content.
- Works on files without trailing newline, new files, deleted files.

### [ ] M2-08 Discard changes

- Discard file (hover trash button + context menu), discard hunk, discard selected lines, "Discard all changes".
- Confirmation dialog when discarding a file, a hunk, selected lines and all changes. GitKraken asks only for file and all; asking for hunks and lines is an intentional deviation (see UX-PARITY).
- Before discarding: snapshot every affected file's current worktree content with `git hash-object -w` and record path, blob id and mode for undo (M2-12).
- Tracked files: reverse-apply patch or `git restore --worktree -- <path>`; untracked files: delete.
- Context menu on unstaged file rows: Discard, Stage, Ignore (append to `.gitignore`), Open file location.

Acceptance:
- Integration tests including untracked and binary files.
- Undo data recorded for every discard (tested in M2-12).

### [ ] M2-09 Commit

- Summary with 72-character counter, description, button "Commit changes to N files"; disabled with hint when nothing is staged or the summary is empty.
- Ctrl+Enter commit, Ctrl+Shift+Enter stage all and commit, Ctrl+Shift+M focus summary.
- `git commit -F -` (message via stdin); hooks run normally; failures show the error dialog with hook output; message is kept.
- Prefill description from `commit.template` if configured.
- First commit in an empty repository; merge in progress → commit concludes the merge (message prefilled from `MERGE_MSG`).
- Signing works through Git config without UI; a failed signing shows the Git output.

Acceptance:
- Integration tests: normal, first commit, failing `pre-commit` hook, template, merge conclusion.

### [ ] M2-10 Amend

- "Amend previous commit" checkbox in the commit panel; enabling it fills summary/description from HEAD (if the fields are empty).
- Allows amending with no staged changes (message only).
- Hint when HEAD is already contained in its upstream ("already pushed").
- `git commit --amend -F -`.

Acceptance:
- Integration tests for message-only amend and amend with changes.

### [ ] M2-11 Stash

- Toolbar "Stash": stashes all changes. Includes untracked files (`git stash push -u`), like GitKraken. Default message as Git creates it.
- Toolbar "Pop": pops the latest stash; conflicts → error dialog, stash kept.
- Stashes as graph rows: virtual node connected (dashed) to its base commit, ordered by time; selecting shows stash details and files with read-only diffs.
- Context menu on stash (sidebar and graph): Apply, Pop, Delete (with confirmation). Editing the message is out of scope.
- Commands: `git stash push -u -m`, `git stash pop|apply|drop stash@{n}` — resolve the index by the stash commit id right before running to avoid races.

Acceptance:
- Integration tests for push (with untracked), pop, apply, drop, pop with conflict.
- Graph snapshot test with stash rows.

### [ ] M2-12 Undo and redo

- Per-tab in-memory stack of undoable actions; redo stack cleared on new actions.
- Commit / amend: record branch, HEAD before and after. Undo = `git reset --soft <before>` (root commit: `git update-ref -d HEAD`); redo = `git reset --soft <after>`. Only allowed while HEAD still equals the expected value.
- Discard: undo writes the snapshot blobs back (`git cat-file blob`), restoring files and modes; redo re-applies the discard.
- Preconditions failing → dialog "Cannot undo: the repository changed", entry dropped.
- Toolbar Undo/Redo enabled with tooltip naming the action ("Undo commit abc1234"); Ctrl+Z, Ctrl+Y, Ctrl+Shift+Z (not while a text input has focus).

Acceptance:
- Integration tests for undo/redo of commit, root commit, amend, discard of tracked and untracked files.

### [ ] M2-13 Repository states

- Detect merge, rebase, cherry-pick and revert in progress; banner in the tab like GitKraken ("Merging X into Y"); write actions that would break the state are disabled. Abort/continue UI comes in M4.
- `index.lock` present: operations fail with a clear message and a "Retry" option; never delete the lock automatically.

Acceptance:
- Integration tests for state detection.

### [ ] M2-14 Partial clone fallback

- Detect partial clones (`extensions.partialClone` / promisor remotes).
- When a blob is missing in gix, fetch it through `git cat-file blob <id>` via `CliBackend` (Git fetches it lazily); show "Fetching missing content…" while it runs; network errors show the error dialog.
- Without `git`: message "Content not available locally (partial clone)" instead of "object could not be found".

Acceptance:
- Integration test with a local blobless clone (`git clone --filter=blob:none file://…`).

### [ ] M2-15 `LaneHints` preparation

- `git-graph` takes an optional `LaneHints` input mapping commits to an ordered lane group (group index + color id). Groups reserve lane ranges in their order; inside a group the default allocation applies.
- Without hints the output is identical to today (snapshot tests unchanged).
- Must fit the vertical-run storage (ADR-011).
- No Git flow detection in M2.

Acceptance:
- Tests with hints for two and four groups; benchmark shows no regression without hints.

### [ ] M2-16 Performance and regression check

- Measure on the benchmark clone: status refresh after a single file change, watcher idle CPU, patch apply time on a large file.
- Targets: status refresh < 200 ms on the large clean repo, watcher idle CPU ≈ 0 %, no refresh caused by ignored paths.
- Update `docs/PERFORMANCE.md`.
- Optional (from M1-15): release or limit gix pack mappings and check allocator retention after repeated refreshes; only if cheap, otherwise propose a follow-up.

### [ ] M2-17 Release v0.3.0

Same procedure as M1-16 (`/release`), only when the maintainer asks. After the release Claude Code pushes `develop`; the maintainer pushes `master` and the tag.

## Verified GitKraken behavior

| Item | Result | Used in |
|---|---|---|
| Toolbar "Stash" includes untracked files | Yes | M2-11 |
| Discard asks for confirmation | GitKraken: file and all only; Gitloom: also hunk and lines (deviation) | M2-08 |
| Selection after staging a file | Next unstaged file, its diff is shown | M2-06 |
| J/K direction | J down (older), K up (newer) | M1-12 (confirmed) |
