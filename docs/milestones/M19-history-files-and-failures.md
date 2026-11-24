# M19 – History files and visible failures

Target version: **1.0.0** (no release of its own, ADR-042)

Runs on `develop` after M18. Requested by the maintainer (2026-11-24), findings #53 and #54 (planning update 15). Last milestone before the final check.

## Goal

A file that belongs to a commit opens in the external editor in the state of that commit, whichever branch is checked out. And no action fails silently: whatever cannot run says so and why, where the user looks.

## Scope

In scope:

- Reading a file at a commit and writing it as a read-only copy in the temp folder (ADR-061)
- "Open in external editor" for the files of a commit (commit details) and for the entries of the file history
- "Open file location" for files of a commit that are not in the working tree
- Failures of programs Gitloom starts (editor, terminal, file manager, links, theme folder, crash report folder) shown as toasts instead of status bar text
- An audit of the other places where an action is refused or fails without a visible message (ADR-044 amendment)
- Documentation, the undo list, checklist section 7r

Out of scope:

- Editing the copy back into the repository, an in-app file viewer
- New editor entries elsewhere (blame lines, the palette file list)
- Detecting that a started program fails later (Gitloom only knows that the process started)
- External diff and merge tools (they report failures in dialogs, ADR-055)
- Rewriting the Git error toasts and their catalog (ADR-049 stays as it is)

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Source | Finding #53 (maintainer, 2026-11-23): two feature branches that are not merged, one adds `README.md`. On the branch that was not checked out, a click on its commit and "Open in external editor" did nothing and said nothing: the editor was given the working tree path, the file was not there, and the error text only reached the status bar. If the file does exist in the working tree, the editor shows today's content instead of the commit's. Finding #54: whatever does not work must tell the user that and why. |
| Which state opens | Staging area (Unstaged, Staged): the working tree file, as today. A file of a commit (commit details): the file as it is in that commit. A file history entry: the file as it is in the commit of that entry, under the path it has there (renames). |
| Copy instead of content | Editors take a path, not content. Standard input works only in a few (`code -`) and carries no file name, so there would be no syntax colors. Gitloom therefore writes the content to a copy and gives the editor its path as `{path}`; this works with every preset and every custom command. |
| Copy location | `<temp>/gitloom-view/<commit id, 12 characters>/<repo path>`. File name and folders are kept (syntax highlighting, special names such as `Makefile`, no clash between two versions). Written through one module; the directory is created on demand. |
| Content | Read as a checkout would write it (line endings and other filters, for example `core.autocrlf`), any size, bytes untouched. Binary files are written as they are; what the editor does with them is up to the editor. Read through the backend (`git cat-file`), not from the working tree. |
| Same file again | The copy of one commit and path never changes. If the file exists with identical content, it is reused; otherwise it is rewritten (the read-only flag is cleared for that, then set again). |
| Read-only | The copy is read-only so that nobody believes edits reach the repository; the editor shows a lock or asks before saving. |
| HEAD exception | If the commit is the checked-out commit (HEAD) and the working tree file has exactly its content, the real file opens (editable, no copy, no toast). If the working tree file is modified, or the commit is any other, a copy opens. Older commits always get a copy, even when the content is the same: a copy says "this is history". |
| Deleted in the commit | The entry stays enabled; the copy holds the file as it was in the first parent. The toast says so. |
| File history | The entry's commit and the entry's own path (`FileHistoryEntry::path`, not the history's current path). A deletion entry (`D`) opens the version in the first parent. A merge entry without diff output opens the file at that commit if it exists there, otherwise the warning "The file is not in commit <sha7>". |
| Not possible | Submodule entries (a commit id, no content) and folders: the entry is disabled with the reason "A submodule has no file content". A path that cannot be written on this system (reserved names, characters Windows does not allow, `..` or an absolute path in a tree): warning toast with the reason, nothing is written outside the copy folder. |
| Announcement | A copy shows an info toast: title "Opened a read-only copy", text "<path> as of <sha7>. Changes are not saved to the repository." (deleted file: "... as it was before <sha7> deleted it."). The real file shows nothing. The menu label stays "Open in external editor" (GitKraken parity of the menu). |
| Cleanup | At start-up a background task deletes copies older than 24 hours in `gitloom-view` (best effort: errors and files in use are ignored; the read-only flag is cleared before deleting on Windows). |
| No decision on the UI thread | Whether the working tree file exists or equals the commit's file is decided in the background task that opens the editor, not when the menu is built. The entry stays enabled. |
| Rule for failures | ADR-044 amendment: an action that cannot run says so and why, at once and where the user looks. Before: the entry or button is disabled and its tooltip names the reason. After: if it still fails, a toast with a plain title, the reason and, if there is one, a button for the way forward. Never only the status bar, a log or nothing. |
| Toast kinds | Error: tried and failed (stays until closed). Warning: refused for a reason with a way forward (stays until closed). Info: harmless notices (6 seconds). A harmless no-op ("Already on develop") may stay in the status bar; the audit decides and writes the classification down. The status bar keeps confirmations of successful actions and passive information. |
| No stacking | Pushing a toast that is identical to a visible one (same kind, title and text) replaces it instead of stacking (repeated Alt+E). |
| Editor missing | The file menu entries stay disabled with the reason as today. Alt+E, the toolbar button and the palette show a warning toast "No external editor set" with the button "Open Preferences" (opens Preferences → External tools, like the palette action "Set: External editor ›"). |
| Open file location | For a file that is not in the working tree the nearest existing folder still opens (as today), and an info toast says "<path> is not in the working tree. Showing its folder." |
| GitKraken | Gitloom extension; no GitKraken check needed. Where GitKraken has no such behavior, Gitloom goes further. |

## Maintainer answers and GitKraken verification items

Answered (2026-11-24): files of history open in the state of their commit; the content goes through a file, not standard input; HEAD rule, deleted files and the info toast as in the table; the general rule "if something does not work, tell the user and why" (finding #54); no GitKraken check needed.

Open: none. For the maintainer's check: checklist section 7r.

## Milestone acceptance

- [ ] "Open in external editor" on a file of a commit opens that commit's version, also for a commit on a branch that is not checked out and for a file that is not in the working tree (finding #53); the editor shows a read-only copy and an info toast names it.
- [ ] For a file that exists in the working tree with other content, a commit older than HEAD opens the old content, not the current one.
- [ ] HEAD with an unchanged file opens the real file; HEAD with a modified file opens a copy.
- [ ] A file deleted in a commit opens its version before the deletion; a file history entry opens the version of that entry, also after a rename.
- [ ] The Unstaged and Staged lists still open the working tree file.
- [ ] No program that Gitloom starts can fail without a visible message: editor, terminal, file manager, links, theme folder, crash report folder, "Open file location" (toasts with plain text).
- [ ] Every disabled menu entry in the audited menus has a reason tooltip; refused and failed actions found by the audit show a toast or a reason (list in the M19-03 note).
- [ ] ADR-061 and the ADR-044 amendment are in `docs/DECISIONS.md`; user guide, undo list ("Cannot be undone"), UX-PARITY, release checklist (section 7r) and CHANGELOG updated; `./scripts/check.sh` green; CI green.

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M19-01 | Snapshot engine: read a file at a commit, write the copy, clean up | M18-06 |
| M19-02 | Entry points: commit files, file history, "Open file location" | M19-01 |
| M19-03 | Failures are visible: launch errors as toasts, audit of silent refusals | M19-02 |
| M19-04 | Documentation and close | M19-01 … M19-03 |

---

## Tasks

### [x] M19-01 Snapshot engine

Note: `git_core::file_at_commit` uses `git ls-tree` (literal pathspec, no object needed: submodules and folders are told apart) plus `cat-file --filters`; `commit_id` resolves the first parent. App module `history_copy` (base folder name `identity::VIEW_DIR`); copies of a deleted file are keyed by the parent's id, whose content they hold. The cleanup also removes emptied folders.

- git-core: a function that reads a file at a commit through the backend (`git cat-file --filters <rev>:<path>`, like `blob_for_worktree` does for the working tree), result "not in this commit" distinct from an error; no size limit (not the diff limit); runs off the UI thread.
- App side (no process start in this module, ADR-053): a pure function that builds the copy path `<temp>/gitloom-view/<commit 12>/<repo path>` and rejects unsafe paths (`..`, absolute paths, drive prefixes, empty parts; on Windows reserved names and forbidden characters) with a readable reason; a function that writes the copy (create folders, reuse identical content, rewrite otherwise, set read-only); a cleanup function (delete copies older than a given age, ignore errors, clear the read-only flag first). The base folder comes from one place so tests can point it elsewhere.
- Start-up task: call the cleanup with 24 hours; it never delays the first frame.

Acceptance: unit tests: path building keeps the file name and nested folders and uses the commit id; `../x`, an absolute path and an empty part are rejected and nothing is written outside the base folder; the same commit and path twice reuses the file; changed bytes rewrite even when the first copy is read-only; the written file is read-only; cleanup deletes an old copy and keeps a new one and survives a file it cannot delete. Backend tests against a fixture repository: a file of a commit on a branch that is not checked out and does not exist in the working tree (finding #53); a file whose content differs from the working tree; a file stored with LF in a repository with `core.autocrlf=true` is written with CRLF; a binary file is byte-identical; a file larger than the diff limit works; a missing path gives "not in this commit"; a deleted file is read from the first parent.

### [ ] M19-02 Entry points

- One path for "open in external editor": a source (working tree file, or file of a commit with the commit id, the path and the way to the parent) is resolved in the background task: working tree files as today; commit files by the rules of the decision table (HEAD exception, deleted in the commit, copy, toast). The menu entries pass the source; `Alt+E` and the toolbar keep opening the repository root.
- Commit details: the file menu of the Commit area passes the commit and path; the "deleted" check for the Commit area no longer disables the entry; submodule entries and folders are disabled with the reason. Unstaged and Staged lists unchanged.
- File history: the entry menu passes the entry's commit and its own path (deletion entries: the first parent); the hard-coded working tree path and the `false` for "deleted" are gone.
- "Open file location" for files of a commit that are not in the working tree: info toast as in the table, the nearest folder still opens.
- Info toast "Opened a read-only copy" for copies only.

Acceptance: app tests (with a fixture repository or fake backend, a temp base folder for the copies): Unstaged and Staged open the working tree path; Commit area with a commit older than HEAD opens a copy path under the base folder whose last component is the file name; a commit on a branch that is not checked out, file missing in the working tree, opens the copy (finding #53); HEAD with an identical working tree file opens the real file and shows no toast; HEAD with a modified file opens a copy; deleted in the commit opens the first parent's version and the toast says so; added, renamed (new path) and a file in a folder work; a submodule entry is disabled with its reason; no editor set keeps the entries disabled with the existing reason; the editor receives the copy path as `{path}` (`launch::editor` unchanged); the copy is read-only; the file history entry opens the version of its commit under its own path, a deletion entry the first parent's version, a merge entry without the file shows the warning "The file is not in commit <sha7>". "Open file location" for a missing file gives the info toast and still reveals the nearest folder. No file I/O on the UI thread: menus are built without touching the disk.

### [ ] M19-03 Failures are visible

- One message for the result of every program launch (`LaunchFinished` or similar, carrying what was started) replaces the six `Message::StatusText(result.err()…)` closures (`rg "StatusText\(result.err"`): editor, file manager, "Open file location", links, theme folder, crash report folder, and the terminal if it has its own path. Failure: Error toast "Could not start <the editor / the terminal / the file manager / the link>", plain reason, and where possible the way forward ("Check the command in Preferences → External tools."), the raw error under "Details".
- Editor missing for Alt+E, the toolbar button and the palette: warning toast "No external editor set" with the button "Open Preferences" (External tools); the same pattern for a missing terminal where the setting is empty and nothing can be found.
- `Toasts::push`: an identical visible toast is replaced, not stacked.
- Audit of silent or status-bar-only refusals and failures. Walk through the roughly 55 `self.status = …` assignments in `app.rs` (and the same pattern in other files), all `Task::none()` early returns of user-started messages, and `action_block` results. Classify each: confirmation (stays), harmless no-op (may stay), refusal (warning toast, with a way forward where one exists), failure (error toast). Convert the refusals and failures; the classification list goes into the task note (one line each).
- Menu check: every disabled `MenuEntry::Item` of the context menus (file, commit, branch, tag, remote, stash, submodule, GITFLOW rows) has a non-empty reason tooltip; add a test that builds these menus in their disabled states and fails for an entry without a hint; fix what it finds.
- Open points that need design (not a quick fix) go into `docs/HANDOVER.md` "Open questions" instead of being guessed.

Acceptance: app tests: a failing launch for each of the six paths (the program does not exist) shows an Error toast and leaves the status bar empty; Alt+E, the toolbar button and the palette without an editor show the warning toast with a button that opens Preferences on External tools; two identical toasts show one; the converted refusals from the audit show a toast (a test per group, not per line); the menu test above is green. The task note lists the classification of every `self.status` site.

### [ ] M19-04 Documentation and close

- `docs/DECISIONS.md`: implementation notes for ADR-061 and the ADR-044 amendment (copy folder, cleanup, rules found in the audit, toast kinds in use).
- `docs/user/daily-workflow.md` or `docs/user/settings.md` (where the external editor is explained): files of a commit open as read-only copies, where they are, the HEAD exception, deleted files; `docs/user/troubleshooting.md`: "Nothing happens when I open a file" is now a message, and what each message means. `docs/user/undo.md` → "Cannot be undone": a row for opening a file of a commit in the external editor (nothing in your repository changes; the copy is deleted after a day).
- `docs/UX-PARITY.md`: extend the "External editor" rows (files of a commit open as copies, Gitloom extension, no GitKraken check needed) and add the toast rule to the notes on failures.
- `CHANGELOG.md` `[Unreleased]`: a paragraph for the editor behavior and one for visible failures.
- `docs/RELEASE-CHECKLIST.md`: new section "7r. History files and visible failures (M19) (Windows required)" with the items below; section 8 gets no new item.
- `./scripts/check.sh` green.
- Then the steps of "After M19".

Checklist 7r items:

- [ ] Two branches that are not merged, one adds `README.md`: check out the other one, click the commit of the first, "Open in external editor" on `README.md`: the editor opens a read-only copy with the commit's content; the info toast names it.
- [ ] A file that exists today with other content: click an old commit that changed it, open it in the editor: the old content shows, not the current one.
- [ ] Click the HEAD commit and open an unchanged file: the real file opens and can be edited. Change that file without saving to the index and open it from HEAD again: the committed version opens as a read-only copy.
- [ ] A commit that deleted a file: the entry is enabled and opens the version before the deletion.
- [ ] File history: select an older entry and open it in the editor: that version opens; for a renamed file the name of that time.
- [ ] A file with CRLF line endings (Windows, `core.autocrlf=true`): the copy has the same line endings as your working tree file.
- [ ] The Unstaged and Staged lists still open the working tree file.
- [ ] The copies are in `%TEMP%\gitloom-view\<commit>\…`; after a restart, copies older than a day are gone.
- [ ] No external editor set: Alt+E, the toolbar button and the palette show "No external editor set" with "Open Preferences"; the file menu entries are disabled with the reason.
- [ ] A custom editor command with a program that does not exist: "Could not start the editor" with the reason; nothing fails silently. The same for a custom terminal command that does not exist.
- [ ] "Open file location" on a file of a commit that is not in the working tree: a toast says so and the nearest folder opens.
- [ ] Disabled menu entries (for example in a file menu, a branch menu) show a reason as tooltip.

## After M19

All planned milestones are done. Do not ask for planning. Instead:

1. `docs/ROADMAP.md`: M19 done (table and section); "Current milestone" → "Final check and release 1.0.0 (M6-13, M6-15; ADR-042)" with the checklist sections 7 … 7r; in the following paragraph "after M19".
2. `docs/HANDOVER.md`: current state; the maintainer to-do "Final check before 1.0.0" lists all checklist sections 7 … 7r and the open GitKraken verification items (M19 has none).
3. `docs/PLANNING-HANDOVER.md`: milestone history line for M19 (version "1.0.0 (unreleased)"), section 8 "Next: final check by the maintainer, then 1.0.0".
4. Stop with the milestone report and the line "Bereit für den finalen Test (Checkliste Abschnitte 7 … 7r); 1.0.0 auf Zuruf."
