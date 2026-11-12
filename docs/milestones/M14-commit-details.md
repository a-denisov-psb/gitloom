# M14 – Commit details like GitKraken

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after M13. Last planned milestone before 1.0.0. Requested by the maintainer (2026-11-11, GitKraken 7.5.2 screenshots).

## Goal

The commit details panel looks and works like GitKraken's: the message in a framed box that can be edited by a click, a large author block with co-authors and clickable parents, and a file list with a summary, sorting, path or tree view and all files on demand. The graph shows relative time markers like GitKraken. Editing a message is safe: only commits that were not pushed can be edited, without a rebase and without touching the working tree.

## Scope

In scope:

- Details header and framed message box
- Author block: avatar, "authored <date>", co-authors from trailers, clickable parents
- File list: summary by kind, sort toggle, Path/Tree, "View all files", Expand/Collapse all
- Edit the message of unpushed commits on the checked-out branch (ADR-051)
- Relative time markers in the graph

Out of scope:

- Editing pushed commits (interactive rebase stays the explicit way, ADR-051)
- Custom commands (after 1.0.0, ROADMAP)

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Header | "commit: <short sha>" centered in the panel header like GitKraken; click copies the full sha (status bar confirmation). The icon at the right end follows the GitKraken verification item; until answered: none. |
| Message box | Summary (larger) and description in a framed, scrollable box (max. 40 % of the panel height), selectable text. Hover shows an edit cursor and the tooltip "Click to edit the message" when editing is allowed, otherwise the reason (see "Edit message"). |
| Author block | Avatar 40 px (M5 opt-in avatars; otherwise the initials tile in the author color), name, "authored <date>" in the date format of the graph; when the committer differs: second line "committed by <name> <date>". "parent: <short sha>" per parent at the right, each a link that selects and scrolls to that commit. Co-authors: `Co-authored-by: Name <mail>` trailers (case-insensitive, last paragraph) as 20 px avatars after "Co-authors:", tooltip name and e-mail; the trailers stay visible in the message. Signature status (M10-05) unchanged. |
| File list | Summary line with icon and count per kind: added, modified, deleted, renamed, copied, type changed (e.g. "2 added · 1 modified"), plural rules like the fix of 2026-11-11. Toolbar: sort toggle (A→Z / Z→A by path), "Path"/"Tree" segmented toggle, checkbox "View all files" (all files of the commit's tree, changed ones marked, unchanged muted), "Expand all"/"Collapse all" in Tree mode. Tree mode uses the left panel tree rules (folders collapse, M11-11 order: files first, then folders). Sort, mode and "View all files" are remembered globally. Applies to the commit details and the file list of a multi-commit selection; the staging lists keep their layout (verification item). |
| Edit message | Click on the message box opens an inline editor in the box: summary with the counter of the commit panel, description, "Save" (Ctrl+Enter) and "Cancel" (Esc). Allowed only for commits on the checked-out branch (HEAD or an ancestor) that are not contained in any remote-tracking ref (`git for-each-ref --contains <c> refs/remotes`) and while no Git operation runs. Otherwise not clickable, tooltip: "Already pushed to origin/<branch>. Use interactive rebase to change pushed history." / "Check out a branch that contains this commit" / "Finish the running operation first". Before saving, a note lists local branches and tags that contain the commit: "<name> keeps the old commit". Mechanism, signing and undo: ADR-051. Undo entry "Edit commit message". |
| Time markers | Like GitKraken: a small pill at the right edge of the commit message column on the first row whose relative date label differs from the row above: "just now", "<n> minutes ago", "<n> hours ago" (same day), "yesterday", "<n> days ago" (< 7), "<n> weeks ago" (< 5), "<n> months ago", "<n> years ago". Same date as the graph's date column; labels refresh every minute. Muted pill in all themes, AA contrast; hidden while the row is hovered if it covers the row's actions. |

## Maintainer answers and GitKraken verification items

Answered (GitKraken 7.5.2 screenshot, 2026-11-11): header "commit: 7c906e" with an icon at the right; framed message box; avatar with "authored 9.11.2026 @ 10:23"; "parent: 871066"; "Co-authors:" with avatars; "1 modified"; sort icon, Path/Tree, "View all files", "Expand All"; time pills "1 hour ago", "yesterday", "2 days ago" at the right edge of the graph rows.

Open:

| Item | Used in |
|---|---|
| Can GitKraken edit the message of older or pushed commits by a click? | M14-05 |
| What does the icon at the right of GitKraken's details header do? | M14-01 |
| Does GitKraken show the committer when it differs? | M14-02 |
| Exact buckets of GitKraken's time markers | M14-06 |
| Path/Tree and "View all files" also in GitKraken's staging lists? | M14-03 |

## Milestone acceptance

- [ ] Header, message box and author block look like GitKraken's; co-authors and parent links work.
- [ ] File list summary, sort, Path/Tree, "View all files" and expand/collapse work and are remembered.
- [ ] The message of an unpushed commit on the checked-out branch can be edited by a click, also with merges after it; the working tree and index stay untouched; undo and redo restore the branch exactly; pushed commits and commits of other branches are not editable and say why.
- [ ] Time markers appear like GitKraken's.
- [ ] User guide, UX-PARITY, `docs/user/undo.md`, release checklist and CHANGELOG updated; `./scripts/check.sh` green; CI green.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M14-01 | Details header and message box | – |
| M14-02 | Author block, co-authors, parent links | M14-01 |
| M14-03 | File list summary, sort, Path/Tree, all files | – |
| M14-04 | Message rewrite engine (ADR-051) | – |
| M14-05 | Edit message in the details | M14-01, M14-04 |
| M14-06 | Time markers in the graph | – |
| M14-07 | Documentation | M14-01 … M14-06 |

---

## Tasks

### [x] M14-01 Details header and message box

- Header and framed message box per the decisions table.

Acceptance: app test: click on the header copies the full sha; snapshots with a one-line and a long message in all three themes.

Note (M14-01): two themes since ADR-054 (Dark, Light). The message is shown in read-only text editors (iced has no selectable plain text): mouse and keyboard select, Ctrl+C copies, edits are ignored. The header button has the tooltip "Copy the full sha". The 40 % limit is measured with `responsive` on the panel's height.

### [x] M14-02 Author block, co-authors, parent links

- Author block, committer line, co-author parsing and avatars, parent links.

Acceptance: unit tests for trailer parsing (case, several co-authors, trailers not in the last paragraph ignored, malformed lines); app test: a parent link selects and scrolls to the parent; snapshots with avatars on and off, with co-authors and a merge commit (two parents).

Note (M14-02): `git_core::Commit` gained `committer` (name after the mailmap, empty when name and email equal the author's, so 100k commits allocate nothing extra). The e-mail moved from the author line into the name's tooltip. The initials tile keeps the theme's avatar color (no per-author color exists elsewhere in Gitloom). A parent beyond the loaded history is a disabled link with the tooltip "Not in the loaded history".

### [x] M14-03 File list summary, sort, Path/Tree, all files

- Summary, toolbar, tree mode, "View all files", persistence.

Acceptance: unit tests for the summary (every kind, plural) and the tree order; app tests for sort, mode switch, expand/collapse all, "View all files" (unchanged files muted, a click shows the file's content), persistence across restart; snapshots in Path and Tree mode.

Note (M14-03): Git's copy and type-change kinds are reported by git-core as added and modified, so the summary has no "copied" or "type changed" entries. A click on an unchanged file opens the blame view at the commit, which shows the file's content (no extra content view). The sort toggle is a text button "A→Z" / "Z→A" (no sort icon in the icon set). The multi-commit selection has no file list today, so there is nothing to apply the options to; the stash file list keeps its Staged / Unstaged / Untracked sections. Folder collapse state lives for the session (only sort, mode and "View all files" are stored).

### [x] M14-04 Message rewrite engine (ADR-051)

- git-core `reword_unpushed(branch, commit, message)` per ADR-051, including eligibility checks (`EditBlock` reason: pushed with the remote branch name, not on the checked-out branch, operation running) and the list of local branches and tags that keep the old commit.

Acceptance: integration tests: HEAD; a commit three back on a linear branch; a commit before a merge (the merge is rebuilt with its second parent unchanged, no conflict); staged and unstaged changes and untracked files untouched (byte compare, `git ls-files -s`); author name, e-mail and date preserved; committer updated; signed rebuild with an SSH signing key when `commit.gpgSign` is true; refused when pushed, on another branch, or during a merge; ref update refused when the branch moved meanwhile; undo and redo restore the exact branch oid.

Note (M14-04): `git_core::reword` (`edit_block`, `edit_facts`, `reword_unpushed`, `Reworded::apply` for undo/redo). The author is passed to `git commit-tree` through `GIT_AUTHOR_*` with the new `GitWriter::run_with_env`; the encoding header is kept for rebuilt descendants and dropped for the target (its new message is UTF-8). "The branch moved meanwhile" is guarded by `update-ref <new> <old>` and tested through undo after a new commit. Tests: `crates/git-core/tests/reword.rs`.

### [x] M14-05 Edit message in the details

- Inline editor, disabled states with tooltips, note about other refs, undo entry.

Acceptance: app tests: click opens the editor only when allowed; Ctrl+Enter saves, Esc cancels; tooltips for each blocked reason; the note lists other refs; Ctrl+Z restores; the graph selects the rewritten commit after saving; snapshots of the editor in both themes.

Note (M14-05): An editable message is shown as plain text so a click opens the editor (text selection is available in the editor); blocked messages stay selectable and carry the reason as tooltip. Enter in the summary also saves (like Ctrl+Enter). A write block (read-only, Git missing) and an operation started after the check also block, with their own reason. After saving the rewritten commit is selected; Ctrl+Z moves the branch back. Stashes cannot be edited ("Stashes cannot be edited").

### [x] M14-06 Time markers in the graph

- Bucket function and pill drawing per the decisions table.

Acceptance: unit tests for the buckets around every boundary (with a fixed clock); snapshot of a graph spanning minutes to years in all three themes; performance unchanged on the 100k-commit benchmark (`docs/PERFORMANCE.md`).

Note (M14-06): Buckets count calendar days in UTC like the date column ("yesterday" starts at midnight; 7–34 days are weeks, 35–364 days months as days/30, then years as days/365). The pill uses the hover background with the normal text color: muted text on it measured 4.1:1 (below WCAG AA). Two themes since ADR-054. Commit rows have no hover actions over the message, so the pill is never hidden. Measured in `docs/PERFORMANCE.md`.

### [x] M14-07 Documentation

- User guide `daily-workflow.md` (commit details, edit message), `coming-from-gitkraken.md`; `docs/user/undo.md`: "Edit commit message" (and why pushed commits are excluded).
- UX-PARITY rows; deviation if GitKraken edits pushed commits.
- `docs/RELEASE-CHECKLIST.md` section "7h. New in M14" (edit the message of an unpushed commit before a merge, check that pushed commits are blocked, co-authors, Path/Tree, time markers).
- CHANGELOG `[Unreleased]`.

Acceptance: docs reviewed against the implemented behavior.

## After M14

All planned milestones are done. Do not ask for planning. Instead:

1. `docs/ROADMAP.md`: M14 done; "Current milestone" → "Final check and release 1.0.0 (M6-13, M6-15; ADR-042)".
2. `docs/HANDOVER.md`: current state; the maintainer to-do "Final check before 1.0.0" stays open and lists all checklist sections 7 … 7h.
3. `docs/PLANNING-HANDOVER.md`: milestone history lines for M9 … M14 (version "1.0.0 (unreleased)"), section 8 "Next: final check by the maintainer, then 1.0.0".
4. Stop with the milestone report and the line "Bereit für den finalen Test (Checkliste Abschnitte 7 … 7h); 1.0.0 auf Zuruf."
