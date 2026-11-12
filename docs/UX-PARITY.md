# UX parity with GitKraken

Reference for layout, interactions and shortcuts. A GitKraken user must feel at home without relearning. Colors and icons are our own.

Shortcut source: GitKraken Desktop keyboard shortcut documentation (help.gitkraken.com, state April 2026). Verify behavior in a real GitKraken installation when in doubt.

Status values: `done`, `M<n>` (planned milestone), `—` (not planned).

## Layout

| Area | GitKraken behavior | Status |
|---|---|---|
| Repo tabs | Top row, one tab per repo, `+` opens a new tab with the start screen | done (M1) |
| Start screen (New Tab) | Open a repo, Clone a repo, Start a local repo (also hosted); Recent Repos; Customize; Additional Resources | done (M10-01) without hosted repositories and boards (non-goals); Resources open the user guide, the new-issue page and the changelog on GitHub |
| Clone | New Tab → Clone a repo: URL, where to clone to, full path; progress | done (M10-02): name derived from the URL (editable), "Clone submodules" (on), progress and Cancel inside the dialog, askpass like fetch, opens in the tab; not undoable (ADR-043, new folder) |
| Start a local repo | New Tab → Start a local repo: name, folder, initial branch, README, `.gitignore`, license | done (M10-03): no license templates (backlog); `.gitignore` templates bundled from github/gitignore; "Initial commit" when an identity is set, otherwise the files stay uncommitted with a notice; not undoable (ADR-043) |
| Default branch name | Preferences → General → Default Branch Name | done (M10-03): `init.defaultBranch` in the global Git config; empty removes it |
| Signing | Preferences → GPG: Signing Key, GPG Program, Sign Commits/Tags by Default, Generate | done (M10-04) as Preferences → Signing with GPG or SSH format (SSH signing is a Gitloom extension), global Git config (ADR-046), repository overrides shown read-only; no key generator (backlog) |
| Signature status | Commit details mark signed commits | done (M10-05): badge after the author line from `%G?`, key id as tooltip; an SSH signature without `gpg.ssh.allowedSignersFile` reads "Signed (key not available)"; GitKraken's exact look is a verification item |
| Commit template | Repo-specific "Commit Template": Apply, Remove comments, Summary, Description, Save | done (M10-06) as Preferences → Repository → Commit; stored in Git's `commit.template` (a shared global file only after a confirmation), the options in the repository config; both options default to on when unset (Gitloom prefilled the template before M10) |
| Commit message prompt | – | Gitloom extension (M10-07): "Copy commit message prompt…" in the Staged Files header and staged file menus, the palette and (M12-13) a sparkle button right of "Committing as …" like VS Code's, a muted icon like the left panel's icons that looks the same while disabled (deliberate exception to "disabled = dimmed", maintainer request 2026-11-12; the tooltip gives the reason); instruction on the global Preferences → Templates page (deviation: GitKraken has no such page; the commit template stays per repository) |
| Stash message template | – | Gitloom extension (M10-10): Preferences → Templates → "Stash message" (presets, custom, preview); used when a stash would otherwise get Git's default message |
| External editor per repository | – (GitKraken: one global "External Editor") | Gitloom extension (M10-11): Preferences → Repository → External editor, stored in the repository's Git config; all entry points use it, the toolbar button shows with either editor |
| Auto-Prune, Delete ".orig" files, Longpaths, toolbar icon labels | Preferences → General / UI Customization | done (M10-08): "Prune remote branches on fetch" (General, on), "Support long paths" (General, Windows only, global `core.longpaths`), "Delete .orig files after merging" (External tools, off), "Show toolbar labels" (Appearance, on) |
| Toolbar | Repo + branch pickers left; Undo/Redo, Pull/Push, Branch, Stash/Pop centered; Terminal right | done (M2–M4 actions; M5-13 Terminal; M5-02 a Settings gear next to it); M6-17 pickers open the palette lists (see deviations); M8 external editor button next to Terminal |
| Panel toggles | – (GitKraken: shortcuts and the View menu) | Gitloom extension (M11-14; fix 2026-11-12, maintainer decision): panel toggles in the status bar like Zed (left panel at the left end, commit panel at the right end), state shown by filled icons like VS Code (no background); no tooltip (it covered the button; maintainer request 2026-11-12), the shortcuts are listed in the command palette; same as Ctrl+J / Ctrl+K and the palette |
| Remote website | "Open on GitHub" etc. (with an integration) | done (M12-03) without an account: derived from the remote URL; GitHub, GitLab, Bitbucket and "Open in browser" for other hosts |
| Left panel | Gitflow (when initialized), Local, Remote (grouped by remote), Stashes, Tags, Submodules (GitKraken also has Issues, Boards and GitHub Actions: non-goals); collapsible; filter on top; counts; branches grouped into folders at `/` with icons; checked-out branch highlighted; "Viewing x/y · Show All" when branches are hidden; GITFLOW is a tree of the development and production branches and the prefixed branches, the prefix as a folder; checked-out branch with a check and its branch icon | done (M0), real data M1; folders, icons, highlight and counter M7-14; order and GITFLOW tree M9-06; since M11-11 every tree (LOCAL, REMOTE, GITFLOW, TAGS) lists branches or tags before folders, TAGS has folders, the check sits left of the branch icon (no folder menu); SUBMODULES always shown with its count since M12-06; STASHES is a tree at `/` since M12-10 (stashes in stash order, then folders; "WIP on <branch>" not split) |
| Submodules | SUBMODULES section with status icons; context menu Update, Open; double click opens the submodule | done (M12-06): states up to date / not initialized / out of date / modified / conflict with tooltips, nested ones indented; Update (`--init --recursive`, network progress, Cancel, askpass), Open in new tab, Open in file manager, Copy path; header menu "Update all submodules"; info toast "Submodules are out of date" with "Update" after HEAD moved; adding/removing submodules is out of scope (Git CLI) |
| Graph | Columns: branch/tag, graph, commit message (+ author, date, SHA); WIP row on top; resizable columns and a column menu | done (M1); straight lines with rounded 90° corners and dotted WIP/stash edges since M7-18; resizable columns (drag a header border, double click for the optimal width, widths kept globally) since M11-02, every column with its own width and horizontal scrolling since M11-16; gear menu at the right end of the header (Author, Date, SHA, Reset column widths) and the same options in Preferences → Appearance since M11-03; no automatic hiding; Date and SHA have a fixed width fitted to their content (fix 2026-11-12) |
| Classic graph look | GitKraken 7.x: rows tinted in the lane color, filled ref pills joined to their node by a line, header band with separators, panel shadows | not offered: built in M11-04 as an extra theme, removed again by maintainer decision (ADR-054); Gitloom has Dark and Light |
| Preferences → Editor | Font, Font Size, Tab Size, End of Line Character, Syntax Highlighting, Show Line Numbers, Word Wrap | done (M11-06): Font (installed monospace fonts, Default), Font size 8–24, Tab size 1–8, Show line numbers for diffs, blame, file history and the conflict view; Word wrap in all diff modes since M11-07; Syntax highlighting in all diff modes, blame, file history and the conflict view since M11-09; no End of Line Character (no in-app editor) |
| Right panel | Commit details or staging area (WIP selected) | done (M0), real M1 |
| Diff | Replaces the graph in the center; close returns to graph | done (M1, hunk view); inline and split M7 (ADR-038) |
| Status bar | Bottom line | done |
| Resizable panes | All splits draggable | done |

