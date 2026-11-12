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
- STASHES as a tree at `/`, rename a stash
- Multi-selection in the left panel with bulk delete
- Command palette: settings toggles, choices and deep links
- Commit message prompt button in the commit panel
- WSL repositories: notice, polling watcher, trust prompt (preparation for a WSL mode, ADR-053)

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
| Stash folders | STASHES is a tree at `/` of the stash label (M11-05 rule). A label of the form "WIP on <branch>" is never split. Each level lists its stashes in stash order (newest first, as Git and the pop default), then its folders alphabetically (M11-11 comparator). Folder collapse remembered per repository; filter applies. Tooltip of a stash row: full message and date. |
| Rename stash | Context menu "Rename stash…" in STASHES and on the stash row in the graph (GitKraken wording to verify). Dialog with the label prefilled; empty disabled with "Enter a name"; `/` allowed (folders). New message `On <branch>: <name>` (branch from the old message; unparsable: `<name>`). Mechanism: `git commit-tree <stash>^{tree}` with the same parents and the author/committer name, e-mail and dates of the old stash commit (environment variables) and the new message; the entry is replaced **at its position**: record (oid, message) of `stash@{0}` … `stash@{n}`, drop them, `git stash store -m` them back from n to 0 with the new commit at n. On any failure the recorded entries are stored back and an error toast is shown. Undo: the same replacement with the old oid and message, precondition: entries 0 … n still have the oids left by the rename ("the stash list changed" otherwise); redo likewise. Entry "Rename stash". |
| Multi-selection in the left panel | Rows of LOCAL, REMOTE, GITFLOW and TAGS (not folders, stashes, submodules). Click: single selection and jump to the commit (as today). Ctrl+click toggles a row, Shift+click selects the range from the anchor in visible order (rows in collapsed folders are not included); Esc clears; filter changes drop rows that are no longer visible. Selected rows use the selection style of the graph multi-selection. |
| Bulk delete | Context menu on a selected row with two or more selected: "Delete <n> branches…" / "Delete <n> tags…" / "Delete <n> branches and tags…", plus "Hide in graph"/"Show in graph" for branches. One dialog lists the items grouped (Local branches, Remote branches per remote, Tags); the checked-out branch is listed greyed as "checked out, skipped". With remote branches the dialog says "Remote branches cannot be restored with Undo." Execution: local `git branch -d` for all, unmerged ones in one follow-up confirmation for `-D` (wording as the single delete); tags `git tag -d`; remote branches one `git push <remote> --delete <b…>` per remote with the network progress and cancel of M3. Partial failures: the others stay deleted, one error toast lists the failed items with reasons. Undo: everything local is one step "Delete <n> branches" (refs at the recorded oids, upstream config restored as for the single delete); remote deletions are listed in `docs/user/undo.md` → "Cannot be undone" (ADR-043). |
| Palette and settings | Ctrl+Shift+P opens the palette too (VS Code habit), unless the shortcut list shows a GitKraken binding for it (then only Ctrl+P, note in UX-PARITY). A settings registry (id, label, page, kind, keywords, repository-specific flag) is the single source for the Preferences pages' field anchors and the palette. Palette entries: (1) "Toggle: <label> (on/off)" for **every** boolean setting; (2) "Set: <label> ›" sub-lists **only** for settings changed in daily work: Theme, Diff view, Stash message template, External editor (global; repository override with a repository tab: "Repository: External editor ›"); "Choose identity…" stays and gets the keyword "profile"; (3) "Preferences: <page>" for every page and "Preferences: <label>" for every setting: opens the Preferences modal at the page, scrolls to the field and highlights it for 1.5 s. Repository-specific entries only with a repository tab active. Maintainer request 2026-11-11. |
| Commit prompt button | Small icon button (Lucide `sparkles`, vendored like the other icons; VS Code shows a sparkle in the same place) at the right end of the line "Committing as <identity>", right-aligned with the summary field. Click opens the modal "Commit message prompt" (M10-07). Tooltip "Copy commit message prompt…"; without staged changes disabled with the tooltip "Stage changes first". Context menu and palette entries stay. Maintainer request 2026-11-11 (the entry was hard to find). Gitloom extension (no AI inside Gitloom; the button only copies). |
| WSL repositories | On Windows a repository path under `\\wsl$\<distro>\…` or `\\wsl.localhost\<distro>\…` (case-insensitive, either slash) is a WSL repository (helper `wsl_location(path) -> Option<WslLocation { distro, linux_path }>`, ADR-053). (1) Notice bar in the tab: "This repository is inside WSL (<distro>). Gitloom uses Windows Git here, which is slow and can show wrong file modes or line endings. A WSL mode is planned." with "Don't show again" (stored per path in `settings.json`). Recent repositories show a muted badge "WSL: <distro>". (2) The file watcher uses polling (2 s) for these paths, since Windows change notifications on `\\wsl$` are unreliable. (3) Git's "detected dubious ownership" error (any repository) becomes a plain-language message with the button "Trust this repository…": after confirmation `git config --global --add safe.directory <path as Git prints it>`, then the repository opens (ADR-044). Not undoable (a setting; listed in `docs/user/undo.md`). |

