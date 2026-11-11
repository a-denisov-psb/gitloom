# M9 – Fixes and consistency

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop`. Found by the maintainer while testing rc.3/rc.4 (2026-11-11).

## Goal

A stash that conflicts is a normal conflict with a clear way back: Undo returns to the exact state before the pop or apply. Discard and reset work while files are conflicted. Icons stay sharp at every zoom level. The left panel and the commit panel follow GitKraken more closely.

## Scope

In scope:

- Stash pop/apply that stops in conflicts: outcome instead of error, banner, conflict view labels, "Drop stash"
- Undo and redo of stash pop and apply (with and without conflicts) by snapshot
- Discard and "Discard all" for conflicted files, undoable
- Reset on the HEAD commit
- Sharp SVG icons at fractional scale factors
- Left panel: section order and GITFLOW content like GitKraken
- Commit panel: character counter inside the summary field
- ADR-043/ADR-044 applied to these areas: `docs/user/undo.md`, user guide, UX-PARITY, release checklist

Out of scope (later):

- Stash node look and "WIP on <branch>" (M11)
- Toasts instead of modal error boxes, undo for the remaining actions (M12)

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Stash conflict detection | `git stash pop`/`apply` exiting non-zero while `git ls-files -u` lists paths afterwards is the outcome `Conflicts` (Git keeps the stash); every other failure stays an error. Stderr that comes with the conflict (seen: `error: could not write index`) is not shown as an error. M9-01 reproduces the maintainer's case (stash with a staged change, then a merge touching the same file, then pop) and records the cause of that message in its note. |
| Banner | Like the banner of a stopped merge: "Stash '<label>' applied with conflicts. The stash was kept." with **Undo** (M9-02) and **Drop stash** (disabled with "Resolve the conflicts first" while conflicted files remain). Without conflicted files: "Conflicts resolved. The stash '<label>' is still in the list." with **Drop stash** and **Keep stash** (closes the banner). State per tab in memory. |
| No known operation | Conflicted files while Git runs no operation and the tab has no stash-conflict state (e.g. after a restart): banner "Conflicted files without a running operation" with "Discard conflicted files…" (M9-03) and the hint to resolve them in the conflict view (ADR-044). |
| Conflict view sides | For a stash conflict: A "Updated upstream" (HEAD), B "Stashed changes" (stash label), like Git's markers. "Save and mark resolved" stages the file as today. |
| Undo of pop/apply | Snapshot before the command: HEAD; the index tree (`git write-tree`; the index has no conflicts before a pop or apply, which Git requires); `git ls-files -s`; blobs (`hash-object -w --no-filters`) of every modified and untracked file; the list of untracked paths; stash commit id and message. After the command: whether the stash was dropped, and the content ids of every path the command touched. Undo: precondition HEAD unchanged ("the repository changed" otherwise); `git read-tree <tree>` (also clears unmerged entries), write back the recorded files, delete paths that did not exist before (created by the stash), and if the stash was dropped `git stash store -m "<message>" <oid>`. If a touched path changed after the command (e.g. a conflict was partly resolved), ask: "Discard your conflict resolution and return to the state before the pop?". Redo: pop/apply the same stash commit again (located by id; if it is no longer in the list, redo refuses with a message). Entries "Pop stash"/"Apply stash"; successful pops and applies are undoable the same way. The banner's Undo runs this entry and is shown only while it is on top of the undo stack. Extends ADR-014. |
| Discard conflicted files | Per file: `git restore --source=HEAD --staged --worktree -- <path>`; a path not in HEAD is removed from index and working tree. Confirmation: "Discard <file>? Both sides and any resolution are lost." "Discard all" includes conflicted files. Undo restores the working tree content and the unmerged index entries (`git ls-files -s` before, `git update-index --index-info` on undo). |
| Reset on HEAD | "Reset <branch> to this commit…" also on the HEAD commit while no Git operation runs (conflicted files from a stash allowed). Same dialog (maintainer: keep the dialog, 2026-11-11). On HEAD, Soft is disabled with "HEAD is already at this commit"; Mixed clears the index including unmerged entries and keeps the files; Hard discards uncommitted changes (untracked files stay, as today). Undo restores the index including unmerged entries; Hard keeps its snapshot. |
| Sharp icons | Icon size rounded so that size × scale is a whole number of physical pixels; a small wrapper widget draws the icon translated by the fractional part of its physical position. Scale = zoom × window scale factor. Applies to every helper in `icons.rs`; text unchanged. Same idea as M6-23. |
| Left panel order | GITFLOW (when initialized), LOCAL, REMOTE, STASHES, TAGS, SUBMODULES (GitKraken, maintainer check 2026-11-11). Replaces M7-15. |
| GITFLOW content | Like GitKraken: a tree of the local production branch, the development branch and the branches with a configured Git flow prefix (feature, release, hotfix); the prefix is a folder ("feature" › "f2"). Same row components and interactions as LOCAL, checked-out branch highlighted, filter applies. Order: one case-insensitive alphabetical list with folders and branches mixed (GitKraken screenshot: develop, feature, master). Count = number of branches. Green "+" menu unchanged; missing production or development branch is not shown. Replaces the groups Features/Releases/Hotfixes of M7-08. LOCAL and REMOTE keep their order until checked (verification item). |
| Summary counter | Inside the summary field at the right edge like GitKraken, muted; 10 or fewer left: warning color; below 0: error color (commit not blocked, as today). The field gets right padding so text never runs under the counter; summary and description have the same width. |

## Maintainer answers and GitKraken verification items

Answers from the maintainer (2026-11-11, GitKraken 7.5.2 screenshots). They also answer open items of M7 and M8:

| Item | Answer |
|---|---|
| Left panel order | GITFLOW, LOCAL, REMOTE, STASHES, (Issues), (Boards), TAGS, SUBMODULES, (GitHub Actions); provider sections are non-goals |
| GITFLOW section content | Tree of develop, master and prefixed branches, folders mixed alphabetically with branches |
| Commit summary counter | Inside the summary field, right aligned |
| Reset | GitKraken uses a submenu Soft/Mixed/Hard; the maintainer prefers Gitloom's dialog |
| Double click on a file in staging | No action in GitKraken; Gitloom keeps none |
| Identity in the commit panel | Not in GitKraken (Commit Message, Summary, Description only): Gitloom extension (M8) |
| Repo-specific categories (GitKraken 7.5.2) | Gitflow, Commit Template, LFS, Issue Tracker |
| External editor | GitKraken: Preferences → General → "External Editor" dropdown; no shortcut seen |
| SSH | GitKraken: one global key pair, "Use local SSH agent", "Generate"; no per-repository key (M8 deviation confirmed) |

Open:

| Item | Used in |
|---|---|
| LOCAL/REMOTE: are folders mixed with branches alphabetically like in GITFLOW? | M9-06 (keep folders first until answered) |
| GitKraken after a stash conflict: are resolved files staged or unstaged? | M9-01 |

## Milestone acceptance

- [ ] A stash pop or apply that conflicts shows the banner, the conflicted files and the conflict view with "Updated upstream"/"Stashed changes"; no error dialog; "Drop stash" after resolving.
- [ ] Undo after a stash pop or apply (with and without conflicts, on a clean tree and on a tree with other local and untracked changes) restores the working tree byte for byte, `git ls-files -s` and the stash list exactly; redo applies again.
- [ ] Conflicted files can be discarded (single and all) after confirmation; undo restores content and unmerged state.
- [ ] Reset is offered on the HEAD commit; Mixed and Hard on HEAD leave no unmerged entries; undoable.
- [ ] Icons are sharp at 100, 110, 125 and 150 % (snapshots checked by eye).
- [ ] Left panel order and GITFLOW content like GitKraken; counter inside the summary field.
- [ ] `docs/user/undo.md`, user guide, UX-PARITY, release checklist and CHANGELOG updated; `./scripts/check.sh` green; CI green.

No release (ADR-042): when all tasks are done, continue with M10.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M9-01 | Stash pop/apply with conflicts | – |
| M9-02 | Undo and redo of stash pop/apply | M9-01 |
| M9-03 | Discard conflicted files | – |
| M9-04 | Reset on the HEAD commit | M9-03 |
| M9-05 | Sharp icons at fractional scales | – |
| M9-06 | Left panel order and GITFLOW content | – |
| M9-07 | Summary counter inside the field | – |
| M9-08 | Documentation | M9-01 … M9-07 |

---

## Tasks

### [x] M9-01 Stash pop/apply with conflicts

- git-core: `stash_pop`/`stash_apply` return an outcome (`Applied` / `Conflicts`) per the decisions table; unit and integration tests.
- App: stash-conflict state per tab, banner with Drop stash/Keep stash, generic banner for conflicted files without an operation, conflict view side labels.
- Reproduce the maintainer's sequence and explain `error: could not write index` in the task note.

Acceptance: integration tests: pop with conflict → `Conflicts`, stash still listed, unmerged path present; apply likewise; a non-conflict failure stays an error. App tests: banner texts and button states before and after resolving, Drop stash, Keep stash, generic banner. Snapshot of the banner in both themes.

Note (2026-11-11): done together with M9-02 on one branch (`feature/M9-01-stash-conflicts`): the outcome reaches the app inside the new undo entry, so the two cannot be merged apart. git-core module `stash.rs` (`apply_stash`, `apply_stash_recorded`, `unmerged_paths`); `stash_pop`/`stash_apply` left the `GitWriter` trait. Cause of `error: could not write index`: the first pop stopped in conflicts and showed as an error, so the stash looked untouched and was popped again; Git refuses to apply a stash while the index has unmerged entries ("<file>: needs merge", newer Git versions add "could not write index" because refreshing the index fails). Gitloom now refuses that up front (Stash/Pop/Apply disabled with "Cannot stash while files are conflicted: resolve or discard them first") and never shows a conflicting pop as an error. The maintainer's sequence (staged change stashed, conflicting merge, pop) is the integration test fixture `stash_then_conflicting_merge`. The banner's Drop stash deletes without the "Delete stash" confirmation: the changes are in the working tree by then. Snapshots `ui_check_stash_conflict_banner` (conflicts, resolved, generic; dark/light) checked. Open GitKraken item (resolved files staged or unstaged) unanswered: resolved files are staged as for every conflict.

### [x] M9-02 Undo and redo of stash pop/apply

- Snapshot and undo per the decisions table; `UndoAction` entry; banner Undo; confirmation when touched paths changed; redo.
- ADR-014 gets an extension paragraph.

Acceptance: integration tests in git-core, each comparing working tree bytes, `git ls-files -s` and `git stash list` before and after undo: pop with conflict on a clean tree; pop with conflict while another file is modified and an untracked file exists; stash with untracked files; successful pop (stash restored by `stash store`); apply. Redo after undo; redo refused when the stash is gone. App tests: Ctrl+Z, toolbar and banner run the same entry; confirmation after editing a conflicted file.

Note (2026-11-11): done (see M9-01). Only the paths the stash touches are recorded (its working tree, index and untracked changes against its base), which covers everything a pop or apply can change; other local changes are left alone. A successful pop of an older entry is restored at its old position (newer entries dropped and stored again in order), so `git stash list` matches exactly. Created folders are removed again. Ctrl+Z, the toolbar and the banner all send `Message::Undo`; for a stash entry the app first checks in the background whether touched files changed and then asks. ADR-014 extended. Tests: `crates/git-core/tests/stash_conflicts.rs` (13), app tests `stash_*`/`undo_of_a_stash_pop_*`.

### [x] M9-03 Discard conflicted files

- Discard (context menu of a conflicted file) and "Discard all" handle unmerged paths per the decisions table, with confirmation; generic banner action "Discard conflicted files…".
- Undo restores content and unmerged entries.

Acceptance: integration tests: discard of a conflicted file (in HEAD and not in HEAD), discard all with conflicted and normal changes, undo restores stages 1–3 and content. App test for the confirmation.

Note (2026-11-11): done. `DiscardSnapshot` index entries carry unmerged stages; undo/redo write them with `git update-index --index-info` after removing the path. A path not in HEAD is removed with `git rm --cached --force` and deleted. Discard of a conflicted file closes its conflict view. The banner action discards every conflicted file after naming them ("Discard 2 conflicted files (a.txt, c.txt)? …"). Discarding conflicted files during a merge leaves the merge running (Abort stays the way out). Tests `crates/git-core/tests/discard_conflicted.rs`, app tests `discarding_a_conflicted_file_*`, `banner_discards_*`; snapshot `stash-conflict-2-orphaned` checked.

### [x] M9-04 Reset on the HEAD commit

- Menu entry on HEAD; Soft disabled on HEAD with the reason; Mixed/Hard clear unmerged entries; undo restores them.

Acceptance: menu tests (HEAD, during an operation, with conflicted files from a stash); integration tests for Mixed and Hard on HEAD with unmerged entries and their undo.

Note (2026-11-11): done. `git write-tree` refuses unmerged entries, so the index is now recorded by `record_index`: unmerged entries entry by entry, the rest as a tree (the unmerged entries are taken out for `write-tree` and put back right away, so the reset itself runs on the untouched index); `restore_index` puts both back. Used by soft/mixed resets (`HistoryUndo::Reset.unmerged`) and hard resets (`ResetSnapshot.unmerged`), on any commit. Dialog buttons can now be disabled with a tooltip (`DialogButton.disabled`). Tests: app `reset_is_offered_on_the_head_commit`, `soft_reset_stays_available_on_other_commits`; git-core `mixed_reset_on_head_*`, `hard_reset_on_head_*`.

### [x] M9-05 Sharp icons at fractional scales

- Size rounding and position snapping in `icons.rs` per the decisions table; all icon call sites go through it.

Acceptance: `iced_test` snapshots of the left panel headers (incl. the "+" of REMOTE and GITFLOW), toolbar and file lists at 100, 110, 125 and 150 %, checked by eye; unit tests for the rounding.

Note (2026-11-11): done. Every helper in `icons.rs` returns the wrapper widget `Sharp`: the size is rounded by `snap_size` (whole physical pixels), and `draw` translates the SVG by `snap_offset` of its bounds. The scale is zoom × the window's scale factor from the operating system (`window::scale_factor` when the window opens, `window::Event::Rescaled` afterwards), set by `ui::view` before each frame. No call site needed changes; no other SVG is drawn outside the helpers. The `iced_test` simulator always renders at scale 2, so `ui_check::snapshot_scaled` renders the real view with the tiny-skia renderer at 1.0/1.1/1.25/1.5 (`ui_check_sharp_icons`, flow app with GITFLOW "+", REMOTE "+", toolbar and a conflicted file, both themes); compared by eye with unsnapped renders (temporary switch, not committed). tiny-skia draws SVGs as vectors, so the difference there is small; the blur the maintainer saw comes from wgpu rasterizing an SVG at a fractional size and sampling it at a fractional position, which snapping removes. To confirm on Windows at 110/125 % (release checklist, M9-08).

### [ ] M9-06 Left panel order and GITFLOW content

- Section order and GITFLOW tree per the decisions table; keyboard focus follows the new order.
- UX-PARITY: left panel row updated, deviation "The GITFLOW section sits between LOCAL and REMOTE" removed; M7-08/M7-15 notes point to M9-06.

Acceptance: layout test for the header order; tree tests (mixed order, prefixes as folders, missing branches, filter); snapshot `ui_check_gitflow_section` updated.

### [ ] M9-07 Summary counter inside the field

- Counter inside the field per the decisions table.

Acceptance: snapshots of the commit panel with 69, 5 and −3 characters left in both themes; long summaries never overlap the counter.

### [ ] M9-08 Documentation

- `docs/user/undo.md`: stash pop/apply, discard of conflicted files, reset on HEAD; a "Cannot be undone" list with reasons (ADR-043).
- User guide `merge-rebase-conflicts.md`: stash conflicts, Undo, Drop stash; `settings.md`/`daily-workflow.md` where affected.
- UX-PARITY: rows for stash conflicts and reset on HEAD; the Reset deviation keeps the dialog with the reason "maintainer prefers the dialog (2026-11-11)".
- `docs/RELEASE-CHECKLIST.md`: new section "7c. New in M9" (stash conflict with Undo on Windows, discard conflicted, reset on HEAD, icons at 110/125 %, left panel order, summary counter).
- CHANGELOG `[Unreleased]`.

Acceptance: docs reviewed against the implemented behavior.
