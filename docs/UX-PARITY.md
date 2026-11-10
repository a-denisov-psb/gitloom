# UX parity with GitKraken

Reference for layout, interactions and shortcuts. A GitKraken user must feel at home without relearning. Colors and icons are our own.

Shortcut source: GitKraken Desktop keyboard shortcut documentation (help.gitkraken.com, state April 2026). Verify behavior in a real GitKraken installation when in doubt.

Status values: `done`, `M<n>` (planned milestone), `—` (not planned).

## Layout

| Area | GitKraken behavior | Status |
|---|---|---|
| Repo tabs | Top row, one tab per repo, `+` opens a new tab with the start screen | done (M1) |
| Toolbar | Repo + branch pickers left; Undo/Redo, Pull/Push, Branch, Stash/Pop centered; Terminal right | done (M2–M4 actions; M5-13 Terminal; M5-02 a Settings gear next to it); M6-17 pickers open the palette lists (see deviations); M8 external editor button next to Terminal |
| Left panel | Local, Gitflow (when initialized), Remote (grouped by remote), Tags, Stashes, Submodules; collapsible; filter on top; counts; branches grouped into folders at `/` with icons; checked-out branch highlighted; "Viewing x/y · Show All" when branches are hidden | done (M0), real data M1; folders, icons, highlight and counter M7-14 (folders before branches, tags flat, no folder menu: GitKraken check open); GITFLOW section M7-08 (groups only when they have branches), between LOCAL and REMOTE since M7-15 (maintainer decision) |
| Graph | Columns: branch/tag, graph, commit message (+ author, date, SHA); WIP row on top; resizable columns and a column menu | done (M1); straight lines with rounded 90° corners and dotted WIP/stash edges since M7-18; resizable columns and column menu M9 |
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
| Right click commit/branch | Context menu | done (M3; graph rows, ref pills, sidebar; M4 actions disabled with "Not available yet") |
| Branch context menu: merge and rebase | "Merge X into Y", "Rebase Y onto X"; "Fast-forward Y to X" when possible; remote branch: "Fast-forward L to origin/L" for a local branch behind it | done (M4-04, M4-06) |
| Drag branch onto branch | Menu: Fast-forward, Merge, Rebase, Interactive rebase | done (M4-11): graph pills and sidebar branches, drop target outlined; items that need another checked-out branch are disabled with "Check out <branch> first" (verify in GitKraken: disabled or automatic checkout?) |
| Toolbar Pull dropdown | Fetch all, Pull (ff if possible), Pull (ff only), Pull (rebase); last choice becomes default | done (M3) |
| Push without upstream | Dialog to choose remote and branch name, sets upstream | done (M3) |
| Push rejected | Offer Pull or Force push (with lease) | done (M3; force push asks once more) |
| Branch button / Ctrl+B | Inline name input at the selected commit; creates and checks out | done (M3; also "Create branch here" in the commit menu) |
| Auto-fetch | Periodic fetch in the background | done (M3, every 5 min; interval in Preferences → General since M5-02) |
| Hover file in staging | Stage/Unstage and Discard buttons appear | done (M2) |
| Right click file in staging | Context menu: Stage/Unstage, Discard, Ignore, Open file location | done (M2, plus Copy file path); M8 "Open in external editor" |
| Stage hunk / lines in the diff view | "Stage hunk" per hunk; click, Shift+click, Ctrl+click select lines, "Stage selected lines" | done (M2) |
| Diff view mode | Hunk / Inline / Split toggle in the diff header; arrows jump between changes | done (M7: one global setting; staging by line in all modes and by block in Inline/Split; read-only diffs have no bars; GitKraken checks on remembering, block buttons and Split alignment open) |
| Gitflow | Preferences → Gitflow initializes; GITFLOW section in the left panel with a green start button; branch menu "Finish …" | done (M7: native, git-flow compatible config, ADR-037; also palette entries; finish across conflicts and one undo step; GitKraken checks on dialogs and section position open) |
| Commit identity | Profiles (Preferences → Profiles) hold author name and email; the active profile commits | M8: Preferences → Identities; per repository "Committing as …" in the commit panel, written to the repository config (see deviations) |
| Repo-specific preferences | Group of categories for the open repository (Encoding, Gitflow, Git Hooks, LFS, Commit, Submodules, …) | M8: group "Repository · <name>" with Identity, SSH key, Gitflow; group with Gitflow done (M8-01), Identity and SSH key follow (M8-04, M8-06) |
| SSH key | Preferences → SSH: one key pair or the local SSH agent | M8: per repository via `core.sshCommand`, system default otherwise (see deviations) |
| External editor | Preferences → External tools: VS Code, Sublime, IntelliJ or a custom path | M8: presets fill a command template with `{path}`; repository via palette, Alt+E and toolbar; files via their context menus |
| Stage file | Selection moves to the next unstaged file and shows its diff | done (M2; unstaging moves within the staged list the same way) |
| Stash (toolbar) | Stashes all changes including untracked files | done (M2) |
| Stash in graph | Stash shown as node attached to its base commit | done (M2, square node, dashed edge) |
| Right click stash | Apply, Pop, Delete | done (M2, sidebar and graph) |
| Right click remote | Fetch, Edit URL, Rename, Remove | done (M3); M6-18 remotes defined outside the repository config cannot be removed, renamed or edited |
| "+" in the remote section | Add a remote (name, URL) | done (M3) |
| Right click tag | Push to remote, Delete local, Delete on remote, Copy name | done (M3, sidebar and graph pill) |
| Create tag here (commit menu) | Name and optional message (annotated if a message is given) | done (M3) |
| Amend | "Amend previous commit" checkbox fills message from HEAD | done (M2) |
| Merge in progress | Banner in the tab with Continue/Abort (Skip for rebase and cherry-pick); the commit button becomes "Continue <operation>" | done (M2-09, M2-13, M4-03); also rebase, cherry-pick, revert; Continue is disabled while conflicts are unresolved (tooltip lists the files) |
| Reset | Commit menu "Reset <branch> to this commit" → Soft / Mixed / Hard | done (M4-10) as a dialog with three buttons instead of GitKraken's submenu (the context menu has no submenus); Hard is always confirmed and lists the files whose changes are lost |
| Cherry-pick and revert | Commit menu "Cherry-pick commit" / "Revert commit"; multi-selection "Cherry-pick N commits" (oldest first) | done (M4-09); a merge commit asks for the parent to revert against; an empty result opens "Skip" / "Commit empty" (verify wording in GitKraken) |
| Conflicted files | "Conflicted Files" section above "Unstaged Files" | done (M4-03) |
| Local + remote branch on same commit | One combined pill | done (M1; since M5-05 a cloud icon, the remote names in its tooltip) |
| Commit node | Avatar (placeholder: initials) | done (M5-11; Gravatar opt-in in Preferences → Appearance, initials otherwise) |
| Conflicted file | Opens conflict view (ours, theirs, output) | done (M4-05); with both sides picked in one conflict, ours lines come first (GitKraken keeps the click order) |
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
| Undo / Redo | Ctrl+Z / Ctrl+Y or Ctrl+Shift+Z | done for commit, amend, discard (M2), checkout, branches, tags (M3), merge, fast-forward, rebase, interactive rebase, cherry-pick, revert, reset, reword, squash (M4-12); history operations are undone with `git reset --keep`, which refuses to overwrite local changes |