## Maintainer answers and GitKraken verification items

Answered (GitKraken 7.5.2 screenshots, 2026-11-11): conflict state shows an orange graph row "A file conflict was found when attempting to merge into develop", a details header "1 file conflict in working directory · View conflict", the panel "Merge conflicts detected – Merging feature/f2 into develop" with Conflicted Files, Resolved Files, "Mark all resolved", "Commit and Merge", "Abort Merge"; the conflict view shows "Commit fcef63 on develop" / "Commit 44f76c on feature/f2" and "conflict 1 of 1" with arrows; a failed Git flow finish shows a red toast bottom left ("Failed to finish feature 'f2' – There are merge conflicts that need to be resolved"); SUBMODULES is shown with "0"; a click on the selected file deselects it.

Open:

| Item | Used in |
|---|---|
| Does GitKraken offer "Open on GitHub" for a remote without a connected integration? Label? | M12-03 |
| How long do GitKraken's info toasts stay? | M12-07 |
| "Mark all resolved" in GitKraken with conflict markers still in a file: warning or not? | M12-05 |
| GitKraken: "Rename stash"/"Edit stash message" in the stash menu? Wording? | M12-10 |
| GitKraken 7.5.2: multi-selection in the left panel and bulk delete? (if not: Gitloom extension) | M12-11 |
| GitKraken: any notice for repositories on WSL or network paths? | M12-14 |

## Milestone acceptance

- [ ] Selected files deselect on click; hover shows only Stage/Unstage.
- [ ] Remote websites open from the context menu and the palette for all URL forms; local remotes show no entry.
- [ ] Conflict view navigation and headers like GitKraken.
- [ ] Merge, rebase, cherry-pick, revert and stash conflicts show the graph row, the panel header, Conflicted/Resolved Files, "Mark all resolved" and the commit/abort buttons; the tab banner is gone; every state has a visible way back (ADR-044).
- [ ] Submodules: status, update (also all), open in tab, out-of-date toast; works with nested submodules.
- [ ] Informational errors appear as toasts with plain-language text and details; decisions stay modals.
- [ ] Every action listed under "Undo" in the decisions table is undoable and redoable with the stated preconditions; `docs/user/undo.md` complete (ADR-043).
- [ ] STASHES shows folders; a stash can be renamed in place (position kept); undo and redo restore the exact stash list.
- [ ] Several branches and tags can be selected with Ctrl/Shift and deleted in one step; the local part is one undo step; remote deletion is confirmed as not undoable.
- [ ] The palette toggles every boolean setting, offers the listed choices and jumps to any setting.
- [ ] The sparkle button opens the commit message prompt; disabled without staged changes.
- [ ] WSL repositories show the notice and the badge, refresh by polling, and "dubious ownership" offers "Trust this repository…".
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
| M12-09 | Documentation | M12-01 … M12-08, M12-10 … M12-14 |
| M12-10 | Stash folders and rename | – (needs M11-05) |
| M12-11 | Multi-selection and bulk delete in the left panel | M12-02 |
| M12-12 | Palette: settings toggles, choices and deep links | – (needs M11-13) |
| M12-13 | Commit prompt button | – |
| M12-14 | WSL notice, polling watcher, trust prompt | M12-02 |