## Interactions

| Interaction | Behavior | Status |
|---|---|---|
| Click commit | Select, show details right | done |
| Click WIP | Staging area right | done |
| Click file in panel | Diff in center | done (M1) |
| Click branch in left panel | Jump to and select its commit | done |
| Double click local branch | Checkout | done (M3; sidebar and graph pill) |
| Double click remote branch | Create tracking branch and checkout | done (M3; an existing local branch with that name is checked out instead) |
| Right click commit/branch | Context menu | done (M3; graph rows, ref pills, sidebar; M4 actions disabled with "Not available yet"); M13-06 branch and remote branch menus ordered like GitKraken, labels name the branch, plus Create branch here, Reset `<head>` to this commit…, Copy commit sha |
| Branch context menu: merge and rebase | "Merge X into Y", "Rebase Y onto X"; "Fast-forward Y to X" when possible; remote branch: "Fast-forward L to origin/L" for a local branch behind it | done (M4-04, M4-06) |
| Drag branch onto branch | Menu: Fast-forward, Merge, Rebase, Interactive rebase | done (M4-11): graph pills and sidebar branches, drop target outlined; items that need another checked-out branch are disabled with "Check out <branch> first" (verify in GitKraken: disabled or automatic checkout?) |
| Toolbar Pull dropdown | Fetch all, Pull (ff if possible), Pull (ff only), Pull (rebase); last choice becomes default | done (M3) |
| Push without upstream | Dialog to choose remote and branch name, sets upstream | done (M3) |
| Push rejected | Offer Pull or Force push (with lease) | done (M3; force push asks once more) |
| Branch button / Ctrl+B | Inline name input at the selected commit; creates and checks out | done (M3; also "Create branch here" in the commit menu; M13-06 also in branch, remote branch and tag menus, with a dialog when the commit has no row) |
| Auto-fetch | Periodic fetch in the background | done (M3, every 5 min; interval in Preferences → General since M5-02) |
| Hover file in staging | Stage/Unstage and Discard buttons appear | done (M2); since M12-01 only Stage/Unstage (deviation below) |
| Right click file in staging | Context menu: Stage/Unstage, Discard, Ignore, Open file location | done (M2, plus Copy file path); M8 "Open in external editor"; M13-07 GitKraken order, File blame, Open in external diff tool, Discard changes for staged files, same menu (without staging entries) for files of a commit incl. Open file location |
| Stage hunk / lines in the diff view | "Stage hunk" per hunk; click, Shift+click, Ctrl+click select lines, "Stage selected lines" | done (M2) |
| Diff view mode | Hunk / Inline / Split toggle in the diff header; arrows jump between changes | done (M7: one global setting; staging by line in all modes and by block in Inline/Split; read-only diffs have no bars; GitKraken checks on remembering, block buttons and Split alignment open) |
| Gitflow | Preferences → Gitflow initializes; GITFLOW section in the left panel with a green start button; branch menu "Finish …" | done (M7: native, git-flow compatible config, ADR-037; also palette entries; finish across conflicts and one undo step; GitKraken checks on dialogs and section position open) |
| Commit identity | Profiles (Preferences → Profiles) hold author name and email; the active profile commits | done (M8-03, M8-04): Preferences → Identities; per repository "Committing as …" in the commit panel (menu), the palette ("Choose identity…") and Preferences → Repository → Identity, written to the repository config (see deviations) |
| Repo-specific preferences | Group of categories for the open repository (Encoding, Gitflow, Git Hooks, LFS, Commit, Submodules, …) | M8: group "Repository · <name>" with Identity, SSH key, Gitflow; done (M8-01, M8-04, M8-06): group with Identity, SSH key and Gitflow |
| SSH key | Preferences → SSH: one key pair or the local SSH agent | done (M8-05, M8-06): per repository via `core.sshCommand` in Preferences → Repository → SSH key, system default otherwise (see deviations) |
| External editor | Preferences → External tools: VS Code, Sublime, IntelliJ or a custom path | done (M8-07, M8-08): presets fill a command template with `{path}`; repository via palette, Alt+E and toolbar; files via their context menus (unstaged, staged, commit files, file history) |
| Stage file | Selection moves to the next unstaged file and shows its diff | done (M2; unstaging moves within the staged list the same way) |
| Stash (toolbar) | Stashes all changes including untracked files | done (M2) |
| Stash in the graph | Dashed square with a stash icon, labelled "WIP on <branch>" (or the message given) | done (M11-05): same label in STASHES and the details title; a stash without a message gets Git's default message |
| Stash in graph | Stash shown as node attached to its base commit | done (M2, square node, dashed edge) |
| Right click stash | Apply, Pop, Delete | done (M2, sidebar and graph) |
| Right click remote | Fetch, Edit URL, Rename, Remove | done (M3); M6-18 remotes defined outside the repository config cannot be removed, renamed or edited |
| "+" in the remote section | Add a remote (name, URL) | done (M3) |
| Right click tag | Push, merge/rebase entries, Create branch here, Reset to this commit, Delete locally / from remote, Copy tag name / commit sha | done (M3, sidebar and graph pill; M13-06 GitKraken order and labels) |
| Create tag here (commit menu) | Name and optional message (annotated if a message is given) | done (M3) |
| Amend | "Amend previous commit" checkbox fills message from HEAD | done (M2) |
| Merge in progress | Orange graph row "A file conflict was found when attempting to merge into develop", details header "1 file conflict in working directory · View conflict", panel "Merge conflicts detected – Merging X into Y" with Conflicted Files, Resolved Files, "Mark all resolved", "Commit and Merge", "Abort Merge" | done (M2-09, M2-13, M4-03; like GitKraken since M12-05, banner removed); also rebase, cherry-pick, revert ("Continue rebase", "Abort rebase", Skip for rebase and cherry-pick); "Commit and merge" is disabled while conflicts are unresolved (tooltip lists the files); "Mark all resolved" names files that still contain conflict markers and asks first; the graph row stays above the commits while scrolling |
| Reset | Commit menu "Reset <branch> to this commit" → Soft / Mixed / Hard | done (M4-10) as a dialog with three buttons instead of GitKraken's submenu (see deviations); Hard is always confirmed and lists the files whose changes are lost; also on the HEAD commit since M9-04 (Soft disabled there, Mixed/Hard clear conflicts, undoable) |
| Stash pop/apply with conflicts | Conflicted files, the stash is kept | done (M9-01, M9-02; M12-05: graph row "A file conflict was found when attempting to apply stash '<name>'", panel "Applying stash '<name>'") with Undo and, once resolved, Drop stash / Keep stash below the commit button; conflict view sides "Updated upstream" / "Stashed changes"; Undo returns to the exact state before (extension, ADR-043) |
| Discard conflicted file | Context menu Discard on a conflicted file | done (M9-03): back to HEAD in index and working tree after a confirmation; undoable |
| Commit summary counter | Remaining characters inside the summary field, right aligned | done (M9-07): muted, warning color at 10 or fewer, error color below 0 (commit not blocked) |
| Cherry-pick and revert | Commit menu "Cherry-pick commit" / "Revert commit"; multi-selection "Cherry-pick N commits" (oldest first) | done (M4-09); a merge commit asks for the parent to revert against; an empty result opens "Skip" / "Commit empty" (verify wording in GitKraken) |
| Conflicted files | "Conflicted Files" section above "Unstaged Files" | done (M4-03) |
| Local + remote branch on same commit | One combined pill | done (M1; since M5-05 a cloud icon, the remote names in its tooltip) |
| Commit node | Avatar (placeholder: initials) | done (M5-11; Gravatar opt-in in Preferences → Appearance, initials otherwise) |
| Conflicted file | Opens conflict view (ours, theirs, output) | done (M4-05); with both sides picked in one conflict, ours lines come first (GitKraken keeps the click order); M12-04: side headers "Commit <short sha> on <branch>" (A = HEAD side, B = incoming), "conflict n of N" with ↑/↓ and Alt+↑/↓, wrapping at the ends |
| Interactive rebase | Panel with actions per commit, reorder by drag | done (M4-07): commit menu "Interactive rebase from here"; newest on top until the maintainer verifies GitKraken's order; P/R/S/F/D, ↑/↓, Alt+↑/↓ reorder; inline message editors for rewords and squash chains |
| Multi-select commits | Ctrl+click, Shift+click | done (M4-01; also Shift+↑/↓; panel "N commits selected") |
| Hide / solo branches | Eye on hover in the left panel hides a branch; context menu Hide/Show, Solo; remote: Hide all / Show all | done (M5-08; the checked-out branch cannot be hidden until verified in GitKraken; stored per repository) |

