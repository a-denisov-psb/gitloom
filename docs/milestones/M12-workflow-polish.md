# M12 – Workflow polish

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after M11.

## Goal

The daily workflow feels like GitKraken in the details: file lists, conflict handling and running operations look and behave like GitKraken, submodules work, errors appear as non-modal notifications in plain language, a remote's website is one click away, and the remaining local actions can be undone.

## Scope

In scope:

- File lists: click on the selected file deselects it; hover shows only Stage/Unstage
- "Open on GitHub/GitLab/Bitbucket" / "Open in browser" for remotes
- Conflict view: "conflict n of N" navigation, side headers
- Running operations like GitKraken: graph row, details panel header, Conflicted/Resolved Files, "Mark all resolved", "Commit and …"/"Abort …" buttons; replaces the tab banner
- Submodules: list with status, update/initialize, open, out-of-date notice
- Toast notifications with plain-language error texts
- Undo coverage (ADR-043): stash push and drop, stage/unstage, mark resolved, the local part of pull, remotes and upstream, submodule update

Out of scope:

- Notification location setting (GitKraken has one; fixed bottom left)
- Adding, removing or moving submodules (Git CLI)
- Undo for settings (ADR-043 amended)

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Deselect | In every file list of the right panel (Unstaged, Staged, Conflicted, Resolved, commit file list) a click on the selected file deselects it and closes the diff or conflict view, like Esc. Graph commits keep their behavior. |
| Hover buttons | Hovering a file shows only "Stage" (unstaged list) or "Unstage" (staged list). Per-file Discard moves to the context menu (it is already there). Deviation from GitKraken (maintainer decision 2026-11-10): tidier, no accidental discard. "Discard all" in the header stays. |
| Remote website | Remote context menu (REMOTE section) and palette ("Open remote in browser": origin, else the only remote, else a list): "Open on GitHub" / "Open on GitLab" / "Open on Bitbucket" for `github.com`, `gitlab.com`, `bitbucket.org`, "Open in browser" for other hosts. URL derived offline: scp-like `git@host:owner/repo.git`, `ssh://[user@]host[:port]/owner/repo.git` and `https://[user[:pw]@]host[:port]/owner/repo.git` become `https://host/owner/repo` (credentials, SSH ports and `.git` removed; HTTPS ports kept). Local paths and `file://` remotes: entry not shown. Only the repository start page, no commit or branch links. |
| Conflict navigation | Output header "conflict n of N" with ↑/↓ buttons (and Alt+↑/Alt+↓ while the view is focused) jumping between open conflicts in the output, like GitKraken; replaces "N conflicts left". Side headers: "Commit <short sha> on <branch>" (A = HEAD side, B = incoming side); for stashes "Updated upstream" / "Stashed changes" stay (M9). |
| Operation state | Like GitKraken instead of the tab banner: (1) a row at the top of the graph in the warning color: "A file conflict was found when attempting to <merge into develop / rebase develop / cherry-pick 1a2b3c4 / revert 1a2b3c4 / apply stash 'WIP on develop'>" (without conflicts: "<Operation> in progress"); (2) the details panel shows a header bar "<n> file conflict(s) in working directory" with "View conflicts" (selects WIP) whenever another commit is selected; (3) with WIP selected the panel title reads "Merging <theirs> into <ours>" (rebasing/cherry-picking/reverting/applying stash accordingly), the lists are "Conflicted Files" and "Resolved Files" (files that were conflicted when the operation stopped and are now staged), "Mark all resolved" stages all conflicted files after a confirmation that names files still containing conflict markers, and the buttons at the bottom are "Commit and merge" / "Continue rebase" / … and "Abort merge" / … (stash: "Drop stash" and "Undo" from M9). Skip stays for rebase and cherry-pick. The banner of M2-13/M9 is removed; ADR-044 is met by the row, the panel header and the buttons. |
| Submodules | Section SUBMODULES always shown with its count, like GitKraken (also 0). Entries from `git submodule status --recursive` (background, on refresh when `.gitmodules` exists): name/path with a status icon and tooltip: not initialized, out of date (checked-out commit differs from the recorded one), modified (dirty), conflict. Context menu: "Update" (`git submodule update --init --recursive -- <path>`), "Open in new tab", "Open in file manager", "Copy path". Section header "+"-less; header context menu "Update all submodules". Double click opens the submodule in a new tab (or switches to it). After checkout, pull, merge or rebase with out-of-date submodules: an info toast "Submodules are out of date" with the button "Update". Network progress, cancel and askpass like fetch. |
| Toasts | Notifications at the bottom left of the window (GitKraken default), stacked up to 3, newest at the bottom. Kinds: error (red, stays until closed), warning (stays), info/success (closes after 6 s, hover pauses). Error toasts: title ("Push failed"), one or two plain-language sentences, buttons "Details" (expands Git's command and output, selectable) and "Copy". All error dialogs that only inform become toasts; dialogs that ask for a decision stay modals (e.g. locked index with Retry, stash-and-checkout, force push). Status bar confirmations stay. ADR-049. |
| Plain-language texts | A catalog in git-core maps Git stderr patterns to a title, explanation and hint: index lock, unmerged paths, local changes would be overwritten, non-fast-forward push rejected, authentication failed (HTTPS and `Permission denied (publickey)`), host not resolvable / network unreachable, nothing to commit, not a Git repository, signing failed (M10), detached HEAD for operations that need a branch. Unknown errors: "Git reported an error." plus Details. |
| Undo: stash push | Entry "Stash": undo applies the stash with `--index` and drops it, only if the working tree and index equal the state right after the push (otherwise "the repository changed"); redo stashes again. |
| Undo: stash drop | Entry "Delete stash": undo `git stash store -m <message> <oid>` (back on top of the list). |
| Undo: stage/unstage | Every stage/unstage action (file, several files, hunk, lines, block, all) records `git ls-files -s` of the affected paths; undo restores those index entries with `git update-index --index-info` (removing entries that did not exist), redo repeats the action. Precondition: the index entries are still as the action left them. |
| Undo: mark resolved | "Save and mark resolved" and "Mark all resolved" record the unmerged entries and the working tree content; undo restores both. |
| Undo: pull | The local part of a pull (fast-forward, merge, rebase) is undone like the matching operation (branch ref back, working tree via the existing history undo); the fetched remote-tracking refs stay. Fetch stays not undoable. |
| Undo: remotes and upstream | Add remote → remove; remove remote → re-add with all recorded `remote.<name>.*` config keys and restore its remote-tracking refs with `git update-ref`; rename → rename back; URL change → old URL; set/unset upstream → previous `branch.<name>.remote`/`merge`. |
| Undo: submodule update | Records each submodule's previous HEAD; undo checks it out again (detached, as Git leaves submodules) when the submodule's working tree is unchanged since the update. |
| Settings | Changes in the Preferences or the Git config (identity, SSH key, Gitflow, signing, commit template, general options) are not undo entries; `docs/user/undo.md` lists them under "Cannot be undone" with "change them back in the Preferences". ADR-043 is amended accordingly. |

## Maintainer answers and GitKraken verification items

Answered (GitKraken 7.5.2 screenshots, 2026-11-11): conflict state shows an orange graph row "A file conflict was found when attempting to merge into develop", a details header "1 file conflict in working directory · View conflict", the panel "Merge conflicts detected – Merging feature/f2 into develop" with Conflicted Files, Resolved Files, "Mark all resolved", "Commit and Merge", "Abort Merge"; the conflict view shows "Commit fcef63 on develop" / "Commit 44f76c on feature/f2" and "conflict 1 of 1" with arrows; a failed Git flow finish shows a red toast bottom left ("Failed to finish feature 'f2' – There are merge conflicts that need to be resolved"); SUBMODULES is shown with "0"; a click on the selected file deselects it.

Open:

| Item | Used in |
|---|---|
| Does GitKraken offer "Open on GitHub" for a remote without a connected integration? Label? | M12-03 |
| How long do GitKraken's info toasts stay? | M12-07 |
| "Mark all resolved" in GitKraken with conflict markers still in a file: warning or not? | M12-05 |

## Milestone acceptance

- [ ] Selected files deselect on click; hover shows only Stage/Unstage.
- [ ] Remote websites open from the context menu and the palette for all URL forms; local remotes show no entry.
- [ ] Conflict view navigation and headers like GitKraken.
- [ ] Merge, rebase, cherry-pick, revert and stash conflicts show the graph row, the panel header, Conflicted/Resolved Files, "Mark all resolved" and the commit/abort buttons; the tab banner is gone; every state has a visible way back (ADR-044).
- [ ] Submodules: status, update (also all), open in tab, out-of-date toast; works with nested submodules.
- [ ] Informational errors appear as toasts with plain-language text and details; decisions stay modals.
- [ ] Every action listed under "Undo" in the decisions table is undoable and redoable with the stated preconditions; `docs/user/undo.md` complete (ADR-043).
- [ ] User guide, UX-PARITY, release checklist and CHANGELOG updated; `./scripts/check.sh` green; CI green.

No release (ADR-042): when all tasks are done, continue with M13.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M12-01 | Deselect on click and Stage/Unstage-only hover | – |
| M12-02 | Toasts and plain-language errors | – |
| M12-03 | Remote website links | – |
| M12-04 | Conflict view navigation and headers | – |
| M12-05 | Operation state like GitKraken | M12-02, M12-04 |
| M12-06 | Submodules | M12-02 |
| M12-07 | Undo: stash push/drop, stage/unstage, mark resolved | M12-05 |
| M12-08 | Undo: pull, remotes, upstream, submodule update | M12-06 |
| M12-09 | Documentation | M12-01 … M12-08 |

---

## Tasks

### [ ] M12-01 Deselect on click and Stage/Unstage-only hover

- Per the decisions table; UX-PARITY deviation for the hover.

Acceptance: app tests for deselect in every list (diff/conflict view closes); snapshots of hovered rows.

### [ ] M12-02 Toasts and plain-language errors

- Toast area, kinds, stacking, timeout, Details/Copy; catalog in git-core; informational `Modal::error` call sites moved to toasts; ADR-049.

Acceptance: unit tests for the catalog (one stderr sample per entry, unknown falls back); app tests: stacking limit, timeout and hover pause (simulated time), error stays, Details toggles, decisions still modal; snapshots of each kind in all themes.

### [ ] M12-03 Remote website links

- URL derivation in git-core, menu and palette entries per the decisions table, browser via `open_target`.

Acceptance: unit tests for all URL forms (with user, password, ports, trailing slash, `.git`, uppercase host, local paths); app tests for labels per host and the palette choice.

### [ ] M12-04 Conflict view navigation and headers

- "conflict n of N" with arrows and Alt+↑/↓; side headers with short SHA and branch.

Acceptance: app tests for navigation (wraps or stops at the ends like GitKraken; record the choice), header texts for merge, rebase and cherry-pick; snapshot.

### [ ] M12-05 Operation state like GitKraken

- Graph row, details header, panel title, Conflicted/Resolved Files (resolved set recorded when the operation stops; after a restart: staged files of the operation), "Mark all resolved" with marker check, bottom buttons; banner removed; M9 stash state uses the same parts.

Acceptance: app tests per operation (texts, buttons, resolved list, marker warning); integration test: merge with two conflicts → mark all resolved → commit and merge; snapshots in all themes.

### [ ] M12-06 Submodules

- Status reading, section, menus, update with progress/cancel/askpass, open in tab, out-of-date toast after checkout/pull/merge/rebase.

Acceptance: integration tests with a local superproject and submodule (not initialized, out of date, modified, nested); app tests for menus, double click and the toast; snapshot of the section.

### [ ] M12-07 Undo: stash push/drop, stage/unstage, mark resolved

- Undo entries per the decisions table.

Acceptance: integration tests per entry comparing working tree, `git ls-files -s` and stash list before/after undo and redo, including a refused undo after an outside change.

### [ ] M12-08 Undo: pull, remotes, upstream, submodule update

- Undo entries per the decisions table; ADR-043 amendment applied.

Acceptance: integration tests per entry (pull with fast-forward, merge and rebase; remove remote restores config and tracking refs; rename; URL change; upstream; submodule update) including refused cases.

### [ ] M12-09 Documentation

- `docs/user/undo.md` complete with the "Cannot be undone" list; user guide for operations, toasts, submodules, remote links; `coming-from-gitkraken.md`.
- UX-PARITY rows and deviations (hover, fixed toast location).
- `docs/RELEASE-CHECKLIST.md` section "7f. New in M12".
- CHANGELOG `[Unreleased]`.

Acceptance: docs reviewed against the implemented behavior.