---

## Tasks

### [x] M12-01 Deselect on click and Stage/Unstage-only hover

- Per the decisions table; UX-PARITY deviation for the hover.

Acceptance: app tests for deselect in every list (diff/conflict view closes); snapshots of hovered rows.

Note: the selected row sends `DiffClosed` (like Esc) in every list built by `file_row` (Unstaged, Staged, Conflicted, commit and stash files). Hover labels keep GitKraken's "Stage File" / "Unstage File". Tests: layout test `second_click_on_a_file_deselects_it` (unstaged, staged, conflicted), snapshot test `ui_check_file_row_hover` (no Discard on hover).

### [x] M12-02 Toasts and plain-language errors

- Toast area, kinds, stacking, timeout, Details/Copy; catalog in git-core; informational `Modal::error` call sites moved to toasts; ADR-049.

Acceptance: unit tests for the catalog (one stderr sample per entry, unknown falls back); app tests: stacking limit, timeout and hover pause (simulated time), error stays, Details toggles, decisions still modal; snapshots of each kind in all themes.

Note: see ADR-049 "Implementation". Timeouts are tasks that send `ToastExpired { id, seq }`; tests send it directly (simulated time). A fourth toast pushes out the oldest. `App::notify` and `Toast::with_action` are first used by M12-06. Existing tests that expected error dialogs now check the toast. Snapshots `ui_check_toasts` (error with details, warning, success, info; three themes) checked.

### [x] M12-03 Remote website links

- URL derivation in git-core, menu and palette entries per the decisions table, browser via `open_target`.

Acceptance: unit tests for all URL forms (with user, password, ports, trailing slash, `.git`, uppercase host, local paths); app tests for labels per host and the palette choice.

Note: `git_core::remote_web` (`web_url`, `web_label`); the entry follows "Fetch" in the remote menu; palette action "Open remote in browser" (origin, the only remote, else the list `Mode::RemoteWebsites`; disabled "No remote with a website"). A host without a dot (except `localhost`) counts as local. Opens with the existing `OpenLink` (`launch::open_target`).

### [x] M12-04 Conflict view navigation and headers

- "conflict n of N" with arrows and Alt+↑/↓; side headers with short SHA and branch.

Acceptance: app tests for navigation (wraps or stops at the ends like GitKraken; record the choice), header texts for merge, rebase and cherry-pick; snapshot.