## Keyboard shortcuts

Windows/Linux shown; macOS uses ⌘ instead of Ctrl (and Option instead of Alt).

### Repo actions

| Action | Shortcut | Status |
|---|---|---|
| Create branch | Ctrl+B | done (M3) |
| Fetch all | Ctrl+L | done (M3) |
| Stage current file | S | done (M2) |
| Stage all files | Ctrl+Shift+S | done (M2) |
| Unstage current file | U | done (M2) |
| Unstage all files | Ctrl+Shift+U | done (M2) |
| Commit staged files | Ctrl+Enter | done (M2) |
| Stage all and commit | Ctrl+Shift+Enter | done (M2) |
| Focus commit message | Ctrl+Shift+M | done (M2) |

### Navigation

| Action | Shortcut | Status |
|---|---|---|
| Next / previous item | ↓ / J next (older), ↑ / K previous (newer) | done (M1) |
| Context menu of the selected commit | Shift+F10 / Menu key; ↑/↓ choose, Enter runs, Esc closes | done (M6-07) |
| Next / previous input field | Tab / Shift+Tab | done (M6-07) |
| Item right / left | → / L, ← / H | — in M1 (no horizontal items in the read-only graph yet) |
| Next / previous commit in branch | Shift+J (first parent), Shift+K (child on the same branch) | done (M1; Shift+↑/↓ extend the selection since M4-01) |
| First / last commit | Ctrl+Home / Ctrl+End (macOS ⌘↑ / ⌘↓) | done (M1) |
| Undo / Redo | Ctrl+Z / Ctrl+Y or Ctrl+Shift+Z | done for commit, amend, discard (M2), checkout, branches, tags (M3), merge, fast-forward, rebase, interactive rebase, cherry-pick, revert, reset, reword, squash (M4-12), stash pop/apply, discard of conflicted files, reset on HEAD (M9), stash push and delete, stage/unstage and mark resolved (M12-07), pull, remotes, upstream and submodule update (M12-08); history operations are undone with `git reset --keep`, which refuses to overwrite local changes; what cannot be undone is listed in `docs/user/undo.md` (ADR-043) |

