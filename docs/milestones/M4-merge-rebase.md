# M4 – Merge, rebase, conflicts

Target version: **0.5.0**

## Goal

History operations work from the UI like in GitKraken: merge, rebase, interactive rebase, cherry-pick, revert and reset, with a conflict resolution view, drag and drop of branches, multi-selection in the graph and undo/redo for all of them.

## Scope

In scope:

- Multi-selection in the graph
- Sequencer states: continue, skip, abort for merge, rebase, cherry-pick, revert
- Gitloom as Git editor (sequence editor and message editor) without interactive terminals
- Merge, fast-forward, rebase, interactive rebase (pick, reword, squash, fixup, drop, reorder)
- Reword older commits, squash selected commits
- Cherry-pick (one or many), revert
- Reset soft, mixed, hard
- Conflict resolution view (ours, theirs, output)
- Drag and drop of branches (moved from M3)
- Undo/redo for all operations above

Out of scope (later):

- External diff/merge tools, file history, blame → M5
- `--rebase-merges`, `edit` (stop for amending) in interactive rebase, autosquash UI, submodule conflicts → backlog
- Pull requests and hosting integrations → non-goal

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Conflict view | GitKraken layout: "ours" top left, "theirs" top right, each with checkboxes per hunk and line; editable output below. Buttons: take all ours, take all theirs, save and mark resolved. |
| Interactive rebase | Panel replacing the graph: list of commits with action per commit (pick, reword, squash, fixup, drop), keyboard shortcuts P/R/S/F/D, drag to reorder, inline message editing for reword/squash. Start and Cancel buttons. |
| Git editor | Gitloom acts as `GIT_SEQUENCE_EDITOR` and `GIT_EDITOR` (env flag like askpass, ADR-019 addendum): it writes a prepared todo list or message file. No interactive terminal editors are ever started. |
| Reset hard | Always confirmed. Undoable: before the reset, the index tree (`git write-tree`) and every changed worktree file (blob snapshot, `--no-filters`) are recorded; undo restores HEAD, index and files. |
| Drag and drop | Drop source branch S onto target T → menu: Fast-forward T to S (if possible), Merge S into T, Rebase S onto T, Interactive rebase S onto T. Items that need a different checked-out branch are disabled with tooltip "Check out <branch> first". |
| Dirty working tree before merge/rebase | Respect `rebase.autoStash` / `merge.autoStash`; otherwise dialog "Stash and continue" / "Cancel" (same pattern as checkout). |

## Maintainer verification items

| Item | Used in |
|---|---|
| Drag and drop onto a branch that is not checked out: disabled, or does GitKraken check out automatically? | M4-11 |
| Interactive rebase list order (newest on top or oldest on top) | M4-07 |

Until answered, implement the decisions above; adjust afterwards.

## Milestone acceptance

- [ ] On a throwaway branch of this repo: merge with conflict resolved in the conflict view, rebase with a conflict, interactive rebase with reorder/squash/reword/drop, cherry-pick of three commits, revert, reset soft/mixed/hard — each undone and redone once. Results identical to the CLI.
- [ ] No Git command ever waits for a terminal editor or prompt.
- [ ] CRLF repository: conflict resolution keeps line endings (Windows CI).
- [ ] `./scripts/check.sh` green on all CI platforms, docs and CHANGELOG up to date.
- [ ] Released as `v0.5.0` (M4-15).

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M4-01 | Multi-selection in the graph | – |
| M4-02 | Gitloom as Git editor | – |
| M4-03 | Sequencer states: continue, skip, abort | M4-02 |
| M4-04 | Merge and fast-forward | M4-03 |
| M4-05 | Conflict resolution view | M4-03 |
| M4-06 | Rebase | M4-03, M4-05 |
| M4-07 | Interactive rebase | M4-02, M4-06 |
| M4-08 | Reword and squash shortcuts | M4-07, M4-01 |
| M4-09 | Cherry-pick and revert | M4-01, M4-05 |
| M4-10 | Reset | – |
| M4-11 | Drag and drop of branches | M4-04, M4-06, M4-07 |
| M4-12 | Undo/redo for history operations | M4-04 … M4-10 |
| M4-13 | Edge cases and robustness | M4-04 … M4-09 |
| M4-14 | End-to-end check, docs, performance | all others |
| M4-15 | Release v0.5.0 | all |

---

## Tasks