Note: navigation wraps around at the ends (choice recorded; GitKraken's arrows cycle). The count follows the `<<<<<<<` lines of the output, so a hand-edited output counts its own markers; a step puts the editor cursor on the marker, which scrolls it into view. Alt+↑/↓ is the action "Previous/Next conflict" (context Staging, palette entry disabled outside the conflict view); the rebase panel receives the same keys as reorder; the output editor maps them too. Headers: A = HEAD commit on the checked-out branch (rebase: the branch at `onto`), B = merged commit / picked commit / replayed commit (`REBASE_HEAD`, new field `RepoState::Rebasing::commit`) with the branch pointing at it (rebase: the rebased branch); without a branch only "Commit <sha>". Marker labels in the output stay Git's. Tests: `navigation_wraps_around_the_open_conflicts`, `edited_output_counts_its_own_markers`, `conflict_headers_name_commit_and_branch`, `conflict_navigation_by_buttons_and_keys`; snapshot `ui_check_conflict_navigation` (dark, light) checked.

### [x] M12-05 Operation state like GitKraken

- Graph row, details header, panel title, Conflicted/Resolved Files (resolved set recorded when the operation stops; after a restart: staged files of the operation), "Mark all resolved" with marker check, bottom buttons; banner removed; M9 stash state uses the same parts.

Acceptance: app tests per operation (texts, buttons, resolved list, marker warning); integration test: merge with two conflicts → mark all resolved → commit and merge; snapshots in all themes.

Note: one model (`operation::status`) feeds the graph row, the details header, the panel title and the buttons for Git operations, stash conflicts (M9) and orphaned conflicts (ADR-044: "Discard conflicted files…" below the commit button). The graph row is pinned above the commit rows (does not scroll away) and selects WIP on click. "Mark all resolved" (open item answered conservatively): without markers it stages right away, with markers a confirmation names the files (three-line check `<<<<<<<`/`=======`/`>>>>>>>`). Resolved Files: the conflicted files recorded from the moment the operation appeared in this tab; when the tab opened during the operation (restart), every staged file. Resolved files are not repeated under Staged Files. A Git flow finish that stopped in a merge shows "Finishing … · Merging …" as panel title and "Abort finish"; the waiting finish bar (M7-10) stays. The banner's "Continue" message was removed (the commit button does the same). Tests: `operation_texts_like_gitkraken`, `stash_and_orphaned_conflicts_use_the_same_parts`, `resolved_files_are_the_conflicts_staged_during_the_operation`, `mark_all_resolved_confirms_files_with_markers`, git-core `mark_all_resolved_then_commit_and_merge`, `conflict_markers_need_all_three_lines_in_order`; snapshots `ui_check_operation_state` (merge WIP, details header, rebase; three themes) checked.

### [x] M12-06 Submodules

- Status reading, section, menus, update with progress/cancel/askpass, open in tab, out-of-date toast after checkout/pull/merge/rebase.

Acceptance: integration tests with a local superproject and submodule (not initialized, out of date, modified, nested); app tests for menus, double click and the toast; snapshot of the section.

Note: `git_core::submodule` parses `git submodule status --recursive`; initialized submodules with output from `git -C <path> status --porcelain --ignore-submodules=dirty` are "modified" (a dirty nested submodule marks only itself). The list is read in the background after every refresh (cheap without `.gitmodules`). The toast fires when HEAD changed since the previous read and a submodule is out of date (covers checkout, pull, merge, rebase and reset; the first read after opening stays quiet). Update is a network request (`NetworkKind::SubmoduleUpdate`) like fetch, so progress, Cancel and askpass apply. Opening a submodule matches tab roots exactly; the duplicate-tab check after opening now compares roots for equality (it closed nested repositories as "subfolders" before). The section is no longer collapsed by default; the old inactive header and its "Not available yet" tooltip are gone. Double click uses the same `double_click_area` as branch rows (tested via `OpenSubmodule`). Tests: git-core `status_lines`, `update_arguments`, integration `nested_submodules_modified_and_out_of_date`, `not_initialized_until_updated`, `no_gitmodules_no_submodules`; app `out_of_date_submodules_raise_a_toast_after_head_moved`, `submodule_menus_update_and_open`; snapshot `ui_check_submodules` (dark, light) checked.

### [x] M12-07 Undo: stash push/drop, stage/unstage, mark resolved

- Undo entries per the decisions table.

Acceptance: integration tests per entry comparing working tree, `git ls-files -s` and stash list before/after undo and redo, including a refused undo after an outside change.

Note: `git_core::work_undo`. Stage/unstage of files, lines, hunks and blocks record the index entries of the affected paths (`Scope::Paths`, incl. the old path of a rename); "Stage/Unstage all changes" record the whole index (`Scope::Whole`: tree plus unmerged entries, like reset M9-04), since untracked folders are not listed path by path. Redo writes the recorded "after" entries (equivalent to repeating the action under the precondition). Mark resolved ("Save and mark resolved", take one side, "Mark all resolved", staging a conflicted file) records index entries and file content. An action that changed nothing adds no entry. Stash push precondition: HEAD and `git status --porcelain -uall` as right after the push. Tests: `stage_and_unstage_paths_undo_and_redo`, `unstage_all_undo_restores_the_whole_index`, `mark_resolved_restores_conflict_stages_and_content`, `stash_push_undo_and_redo`, `stash_drop_undo_keeps_the_position` (each with refusal where applicable).

### [x] M12-08 Undo: pull, remotes, upstream, submodule update

- Undo entries per the decisions table; ADR-043 amendment applied.

Acceptance: integration tests per entry (pull with fast-forward, merge and rebase; remove remote restores config and tracking refs; rename; URL change; upstream; submodule update) including refused cases.

Note: network requests carry `NetworkUndo` (None, Pull, PullBranch, SubmoduleUpdate); the blocking part records around the command, so progress, Cancel and askpass stay. A pull that stops in conflicts records nothing (completing the merge is a commit, which is undoable; aborting restores the start). Undoing "Add remote" also removes refs fetched since; redo re-adds it without them. Restored config keys keep their order except after a rename back, where Git reorders them (same values). ADR-043 amendment (settings are not undo entries) was already applied in planning; ADR-043 got an implementation note. Tests: `pull_fast_forward_merge_and_rebase_undo_and_redo`, `pull_of_a_branch_that_is_not_checked_out`, `remove_remote_restores_config_tracking_refs_and_upstreams`, `add_rename_and_url_changes`, `upstream_set_and_unset`, `submodule_update_undo_and_redo` (each with refusals).

### [x] M12-09 Documentation

- `docs/user/undo.md` complete with the "Cannot be undone" list (also rename stash and bulk delete, the remote part under "Cannot be undone"); user guide for operations, toasts, submodules, remote links, `daily-workflow.md` (stash folders and rename, multi-selection and bulk delete), `settings.md` (palette entries); `coming-from-gitkraken.md`.
- UX-PARITY rows and deviations (hover, fixed toast location).
- `docs/RELEASE-CHECKLIST.md` section "7f. New in M12" (also: rename a stash in the middle of the list and undo it, bulk delete local and remote branches on Windows, palette toggles and deep links).
- `getting-started.md` (WSL repositories and "Trust this repository"), `undo.md` (safe.directory under "Cannot be undone"), UX-PARITY extensions (commit prompt button, WSL notice); release checklist 7f: sparkle button, open a repository from `\\wsl.localhost\…` on Windows (notice, badge, changes in WSL appear within a few seconds), dubious ownership.
- CHANGELOG `[Unreleased]`.

Acceptance: docs reviewed against the implemented behavior.

Note: most guide changes landed with their tasks (undo.md, merge-rebase-conflicts.md, branches-and-remotes.md, settings.md, troubleshooting.md); this task added the notifications row, WSL and trust in getting-started.md, remote website links, stash folders/rename and multi-selection in daily-workflow.md, the M12 differences in coming-from-gitkraken.md, the hover and fixed notification location in UX-PARITY and release checklist section 7f. Reviewed against the code and the snapshots of M12-01 … M12-14 and the panel toggle fix.

### [x] M12-10 Stash folders and rename

- STASHES tree, tooltip and rename per the decisions table; git-core `stash_rename(index, name)` with recovery; `UndoAction` entry.

Acceptance: integration tests: rename of `stash@{0}` and of `stash@{2}` of three keeps the order, the oids of the other entries and author/dates; a failure in the middle restores the list; undo/redo compare `git stash list --format='%H %gs'` exactly; undo refused after a new stash push. Unit tests for the folder split (custom label with `/`, "WIP on feature/f1" not split, order). Snapshot of STASHES with folders.

Note: `branch_tree::stash_tree` (stable order for stashes, folders after them by the M11-11 comparator), collapse keys `stashes:<path>`; the filter still matches the full message and expands all folders. Rename keeps tree, parents, author and committer by writing a copy of the stash commit with the new message (`git cat-file commit` → `git hash-object -t commit -w`, a `gpgsig` header is dropped) instead of `commit-tree` with environment variables (same result, no environment support in `GitWriter`); then drop `stash@{0…n}` and `git stash store` them back with the new commit at n; a failure stores the recorded entries back. Empty names show "Enter a name" on submit (forms have no per-field disabling). The menu is shared by STASHES and the graph's stash rows. Tests: `stash_tree_keeps_stash_order_then_folders`, `rename_stash_dialog`, `stash_menu_and_confirmed_delete`, git-core `rename_stash_in_place_with_undo_and_redo` (incl. refusal), `rename_first_and_last_of_three_keeps_the_others`, `a_failed_replacement_restores_the_list`; snapshot `ui_check_stash_folders` (dark, light) checked.

### [x] M12-11 Multi-selection and bulk delete in the left panel

- Selection model, Ctrl/Shift/Esc, context menu, grouped dialog, execution, partial failure toast, undo per the decisions table.

Acceptance: app tests for Ctrl/Shift ranges (also across collapsed folders), Esc, filter; menu entries for 1 vs. n selected and mixed kinds; dialog grouping incl. the skipped checked-out branch; integration tests: bulk delete of merged and unmerged local branches and tags, undo restores refs and upstream config in one step; remote deletion against a local bare remote, one push per remote; a partial failure leaves the rest deleted.

Note: `ref_selection` (pure selection model and `BulkPlan`), `RepoView::visible_refs` (panel order: GITFLOW, LOCAL, REMOTE, TAGS; collapsed sections/folders and the filter leave rows out; a Git flow branch counts once). The selection style shows only with two or more rows selected, so a plain click looks as before; Esc clears a multi-selection before closing the diff view. Local part: `git_core::delete_refs` one ref at a time (`-d`), failures collected; the forced part after "Delete anyway" joins the same undo step (`UndoAction::Refs { merge }`). The partial-failure report travels inside the undo entry to `WriteFinished` and is stripped there, so the generic write path (busy, lock retry, refresh) is kept. Remote branches: one `git push <remote> --delete …` per remote, run one after the other through `RepoView::network_chain` after the local part (or directly); Git refuses a remote's push as a whole when one of its branches no longer exists there, the other remotes stay deleted; remote failures use the regular error toast. Tests: `click_toggle_range_and_visibility`, `plan_groups_and_labels`, `left_panel_multi_selection`, `bulk_delete_runs_local_then_remote_and_reports`, git-core `local_branches_and_tags_with_undo_in_one_step`, `remote_branches_with_one_push_per_remote`; snapshots `ui_check_bulk_delete` (menu, dialog; dark, light) checked.

### [x] M12-12 Palette: settings toggles, choices and deep links

- Settings registry, palette entries and deep links per the decisions table; Ctrl+Shift+P.

Acceptance: unit test: every boolean setting of the registry has a toggle entry, every page and setting a "Preferences:" entry; app tests: a toggle changes the setting, a choice sub-list sets the value, "Preferences: Default branch name" opens the modal at the page with the field highlighted; repository-specific entries hidden on the start screen.

Note: `settings_registry` (label, page, field anchor, kind, keywords; page order). "Every boolean setting" covers the settings of `settings.json` (13 toggles, sending the same messages as the Preferences controls); on/off values that live in the Git config and are read in the background (long paths, sign commits/tags by default, the repository's commit template options) have "Preferences:" deep links instead, since their current value is not known without loading the page. "Custom" is not in the editor and stash template lists (it needs the text field: use the deep link). The field highlight is the accent tint for 1.5 s (timer with a sequence number); scrolling uses `snap_to` with the field's position among the page's fields. "Support long paths" is listed on Windows only, like its field. Palette entries got hidden `keywords` for the search ("profile" for "Choose identity…"). Ctrl+Shift+P has no GitKraken binding (checked against the shortcut list), so both open the palette. Tests: `every_boolean_setting_has_a_toggle`, `every_page_has_settings_and_choices_have_values`, `palette_settings_toggles_choices_and_deep_links`, `repository_settings_entries_need_a_repository`, `palette_runs_actions_and_remembers_them`; snapshots `ui_check_palette_settings` (dark, light) checked.

### [x] M12-13 Commit prompt button

- Button per the decisions table.

Acceptance: app tests: click opens the modal, disabled state and tooltip without staged changes; snapshots of the commit panel with and without staged changes in all three themes.

Note: Lucide `sparkles` (lucide-static 1.55.0, same version and ISC license as the other icons) vendored as `assets/icons/sparkles.svg`. The button has a fixed 22 px size and the identity line takes the rest of the row (it is cut, not the button). The click sends `CopyPromptRequested`, which builds the prompt in the background and opens the modal (`PromptReady`, M10-07). Tests: layout test `prompt_button_click` (click at the button sends the message with staged changes, nothing without), `prompt_button_is_disabled_without_staged_changes`, `copy_commit_prompt_needs_staged_changes`; snapshots `ui_check_prompt_button` (with/without staged changes, three themes) checked.

### [x] M12-14 WSL notice, polling watcher, trust prompt

- `wsl_location`, notice bar with "Don't show again", recent-list badge, watcher mode by location, dubious-ownership message and trust action, per the decisions table and ADR-053.

Acceptance: unit tests for `wsl_location` (both prefixes, case, `/` and `\`, distro names with `-` and `.`, non-WSL UNC paths, always `None` on non-Windows); unit test for recognizing Git's dubious-ownership message; app tests: notice shown and dismissed per path, badge in the recent list, watcher chooses polling for a WSL path; integration test (Linux): a repository owned by another uid (or `GIT_TEST_ASSUME_DIFFERENT_OWNER=1`) shows the message and opens after "Trust this repository…" with `safe.directory` written to a temporary global config.

Note: `git_core::wsl` (`wsl_location`, `None` off Windows; `parse_wsl_path` platform-independent, also for `\\?\UNC\` paths); the app's `wsl_of` uses the parser in tests so the notice, badge and watcher choice are tested on every platform. Watcher: `notify::PollWatcher` with 2 s for WSL paths, the native watcher otherwise. "Don't show again" stores the repository root in `settings.json` (`general.wsl_notice_dismissed`). Dubious ownership: opening runs `git rev-parse --git-dir` first, because gitoxide opens such repositories with reduced trust while Git refuses them; only that refusal fails the open (other errors keep the old behavior). The failed tab shows the plain-language text with "Trust this repository…"; write failures with the same error get the button in their toast; the catalog (M12-02) has the entry "Repository not trusted". Trust asks first, runs `git config --global --add safe.directory <path as Git prints it>` through git-core's command helper (ADR-053), then opens the tab again; listed under "Cannot be undone" (a setting). Integration test on Unix only (Git for Windows checks ownership by SID and ignores `GIT_TEST_ASSUME_DIFFERENT_OWNER`); it uses a temporary `GIT_CONFIG_GLOBAL` in its own test binary. Tests: `wsl_prefixes_case_and_separators`, `only_on_windows`, `dubious_ownership_message`, `one_sample_per_entry`, `wsl_notice_badge_and_watcher`, `trusting_a_refused_repository`, `trusting_a_repository_owned_by_another_user`; snapshots `ui_check_trust_and_wsl` (dark, light) checked.

