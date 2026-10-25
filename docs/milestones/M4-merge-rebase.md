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

### [ ] M4-01 Multi-selection in the graph

- Ctrl+click toggles, Shift+click selects a range, Shift+↑/↓ extends (Shift+J/K stay parent/child navigation; document the difference in UX-PARITY).
- Right panel with several commits: "N commits selected", list of their summaries, actions (cherry-pick, squash if contiguous on the current branch).
- Selection by commit ids, survives refresh.

Acceptance: unit tests for the selection model (toggle, range, refresh).

### [ ] M4-02 Gitloom as Git editor

- Env flag (e.g. `GITLOOM_EDITOR=1`) plus a path to a prepared file: Gitloom started as editor copies the prepared content into the file Git passes and exits 0; missing preparation → exit 1 (Git aborts cleanly).
- Used as `GIT_SEQUENCE_EDITOR` (todo list) and `GIT_EDITOR` (commit messages for reword, squash, merge, revert).
- Never starts a UI in editor mode.
- Record as addendum to the askpass ADR.

Acceptance: integration tests with a prepared todo list and a prepared message; missing preparation aborts without hanging.

### [ ] M4-03 Sequencer states: continue, skip, abort

- Banner buttons become active: Continue (`--continue`), Skip (rebase/cherry-pick), Abort (`--abort`) for merge, rebase, cherry-pick, revert.
- Continue requires all conflicts resolved; otherwise disabled with tooltip listing the files.
- Message for continue (merge/revert/squash) comes from the commit panel via the editor mode.
- Staging panel shows a "Conflicted Files" section above unstaged files.

Acceptance: integration tests for each state with continue, skip and abort.

### [ ] M4-04 Merge and fast-forward

- Branch context menu "Merge <branch> into <current>" (`git merge`, respecting user config for ff).
- "Fast-forward <branch> to <commit>" for branches that are not checked out (`git fetch . <commit>:<branch>` or `update-ref` with ancestry check) and for the current branch (`git merge --ff-only`).
- Conflicts → state banner + conflict view.

Acceptance: integration tests for ff, merge commit, conflict, fast-forward of a non-checked-out branch.

### [ ] M4-05 Conflict resolution view

- Clicking a conflicted file opens the view in the center (replaces graph like the diff view).
- Data from index stages 1/2/3; default output from `git merge-file -p` (with markers); a pure model in git-core maps hunks/lines of ours and theirs to the output.
- Checkboxes per hunk and per line in ours/theirs update the output; the output is editable (text editor) and wins over checkboxes once edited (indicator + "Reset output").
- Buttons: take all ours, take all theirs, save and mark resolved (`git add`). Binary, delete/modify and add/add conflicts: choose ours or theirs file.
- Line endings preserved (CRLF repos).

Acceptance: unit tests for the conflict model (overlapping hunks, line picks from both sides, CRLF, no trailing newline); integration test resolve → continue.

### [ ] M4-06 Rebase

- Branch context menu "Rebase <current> onto <branch>" (`git rebase <branch>`).
- Stops on conflicts → conflict view → continue/skip/abort (M4-03).
- Dirty tree handling per planning decision.

Acceptance: integration tests: clean rebase, rebase with conflict resolved, abort.

### [ ] M4-07 Interactive rebase

- Entry: commit context menu "Interactive rebase from here" (commits after the selected one up to HEAD), drag and drop menu (M4-11).
- Panel per planning decision; validation (first commit cannot be squash/fixup; at least one commit kept).
- Execution: prepared todo via M4-02, reword/squash messages prepared upfront; stops for conflicts handled by M4-03/M4-05.

Acceptance: integration tests for reorder, reword, squash (combined message), fixup, drop, conflict during reorder.

### [ ] M4-08 Reword and squash shortcuts

- Commit context menu "Edit commit message": HEAD → amend; older commit on the current branch → interactive rebase with a single reword (no panel).
- Multi-selection "Squash N commits" (contiguous, on current branch, not containing merges) → interactive rebase with squash, message dialog prefilled with all messages.

Acceptance: integration tests for both, incl. rejection of non-contiguous selections.

### [ ] M4-09 Cherry-pick and revert

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
