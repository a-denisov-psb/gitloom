# UX parity with GitKraken

Reference for layout, interactions and shortcuts. A GitKraken user must feel at home without relearning. Colors and icons are our own.

Shortcut source: GitKraken Desktop keyboard shortcut documentation (help.gitkraken.com, state April 2026). Verify behavior in a real GitKraken installation when in doubt.

Status values: `done`, `M<n>` (planned milestone), `—` (not planned).

## Layout

| Area | GitKraken behavior | Status |
|---|---|---|
| Repo tabs | Top row, one tab per repo, `+` opens a new tab with the start screen | done (M1) |
| Toolbar | Repo + branch pickers left; Undo/Redo, Pull/Push, Branch, Stash/Pop centered; Terminal right | layout done, actions M2–M4 |
| Left panel | Local, Remote (grouped by remote), Tags, Stashes, Submodules; collapsible; filter on top; counts | done (M0), real data M1 |
| Graph | Columns: branch/tag, graph, commit message (+ author, date, SHA); WIP row on top | done (M1) |
| Right panel | Commit details or staging area (WIP selected) | done (M0), real M1 |
| Diff | Replaces the graph in the center; close returns to graph | done (M1, hunk view; inline/split later) |
| Status bar | Bottom line | done |
| Resizable panes | All splits draggable | done |

## Interactions

| Interaction | Behavior | Status |
|---|---|---|
| Click commit | Select, show details right | done |
| Click WIP | Staging area right | done |
| Click file in panel | Diff in center | done (M1) |
| Click branch in left panel | Jump to and select its commit | done |
| Double click local branch | Checkout | M3 |
| Double click remote branch | Create tracking branch and checkout | M3 |
| Right click commit/branch | Context menu | M3 |
| Drag branch onto branch | Merge/rebase menu | M4 (moved from M3) |
| Toolbar Pull dropdown | Fetch all, Pull (ff if possible), Pull (ff only), Pull (rebase); last choice becomes default | Fetch all done (M3-03), pull modes M3-04 |
| Push without upstream | Dialog to choose remote and branch name, sets upstream | M3 |
| Push rejected | Offer Pull or Force push (with lease) | M3 |
| Branch button / Ctrl+B | Inline name input at the selected commit; creates and checks out | M3 |
| Auto-fetch | Periodic fetch in the background | done (M3, every 5 min, `auto_fetch_minutes` in the session file) |
| Hover file in staging | Stage/Unstage and Discard buttons appear | done (M2) |
| Right click file in staging | Context menu: Stage/Unstage, Discard, Ignore, Open file location | done (M2, plus Copy file path) |
| Stage hunk / lines in the diff view | "Stage hunk" per hunk; click, Shift+click, Ctrl+click select lines, "Stage selected lines" | done (M2) |
| Stage file | Selection moves to the next unstaged file and shows its diff | done (M2; unstaging moves within the staged list the same way) |
| Stash (toolbar) | Stashes all changes including untracked files | done (M2) |
| Stash in graph | Stash shown as node attached to its base commit | done (M2, square node, dashed edge) |
| Right click stash | Apply, Pop, Delete | done (M2, sidebar and graph) |
| Amend | "Amend previous commit" checkbox fills message from HEAD | done (M2) |
| Merge in progress | Banner in the tab, commit concludes the merge | done (M2-09, M2-13); also rebase, cherry-pick, revert; Abort/Continue shown disabled until M4 |
| Local + remote branch on same commit | One combined pill | done (M1, remote names as text until the icon set in M5) |
| Commit node | Avatar (placeholder: initials) | initials done (M1), avatars M5 |

## Keyboard shortcuts

Windows/Linux shown; macOS uses ⌘ instead of Ctrl (and Option instead of Alt).

### Repo actions

| Action | Shortcut | Status |
|---|---|---|
| Create branch | Ctrl+B | M3 |
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
| Item right / left | → / L, ← / H | — in M1 (no horizontal items in the read-only graph yet) |
| Next / previous commit in branch | Shift+↓ / Shift+J (first parent), Shift+↑ / Shift+K (child on the same branch) | done (M1) |
| First / last commit | Ctrl+Home / Ctrl+End (macOS ⌘↑ / ⌘↓) | done (M1) |
| Undo / Redo | Ctrl+Z / Ctrl+Y or Ctrl+Shift+Z | done for commit, amend, discard (M2), rest M4 |

### Command palette and search

| Action | Shortcut | Status |
|---|---|---|
| Command palette | Ctrl+P | M5 |
| Search commits | Ctrl+F | M5 |
| Open repo via palette | Ctrl+Shift+O | M5 |
| File history and blame | Ctrl+Shift+H | M5 |
| Focus left panel filter | Ctrl+Alt+F | done (M1) |
| Open repo in terminal | Alt+T | M5 |
| Open repo in file manager | Alt+O | M5 |
| Open diff/merge tool | Ctrl+D | M5 |
| Close current repo | Ctrl+W | done (M1) |

### Interface

| Action | Shortcut | Status |
|---|---|---|
| Fullscreen | F11 | M5 |
| Zoom in / out / reset | Ctrl+= / Ctrl+- / Ctrl+0 | M5 |
| Toggle left panel | Ctrl+J | done (M1) |
| Toggle commit detail panel | Ctrl+K | done (M1) |
| Close current panel | Esc | done (M1, diff view) |
| Keyboard shortcut list | Ctrl+/ | M5 |

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
| Ctrl+O opens a folder dialog directly | Quick access before the command palette exists (M5); does not collide with a GitKraken shortcut |
| F5 reloads the whole repository | Manual full reload in addition to the file watcher (M2) |
| Author, date and SHA columns hide automatically (SHA first) when the commit message would get narrower than 200 px | Column resizing and a column menu like GitKraken's follow later; until then the message stays readable on wide graphs |
| Start screen has a path input next to "Open repository" | Fallback where no native folder dialog is available (Linux without XDG desktop portal, e.g. WSL) |
| "Discard all" with staged changes asks whether to discard them too ("Keep staged" is the default) | Maintainer decision (HANDOVER question 8); staged work is never lost by accident |
| Discarding a hunk or selected lines asks for confirmation | Maintainer decision: discarding is destructive at every granularity; GitKraken asks only for file and all |
| While an operation is in progress, Amend, Stash/Pop/Apply and undo/redo of commits are disabled; during a rebase also Commit | Prevents breaking the operation until M4 brings Continue/Abort; Git itself refuses some of these (amend during a merge) |
| A Git lock file (`index.lock`, ref locks) makes the operation fail with a dialog offering "Retry"; the lock is never removed | Deleting a lock of a running Git process corrupts the repository |
| In partial clones, commit file lists may show a rename as deletion plus addition | Similarity rename detection needs blob contents that are not local; fetching them all for a file list would download much of the history |
| Checkout blocked by local changes offers "Stash and checkout" | Avoids a dead end; the stash stays visible and is not popped automatically |
| No accounts, integrations, AI, cloud features | Project scope (see ROADMAP non-goals) |
