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
| Graph | Columns: branch/tag, graph, commit message (+ author, date, SHA); WIP row on top | done (M0), real M1 |
| Right panel | Commit details or staging area (WIP selected) | done (M0), real M1 |
| Diff | Replaces the graph in the center; close returns to graph | placeholder M0, real M1 |
| Status bar | Bottom line | done |
| Resizable panes | All splits draggable | done |

## Interactions

| Interaction | Behavior | Status |
|---|---|---|
| Click commit | Select, show details right | done |
| Click WIP | Staging area right | done |
| Click file in panel | Diff in center | done (placeholder), M1 real |
| Click branch in left panel | Jump to and select its commit | done |
| Double click local branch | Checkout | M3 |
| Double click remote branch | Create tracking branch and checkout | M3 |
| Right click commit/branch | Context menu | M3 |
| Drag branch onto branch | Merge/rebase menu | M3 |
| Hover file in staging | Shows stage/unstage button | M2 |
| Local + remote branch on same commit | One combined pill | M1 |
| Commit node | Avatar (placeholder: initials) | M1 initials, M5 avatars |

## Keyboard shortcuts

Windows/Linux shown; macOS uses ⌘ instead of Ctrl (and Option instead of Alt).

### Repo actions

| Action | Shortcut | Status |
|---|---|---|
| Create branch | Ctrl+B | M3 |
| Fetch all | Ctrl+L | M3 |
| Stage current file | S | M2 |
| Stage all files | Ctrl+Shift+S | M2 |
| Unstage current file | U | M2 |
| Unstage all files | Ctrl+Shift+U | M2 |
| Commit staged files | Ctrl+Enter | M2 |
| Stage all and commit | Ctrl+Shift+Enter | M2 |
| Focus commit message | Ctrl+Shift+M | M2 |

### Navigation

| Action | Shortcut | Status |
|---|---|---|
| Next / previous item | ↓ / J, ↑ / K (verify direction, docs table looks swapped) | M1 |
| Item right / left | → / L, ← / H | M1 (where applicable) |
| Next / previous commit in branch | Shift+↓ / Shift+J, Shift+↑ / Shift+K | M1 |
| First / last commit | Ctrl+Home / Ctrl+End (macOS ⌘↑ / ⌘↓) | M1 |
| Undo / Redo | Ctrl+Z / Ctrl+Y or Ctrl+Shift+Z | M4 |

### Command palette and search

| Action | Shortcut | Status |
|---|---|---|
| Command palette | Ctrl+P | M5 |
| Search commits | Ctrl+F | M5 |
| Open repo via palette | Ctrl+Shift+O | M5 |
| File history and blame | Ctrl+Shift+H | M5 |
| Focus left panel filter | Ctrl+Alt+F | M1 |
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
| Close current panel | Esc | M1 (diff view) |
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
| F5 refreshes the repository | No file watcher until M2 |
| Start screen has a path input next to "Open repository" | Fallback where no native folder dialog is available (Linux without XDG desktop portal, e.g. WSL) |
| No accounts, integrations, AI, cloud features | Project scope (see ROADMAP non-goals) |