### [x] M4-01 Multi-selection in the graph

> Done: pure model `crates/app/src/selection.rs` (toggle, range from the anchor, retain after refresh, graph order); `RepoView::multi` next to the primary selection; Ctrl+click (command key on macOS) toggles, Shift+click and Shift+↑/↓ select a range from the anchor, plain clicks and arrows reset; stashes and the WIP row are not selectable. Shift+↑/↓ no longer navigate parent/child (Shift+J/K do; UX-PARITY deviation entry, CHANGELOG "Changed"). Panel with more than one commit: "N commits selected", summaries, "Cherry-pick N commits" and — only for a contiguous first-parent run on HEAD without merges (`contiguous_on_head`) — "Squash N commits", both disabled with "Not available yet" until M4-09/M4-08. Tests: model (toggle, range both directions, no anchor, refresh), app (clicks with modifiers, Shift+arrows, refresh, squash rule). Checked by hand: click + Ctrl+click + Shift+click → 3 commits and panel; Shift+↓ twice → 3 commits; Squash hidden for a selection with a merge and for commits off the first-parent line.

- Ctrl+click toggles, Shift+click selects a range, Shift+↑/↓ extends (Shift+J/K stay parent/child navigation; document the difference in UX-PARITY).
- Right panel with several commits: "N commits selected", list of their summaries, actions (cherry-pick, squash if contiguous on the current branch).
- Selection by commit ids, survives refresh.

Acceptance: unit tests for the selection model (toggle, range, refresh).

### [x] M4-02 Gitloom as Git editor

> Done: `git_core::editor` (`EditorInput { sequence, messages }`, `editor_call`, `editor_main`) and `GitWriter::run_editing`; `CliBackend::with_editor(exe)` set by the app next to askpass; `main.rs` handles `--git-editor=<kind> <file>` before askpass and iced. Deviation (extension): a queue of messages instead of one prepared file, consumed in order, so an interactive rebase with several rewords/squashes (M4-07) runs in one command. Tests: unit tests (parsing, quoting, todo copy, message order, missing preparation, cleanup) and `crates/app/tests/git_editor.rs` against the real binary (reorder, reword + squash, commit message, missing todo aborts with no rebase left, missing message stops the rebase without hanging, no editor configured). No user-visible change, so no CHANGELOG entry.

- Env flag (e.g. `GITLOOM_EDITOR=1`) plus a path to a prepared file: Gitloom started as editor copies the prepared content into the file Git passes and exits 0; missing preparation → exit 1 (Git aborts cleanly).
- Used as `GIT_SEQUENCE_EDITOR` (todo list) and `GIT_EDITOR` (commit messages for reword, squash, merge, revert).
- Never starts a UI in editor mode.
- Record as addendum to the askpass ADR.

Acceptance: integration tests with a prepared todo list and a prepared message; missing preparation aborts without hanging.

### [x] M4-03 Sequencer states: continue, skip, abort

> Done: `git_core::Operation` (`RepoState::kind`), `GitWriter::continue_operation(op, message)` / `skip_operation` / `abort_operation` (replaces `abort_merge`/`abort_rebase`). Continue answers Git's editor with the panel message (M4-02); cherry-pick and revert `--continue` commit `MERGE_MSG` without asking, so it is replaced first. `merge_message` now also returns the message of a stopped rebase step (prefill per step). Banner: Continue (disabled with a tooltip listing up to five conflicted files, or the missing summary), Skip (rebase, cherry-pick), Abort; the commit button reads "Continue <operation>" and does the same; "Conflicted Files" section above "Unstaged Files"; the detached HEAD hint is hidden during a rebase. Tests: `crates/app/tests/sequencer.rs` (continue with message, continue with unresolved conflicts, skip, skip refused, abort — for each operation) and app tests (blocking rules, labels, skip/abort per operation, prefill per step). Checked by hand: rebase conflict → resolve → edited summary → banner Continue → rebased with the edited message; cherry-pick conflict → Skip; merge conflict → Continue disabled with tooltip, no Skip, Abort.

- Banner buttons become active: Continue (`--continue`), Skip (rebase/cherry-pick), Abort (`--abort`) for merge, rebase, cherry-pick, revert.
- Continue requires all conflicts resolved; otherwise disabled with tooltip listing the files.
- Message for continue (merge/revert/squash) comes from the commit panel via the editor mode.
- Staging panel shows a "Conflicted Files" section above unstaged files.