### Command palette and search

| Action | Shortcut | Status |
|---|---|---|
| Command palette | Ctrl+P | done (M5-06; all actions with shortcuts, disabled ones with the reason; sub-lists for repositories and branch checkout) |
| Search commits | Ctrl+F | done (M5-07; field above the graph, non-matching rows dimmed, n of m, Enter / Shift+Enter; verify in GitKraken: dim or filter) |
| Open repo via palette | Ctrl+Shift+O | done (M5-06; recent repositories and Browse…) |
| File history and blame | Ctrl+Shift+H | done (M5-09 history: palette list of tracked files, also the file context menu and the History button of the diff; M5-10 blame: History / Blame toggle, "Blame at this commit"; verify layout in GitKraken) |
| Focus left panel filter | Ctrl+Alt+F | done (M1) |
| Pull, Push, Stash, Pop stash | Command palette | done (M6-07; the toolbar actions are palette entries) |
| Open repo in terminal | Alt+T | done (M5-13; the system terminal, see deviations) |
| Open repo in file manager | Alt+O | done (M5-13) |
| Open repo in external editor | Alt+E | M8 (Gitloom shortcut next to Alt+T/Alt+O; GitKraken check open) |
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
| A hotfix finished while a release branch exists is merged into the development branch | GitKraken's documentation; the `git-flow` CLI merges into the release branch instead (M7-09, GitKraken check open) |
| Ctrl+O opens a folder dialog directly | Quick access to the folder dialog (the palette list is Ctrl+Shift+O); does not collide with a GitKraken shortcut |
| F5 reloads the whole repository | Manual full reload in addition to the file watcher (M2) |
| Author, date and SHA columns hide automatically (SHA first) when the commit message would get narrower than 200 px | Column resizing and a column menu like GitKraken's follow in M9; until then the message stays readable on wide graphs |
| The repository and branch pickers open the centered palette lists ("Open recent repository", "Checkout branch") instead of a dropdown below the picker | M6-17: reuses the existing lists before 1.0; a real dropdown is a backlog item |
| Start screen has a path input next to "Open repository" | Fallback where no native folder dialog is available (Linux without XDG desktop portal, e.g. WSL) |
| "Discard all" with staged changes asks whether to discard them too ("Keep staged" is the default) | Maintainer decision (HANDOVER question 8); staged work is never lost by accident |
| Discarding a hunk, a change block or selected lines asks for confirmation | Maintainer decision: discarding is destructive at every granularity; GitKraken asks only for file and all |
| While an operation is in progress, Amend, Stash/Pop/Apply and undo/redo of commits are disabled | Prevents breaking the operation; Git itself refuses some of these (amend during a merge) |
| A Git lock file (`index.lock`, ref locks) makes the operation fail with a dialog offering "Retry"; the lock is never removed | Deleting a lock of a running Git process corrupts the repository |
| In partial clones, commit file lists may show a rename as deletion plus addition | Similarity rename detection needs blob contents that are not local; fetching them all for a file list would download much of the history |
| Checkout blocked by local changes offers "Stash and checkout" | Avoids a dead end; the stash stays visible and is not popped automatically |
| Merge, fast-forward or rebase blocked by local changes offers "Stash and merge" / "Stash and rebase" (stash, run, apply again with `--autostash`) | Planning decision M4 (same pattern as checkout); unlike checkout the stash is applied again automatically, and stays in the list only if applying it conflicts |
| A rebase over merge commits is confirmed first ("Rebase flattens merges"), and the interactive rebase panel says how many merges it drops | M4-13 plan: Git's default flattens merges; the hint prevents surprises |
| A pull that stops in a conflict shows a dialog offering "Abort merge"/"Abort rebase" | The user is not left in a half-finished state without a way back; resolving and continuing works from the banner |
| The commit message of a rebase step is editable before Continue (prefilled with the original message) | Git asks for the message on `rebase --continue` after a conflict; the commit panel answers instead of a terminal editor |
| Shift+↑/↓ extend the commit selection; branch navigation stays on Shift+J/K | M4 plan: multi-selection needs a keyboard way to extend; until M4 Shift+↑/↓ also navigated parent/child |
| The toolbar's Terminal button and Alt+T open the system terminal in the repository instead of a terminal inside the app | M5 plan: an integrated terminal is a backlog item; the command can be set in Preferences → External tools |
| "About Gitloom" is in the command palette and at the bottom of the Preferences categories instead of a Help menu | Gitloom has no menu bar and its gear button opens the Preferences directly (M6-03) |
| Focused text fields get a 2 px accent border; Esc closes a dialog or menu even while one of its fields has focus | Accessibility pass (M6-07): visible focus and one Esc per overlay |
| Screen readers are not supported | iced 0.14 exposes no accessibility tree (M6 plan); documented as a known limitation |
| The GITFLOW section sits between LOCAL and REMOTE | Maintainer decision (2026-11-10, M7-15); GitKraken's position not verified |
| UTF-16 files show as text, but line and block staging is disabled for them | M7-17: `git apply` treats UTF-16 as binary; whole-file staging works |
| Identity per repository (`user.name`/`user.email` in the repository config) instead of switching a global profile | ADR-041; identities only, no workspace profiles (planning decision 2026-11-09) |
| SSH key per repository instead of one global key | ADR-041; uses Git's own `core.sshCommand`, so the command line behaves the same |
| Toolbar button "Editor" and Alt+E for the external editor | M8 plan; mirrors Terminal (Alt+T) |
| No accounts, integrations, AI, cloud features | Project scope (see ROADMAP non-goals) |