### Command palette and search

| Action | Shortcut | Status |
|---|---|---|
| Command palette | Ctrl+P | done (M5-06; all actions with shortcuts, disabled ones with the reason; sub-lists for repositories and branch checkout); M12-12: also Ctrl+Shift+P (VS Code habit, no GitKraken binding there), settings toggles, value lists and deep links into the Preferences |
| Search commits | Ctrl+F | done (M5-07; field above the graph, non-matching rows dimmed, n of m, Enter / Shift+Enter; verify in GitKraken: dim or filter) |
| Open repo via palette | Ctrl+Shift+O | done (M5-06; recent repositories and Browse…) |
| File history and blame | Ctrl+Shift+H | done (M5-09 history: palette list of tracked files, also the file context menu and the History button of the diff; M5-10 blame: History / Blame toggle, "Blame at this commit"; verify layout in GitKraken) |
| Focus left panel filter | Ctrl+Alt+F | done (M1) |
| Pull, Push, Stash, Pop stash | Command palette | done (M6-07; the toolbar actions are palette entries) |
| Open repo in terminal | Alt+T | done (M5-13; the system terminal, see deviations) |
| Open repo in file manager | Alt+O | done (M5-13) |
| Open repo in external editor | Alt+E | done (M8-08; Gitloom shortcut next to Alt+T/Alt+O; GitKraken check open) |
| Open diff/merge tool | Ctrl+D | done (M5-12; `git difftool`/`git mergetool`, tool chosen in Preferences → External tools; also the External button of the diff and "Open in merge tool" for conflicts) |
| Close current repo | Ctrl+W | done (M1) |