Acceptance: integration tests for each state with continue, skip and abort.

### [x] M4-04 Merge and fast-forward

> Done: `git_core::{merge, fast_forward_head, fast_forward_branch, merge_blocked, MergeOutcome}`: `git merge [--autostash] <branch>` (user's ff/autostash config, Git's default message from the short branch name; a conflict returns `Conflicts`, not an error, and the banner takes over), `git merge --ff-only` for the checked-out branch, `git fetch --no-write-fetch-head . <ref>:refs/heads/<branch>` for other branches (Git refuses non-fast-forwards and the checked-out branch). Menus: local branch (not checked out) "Merge X into Y" and "Fast-forward Y to X" if HEAD is a proper ancestor (`RepoSnapshot::is_ancestor` on the loaded history; unknown → offered, Git decides); remote branch the same plus "Fast-forward L to origin/L" for a non-checked-out local branch tracking it. Merging is blocked while an operation is in progress (status message). Dirty tree: "Local changes block the merge" dialog → "Stash and merge" retries with `--autostash` (Git re-applies the stash; on conflict it stays in the stash list). Tests: `crates/git-core/tests/merge.rs` (ff, merge commit incl. remote message, conflict, blocked, autostash, `--ff-only` refusal, fast-forward of a non-checked-out branch, refusal for diverged and checked-out branches), snapshot ancestry, menus, app (blocked dialog → autostash retry, operation guard). Checked by hand: "Fast-forward main to topic" from the sidebar menu; merge with a dirty file → dialog → "Stash and merge" → merge commit, stash re-applied with a conflict and kept in the list (Git's behavior).

- Branch context menu "Merge <branch> into <current>" (`git merge`, respecting user config for ff).
- "Fast-forward <branch> to <commit>" for branches that are not checked out (`git fetch . <commit>:<branch>` or `update-ref` with ancestry check) and for the current branch (`git merge --ff-only`).
- Conflicts → state banner + conflict view.

Acceptance: integration tests for ff, merge commit, conflict, fast-forward of a non-checked-out branch.

### [x] M4-05 Conflict resolution view

> Done: pure model `git_core::conflict` (`ConflictText` parsed from `git merge-file -p --diff3 --marker-size=31`, `Picks` per hunk/line/side, `output` with Git markers for open conflicts, `with_line_endings`), I/O in `git_core::resolve` (`load_conflict` from stages via `ls-files -u`, `resolve_conflict` = blob + `update-index --cacheinfo` + `checkout-index`, `resolve_with_side`), `GitWriter::run_status` for commands that report through the exit code. App: `conflict::ConflictView` (picks, output editor, edited-wins with "Reset output", save blocker) and `ui::conflict` (A · ours top left, B · theirs top right with checkboxes per conflict and line, clean lines dimmed, output below with "N conflicts left"; Take all ours/theirs, Save and mark resolved, Close); clicking a file under "Conflicted Files" opens it; the view closes once the file is no longer conflicted. Labels: checked-out branch vs. merged/picked commit name. Deviation: with both sides picked, ours lines come first (UX-PARITY). Large files (> 2 MB) follow in M4-13. Tests: model (parsing, committed markers, overlapping hunks, line picks from both sides, take all, CRLF, missing trailing newline), view state (picks vs. edits, reset, CRLF for edits, failed load), git-core integration (text, committed markers, CRLF, autocrlf, add/add, delete/modify, binary, unknown path), app (open → load → save blocked → take theirs → save → closes; stale load), resolve → continue for merge, rebase and cherry-pick. Checked by hand: merge with two conflicts → picked one line per side in conflict 1 and theirs in conflict 2 → output correct; typed into the output → indicator, picks ignored → Reset output; Save and mark resolved → file staged, Continue merge → merge commit.

- Clicking a conflicted file opens the view in the center (replaces graph like the diff view).
- Data from index stages 1/2/3; default output from `git merge-file -p` (with markers); a pure model in git-core maps hunks/lines of ours and theirs to the output.
- Checkboxes per hunk and per line in ours/theirs update the output; the output is editable (text editor) and wins over checkboxes once edited (indicator + "Reset output").
- Buttons: take all ours, take all theirs, save and mark resolved (`git add`). Binary, delete/modify and add/add conflicts: choose ours or theirs file.
- Line endings preserved (CRLF repos).

Acceptance: unit tests for the conflict model (overlapping hunks, line picks from both sides, CRLF, no trailing newline); integration test resolve → continue.

### [x] M4-06 Rebase

> Done: `git_core::{rebase, rebase_blocked}` (`git rebase [--autostash] <ref>`, user's `rebase.autoStash` respected; a stop with conflicts — `REBASE_HEAD` — returns `MergeOutcome::Conflicts`, renamed `Merged` → `Done`). Menus: "Rebase Y onto X" next to merge for local (not checked out) and remote branches, replacing the disabled entry. Dirty tree: Git refuses ("cannot rebase: You have unstaged changes" / "Your index contains uncommitted changes") → "Local changes block the rebase" → "Stash and rebase" with `--autostash`. Tests: git-core (clean rebase, conflict + abort, blocked by unstaged and staged changes, autostash), `crates/app/tests/sequencer.rs` (rebase with a conflict resolved, continue with message), menus, app (blocked dialog). Checked by hand: sidebar menu on `main` while `topic` is checked out → "Rebase topic onto main" → linear history.

- Branch context menu "Rebase <current> onto <branch>" (`git rebase <branch>`).
- Stops on conflicts → conflict view → continue/skip/abort (M4-03).
- Dirty tree handling per planning decision.

Acceptance: integration tests: clean rebase, rebase with conflict resolved, abort.

### [x] M4-07 Interactive rebase

> Done: pure plan `git_core::todo` (`TodoAction`, `validate_todo`, `chains`, `needs_message`, `default_message`, `build_todo`: one message per reword and one combined per chain with a squash, in Git's editor order; a reword heading a squash chain becomes pick), `rebase_commits` (`base..HEAD`, no merges, topological, oldest first) and `rebase_interactive` (`rebase -i` via `run_editing`). App: `rebase_panel::RebasePanel` (newest on top, selection, drag, Alt+↑/↓, message editors keyed by chain head) and `ui::rebase` (action buttons per row, "↓ into the commit below" for squash/fixup, validation hint, Cancel / Start rebase); commit menu "Interactive rebase from here" only for proper ancestors of HEAD without an operation in progress; P/R/S/F/D, ↑/↓, Alt+↑/↓ in the panel; local changes → "Stash and rebase". List order: newest on top, pending the maintainer's GitKraken check. Known limit (→ M4-13): after a conflict stop the remaining prepared messages are gone; Git then stops at those reword/squash steps and Continue uses the prefilled original message. Drag and drop entry follows in M4-11. Tests: plan model, panel state (selection, reorder, drag, messages), integration with the real binary (reorder, reword, squash with combined message, fixup, drop, mixed plan message order, conflict during reorder), app (menu rule, keys → plan → start closes the panel). Checked by hand: menu → panel with 4 commits; Squash on the top commit → combined message editor under the commit below; Drop; drag of the oldest commit to the top; Start rebase → history and messages as planned. Keys could not be checked by hand (WSLg; maintainer to-do).

- Entry: commit context menu "Interactive rebase from here" (commits after the selected one up to HEAD), drag and drop menu (M4-11).
- Panel per planning decision; validation (first commit cannot be squash/fixup; at least one commit kept).
- Execution: prepared todo via M4-02, reword/squash messages prepared upfront; stops for conflicts handled by M4-03/M4-05.

Acceptance: integration tests for reorder, reword, squash (combined message), fixup, drop, conflict during reorder.

### [x] M4-08 Reword and squash shortcuts

> Done: `git_core::{reword_commit, squash_commits}` on top of M4-07: HEAD is amended with `commit --amend --only` (staged changes stay out), older commits are reworded by `rebase -i <id>^` with a single reword; squash finds the oldest selected commit (the one whose parent is not selected), checks that the selection is the start of `base..HEAD` and squashes it into one commit, later commits stay picks. Both refuse merges between the commit and HEAD (would be flattened). App: commit menu "Edit commit message" (enabled by `repo_view::rewordable`: HEAD, or linear first-parent history with a parent), multi-selection "Squash N commits" enabled (`repo_view::squashable`), a commit message dialog (`overlay::MessageDialog`, prefilled with the message, or all messages oldest first for squash), "Stash and rebase" for blocking local changes. Tests: integration with the real binary (reword HEAD keeps the index out, reword older, squash keeps later commits, non-contiguous and single selections rejected, commits off the first-parent line or with a merge after them rejected), app (rules, dialog → reword, squash flow). Checked by hand: menu on a root commit → "Edit commit message" disabled; on an older commit → dialog with the full message → Save message → rebased, message kept. Typing and multi-selection (Ctrl/Shift+click) could not be checked by hand (WSLg keyboard; maintainer to-do).

- Commit context menu "Edit commit message": HEAD → amend; older commit on the current branch → interactive rebase with a single reword (no panel).
- Multi-selection "Squash N commits" (contiguous, on current branch, not containing merges) → interactive rebase with squash, message dialog prefilled with all messages.

Acceptance: integration tests for both, incl. rejection of non-contiguous selections.

### [x] M4-09 Cherry-pick and revert

> Done: `git_core::{cherry_pick, revert, commit_empty}` (`git cherry-pick <ids…>` in the given order, `git revert --no-edit [-m N]`; a stop leaves `CHERRY_PICK_HEAD`/`REVERT_HEAD` and returns `Conflicts`; "Commit empty" = `commit --allow-empty --no-edit`, then `--continue` while the sequencer has steps left). App: commit menu "Cherry-pick commit" (not for HEAD) and "Revert commit", multi-selection "Cherry-pick N commits" (oldest first), revert of a merge → form with the parents (default parent 1), blocked during an operation; after a cherry-pick or revert the next refresh checks for an empty result (operation in progress, no conflicts, nothing staged or changed) → dialog "Abort" / "Skip" (cherry-pick only) / "Commit empty". Tests: git-core integration (single, several in order, conflict + abort, empty → skip and → commit empty with the next commit following, revert, revert of a merge with and without mainline, conflicting revert), app (menu rules, merge revert form → mainline 2, empty dialog for cherry-pick and revert). Checked by hand: cherry-pick of a commit whose change is already on main → "Empty cherry-pick" dialog → Commit empty → empty commit with the original message; menu on HEAD: cherry-pick and interactive rebase disabled. Note (→ M4-13): reverting a commit without changes fails in Git itself ("nothing to commit", no state left); Gitloom shows Git's error.

- Cherry-pick one or many selected commits (applied oldest first).
- Revert one commit; merge commits ask for the mainline parent (`-m`).
- Empty result → dialog "Skip" or "Commit empty".
- Conflicts → M4-03/M4-05.

Acceptance: integration tests for single, multiple, conflict, empty, revert of a merge.

### [ ] M4-10 Reset

- Commit context menu "Reset <current> to this commit" with Soft / Mixed / Hard; Hard always confirmed and lists the changes that will be lost.
- Snapshot for hard reset per planning decision.

Acceptance: integration tests for all three modes incl. untracked files (hard reset keeps untracked files, like Git).

### [ ] M4-11 Drag and drop of branches

- Drag a branch pill (graph) or sidebar branch; drop on another branch pill or sidebar branch; drop target highlighted while hovering.
- Menu per planning decision; Esc or drop outside cancels.

Acceptance: unit tests for the menu rules (fast-forward possible, which branch must be checked out); manual check.

### [ ] M4-12 Undo/redo for history operations

- Record HEAD before/after (and the snapshot for hard reset) for merge, fast-forward, rebase, interactive rebase, cherry-pick, revert, reset, reword, squash.
- Undo = reset to the recorded HEAD (+ snapshot restore for hard reset) only if HEAD still equals the recorded "after" value; redo symmetric.
- Aborted operations create no entry.

Acceptance: integration tests for every undo/redo pair and failing preconditions.

### [ ] M4-13 Edge cases and robustness

- Hooks failing during continue → error dialog, state kept.
- Rebase of a branch with merge commits: Git's default (merges flattened); show a hint in the confirmation.
- Operations started while auto-fetch runs: auto-fetch yields (ADR-013).
- Large conflicted files (> 2 MB): view falls back to "take ours/theirs" only.

Acceptance: integration tests for each case.

### [ ] M4-14 End-to-end check, docs, performance

- Full walkthrough per milestone acceptance on Linux; maintainer repeats on Windows (Maintainer to-dos).
- Update UX-PARITY, CHANGELOG, PERFORMANCE (conflict view with 5 000-line file, interactive rebase of 200 commits).

### [ ] M4-15 Release v0.5.0

Same procedure as before (`/release`), only when the maintainer asks.