### Interface

| Action | Shortcut | Status |
|---|---|---|
| Fullscreen | F11 | done (M5-04) |
| Zoom in / out / reset | Ctrl+= / Ctrl+- / Ctrl+0 | done (M5-04; 70–200 % in 10 % steps, also Ctrl++ and numpad +, saved in the settings) |
| Toggle left panel | Ctrl+J | done (M1) |
| Toggle commit detail panel | Ctrl+K | done (M1) |
| Close current panel | Esc | done (M1, diff view) |
| Keyboard shortcut list | Ctrl+/ | done (M5-01; grouped by category, with a filter) |
| Preferences | Ctrl+, | done (M5-02; replaces the tab content, categories left; also the gear button in the toolbar) |

### Tabs

| Action | Shortcut | Status |
|---|---|---|
| New tab | Ctrl+T | done (M1) |
| Close tab | Ctrl+W | done (M1) |
| Switch to tab 1–9 | Ctrl+1…9 | done (M1) |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab | done (M1) |

## Intentional deviations

| Deviation | Reason |
|---|---|
| Gitflow layout of the graph (lane groups production, hotfix, release, development, features; one color per type; deleted branches inferred from merge messages and topology; faded labels) | Gitloom extension (M13, ADR-048 tokens); GitKraken has no such layout; off by default, per repository |
| Date and SHA columns cannot be resized; their width always fits the content | Maintainer decision 2026-11-12: their content has a constant width, resizing only produced gaps or clipped text |
| Notifications always at the bottom left | GitKraken has a location setting; out of scope for M12 (ADR-049), the default location is GitKraken's |
| WSL notice, recent-list badge and polling watcher for repositories inside WSL; plain-language "Trust this repository…" for Git's dubious-ownership refusal | Gitloom extension (M12-14, ADR-053); whether GitKraken shows a notice for WSL or network paths is an open verification item |
| Multi-selection of branches and tags in the left panel (Ctrl/Shift+click) with bulk delete and hide/show | Gitloom extension (M12-11, maintainer request; whether GitKraken 7.5.2 offers it is an open verification item); a plain click behaves as before |
| A hotfix finished while a release branch exists is merged into the development branch | GitKraken's documentation; the `git-flow` CLI merges into the release branch instead (M7-09, GitKraken check open) |
| Ctrl+O opens a folder dialog directly | Quick access to the folder dialog (the palette list is Ctrl+Shift+O); does not collide with a GitKraken shortcut |
| F5 reloads the whole repository | Manual full reload in addition to the file watcher (M2) |
| The left panel trees list branches (or tags) before folders on every level, both alphabetical | Maintainer decision 2026-11-11 (M11-11): the items you work with come first; GitKraken's own order differs between sections |
| Preferences open as a centered modal over the dimmed window (fills small windows) instead of a full view | Maintainer decision 2026-11-11 (M11-13): the repository stays visible; "×" and Esc close it, a click beside it does not |
| The graph table scrolls horizontally instead of fitting the window: every column, Commit message included, has its own width and handle, and dragging changes only that column | Maintainer decision 2026-11-11 (M11-16): resizing that narrowed the message felt odd and the message had no handle; a narrower table lets the message fill |
| Preferences → Editor has no "End of Line Character" | Gitloom has no in-app file editor; line endings stay as Git writes them |
| Hovering a file shows only "Stage File" / "Unstage File"; Discard is in the context menu | Maintainer decision 2026-11-10 (M12-01): tidier, no accidental discard; "Discard all" in the header stays |
| Notifications are always at the bottom left | GitKraken lets you choose the location; one fixed place keeps the UI simple (ADR-049) |
| Reset is a dialog (Soft / Mixed / Hard) instead of GitKraken's submenu | The maintainer prefers the dialog (2026-11-11); it also explains the three modes |
| Conflicted files without a running Git operation show the graph row "Conflicted files without a running operation" and "Discard conflicted files…" below the commit button | ADR-044 (no dead ends): after a stash conflict and a restart nothing else explains them |
| The repository and branch pickers open the centered palette lists ("Open recent repository", "Checkout branch") instead of a dropdown below the picker | M6-17: reuses the existing lists before 1.0; a real dropdown is a backlog item |
| Start screen has a path input below "Open a repo" | Fallback where no native folder dialog is available (Linux without XDG desktop portal, e.g. WSL) |
| "Discard all" with staged changes asks whether to discard them too ("Keep staged" is the default) | Maintainer decision (HANDOVER question 8); staged work is never lost by accident |
| Discarding a hunk, a change block or selected lines asks for confirmation | Maintainer decision: discarding is destructive at every granularity; GitKraken asks only for file and all |
| While an operation is in progress, Amend, Stash/Pop/Apply and undo/redo of commits are disabled | Prevents breaking the operation; Git itself refuses some of these (amend during a merge) |
| A Git lock file (`index.lock`, ref locks) makes the operation fail with a dialog offering "Retry"; the lock is never removed | Deleting a lock of a running Git process corrupts the repository |
| In partial clones, commit file lists may show a rename as deletion plus addition | Similarity rename detection needs blob contents that are not local; fetching them all for a file list would download much of the history |
| Checkout blocked by local changes offers "Stash and checkout" | Avoids a dead end; the stash stays visible and is not popped automatically |
| Merge, fast-forward or rebase blocked by local changes offers "Stash and merge" / "Stash and rebase" (stash, run, apply again with `--autostash`) | Planning decision M4 (same pattern as checkout); unlike checkout the stash is applied again automatically, and stays in the list only if applying it conflicts |
| A rebase over merge commits is confirmed first ("Rebase flattens merges"), and the interactive rebase panel says how many merges it drops | M4-13 plan: Git's default flattens merges; the hint prevents surprises |
| A pull that stops in a conflict shows a dialog offering "Abort merge"/"Abort rebase" | The user is not left in a half-finished state without a way back; resolving and continuing works from the right panel |
| The commit message of a rebase step is editable before Continue (prefilled with the original message) | Git asks for the message on `rebase --continue` after a conflict; the commit panel answers instead of a terminal editor |
| Shift+↑/↓ extend the commit selection; branch navigation stays on Shift+J/K | M4 plan: multi-selection needs a keyboard way to extend; until M4 Shift+↑/↓ also navigated parent/child |
| The toolbar's Terminal button and Alt+T open the system terminal in the repository instead of a terminal inside the app | M5 plan: an integrated terminal is a backlog item; the command can be set in Preferences → External tools |
| "About Gitloom" is in the command palette and at the bottom of the Preferences categories instead of a Help menu | Gitloom has no menu bar and its gear button opens the Preferences directly (M6-03) |
| Focused text fields get a 2 px accent border; Esc closes a dialog or menu even while one of its fields has focus | Accessibility pass (M6-07): visible focus and one Esc per overlay |
| Screen readers are not supported | iced 0.14 exposes no accessibility tree (M6 plan); documented as a known limitation |
| UTF-16 files show as text, but line and block staging is disabled for them | M7-17: `git apply` treats UTF-16 as binary; whole-file staging works |
| Identity per repository (`user.name`/`user.email` in the repository config) instead of switching a global profile | ADR-041; identities only, no workspace profiles (planning decision 2026-11-09) |
| SSH key per repository instead of one global key | ADR-041; uses Git's own `core.sshCommand`, so the command line behaves the same |
| Toolbar button "Editor" and Alt+E for the external editor | M8 plan; mirrors Terminal (Alt+T) |
| No accounts, integrations, AI, cloud features | Project scope (see ROADMAP non-goals) |
| Ref menus (branch, remote branch, tag) do not offer Cherry pick commit, Revert commit, Edit commit message, Create annotated tag here, Push and start a pull request, tag Hide/Solo, Hide/Show all tags, Annotate tag | M13-06: only actions used often on a ref are taken over; commit actions stay in the commit menu, the tag form covers annotated tags, no provider APIs |
| File menus do not offer the Ignore submenu (file / extension / folder), Edit file, Delete file and Open file in default program | M13-07: Ignore adds the file itself; no in-app editor; the external editor and Open file location cover the rest |
| Open file location of a commit file is disabled only when the file was deleted in that commit; a file missing from the working tree for other reasons opens its nearest folder | M13-07: the UI thread does no file I/O, so the menu cannot check the disk while it opens |
| "Create branch here" of a ref whose commit has no row in the graph opens a small "Create branch at <ref>" dialog | M13-06, ADR-044: no dead end when the branch is hidden or the commit is beyond the loaded history |
| With several remotes or none, the tag entries read "Push `<tag>`…" / "Delete `<tag>` from remote…" and ask which remote | M13-06: GitKraken names the remote; the remote is only known when there is exactly one |
