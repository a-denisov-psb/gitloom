# Coming from GitKraken

Gitloom copies GitKraken's layout and interactions on purpose: tabs, toolbar, left panel, graph with the WIP row, details panel (right or pinned to the bottom), double click to check out, drag a branch onto another, right click for context menus, and the same keyboard shortcuts. If something works differently, it is listed in [UX-PARITY.md](../UX-PARITY.md) together with the reason.

Gitloom is not affiliated with GitKraken or Axosoft.

## What is not there

No accounts, no cloud workspaces, no pull request or issue tracker integrations, no AI features, no subscription. Everything runs locally against your repositories and your `git`.

## Shortcuts you already know

| Action | Shortcut |
|---|---|
| Command palette | Ctrl+P |
| Keyboard shortcut list | Ctrl+/ |
| Search commits | Ctrl+F |
| Fetch all | Ctrl+L |
| Create branch | Ctrl+B |
| Stage / unstage the shown file | S / U |
| Stage all / unstage all | Ctrl+Shift+S / Ctrl+Shift+U |
| Commit / stage all and commit | Ctrl+Enter / Ctrl+Shift+Enter |
| Focus the commit message | Ctrl+Shift+M |
| Undo / redo | Ctrl+Z / Ctrl+Y (or Ctrl+Shift+Z) |
| Next / previous commit | ↓ or J / ↑ or K |
| Along the branch (first parent / child) | Shift+J / Shift+K |
| First / last commit | Ctrl+Home / Ctrl+End |
| File history and blame | Ctrl+Shift+H |
| Open diff or merge tool | Ctrl+D |
| Open terminal / file manager | Alt+T / Alt+O |
| Toggle left / details panel | Ctrl+J / Ctrl+K |
| Preferences | Ctrl+, |
| Zoom in / out / reset, fullscreen | Ctrl+= / Ctrl+- / Ctrl+0, F11 |
| Tabs | Ctrl+T, Ctrl+W, Ctrl+Tab, Ctrl+Shift+Tab, Ctrl+1…9 |

## Differences in daily use

- **Ctrl+O** opens a folder dialog directly; the list of recent repositories is **Ctrl+Shift+O**. **F5** reloads the whole repository.
- **Shift+↑/↓** extend the commit selection (like Ctrl/Shift+click); moving along a branch is Shift+J/K.
- **Shift+F10** opens the context menu of the selected commit; ↑/↓ and Enter pick an entry.
- **Terminal** (Alt+T) opens your system terminal in the repository instead of a terminal inside the app.
- Discarding a hunk or selected lines asks first (GitKraken asks only for files and "discard all"). "Discard all" with staged changes asks whether to discard those too.
- **Reset** opens a dialog with Soft / Mixed / Hard instead of a submenu; Hard lists the changes that will be lost. It is also offered on the checked-out commit.
- A stash pop or apply that conflicts can be undone completely (Undo below the commit button, Ctrl+Z); conflicted files can be discarded and the discard undone.
- Merge, rebase or checkout blocked by local changes offers "Stash and …"; merge and rebase re-apply the stash automatically.
- A Git lock file (`index.lock`) is never deleted; the operation offers **Retry** instead.
- Every graph column, Commit message included, has its own width; a table wider than the window scrolls sideways instead of fitting it. The gear above the graph shows or hides Author, Date and SHA.
- The left panel lists branches before folders on every level, also in GITFLOW and TAGS (tags have folders too).
- **Preferences** open as a window over the app instead of a full view.
- Two small toggles at the ends of the status bar hide and show the side panels, like Zed (GitKraken: shortcuts and the View menu only).
- Hovering a file in the staging area shows only **Stage File** / **Unstage File**; **Discard** is in the right-click menu.
- Notifications sit at the bottom left (GitKraken lets you move them); errors explain Git's message in plain words and keep Git's output under **Details**.
- Stage, unstage, mark resolved, stash, delete/rename stash, pull, remote and upstream changes and submodule updates can be undone as well ([Undo](undo.md)).
- Several branches and tags can be selected (Ctrl/Shift+click) and deleted in one step.
- The command palette also toggles settings and opens any Preferences setting; Ctrl+Shift+P opens it too.
- **Gitflow layout (experimental)** (gear menu above the graph): with Git flow initialized the graph groups lanes like the classic Git flow diagram, also for deleted branches (Gitloom extension).
- Feature, release and hotfix rows of the GITFLOW section show a flag on hover that finishes the branch.
- A sparkle button next to "Committing as …" copies a commit message prompt for a tool of your choice (no AI inside Gitloom).
- Preferences → Editor has no "End of Line Character" (there is no in-app file editor).
- "About Gitloom" is in the command palette and at the bottom of the Preferences.
- **Themes**: besides Dark and Light there are Dark+, Light+, Monokai, GitHub Dark, GitHub Light, Solarized Light, Warm Dark and Warm Light (Preferences → Appearance or "Set: Theme ›" in the palette); System can use any dark and any light theme; Preferences → Themes makes your own themes from a copy, with a preview (Gitloom extensions).
- The repository and branch pickers in the toolbar open the centered command palette lists instead of a dropdown below the picker.
- Commit messages are read and edited in a **Commit message** window instead of inline: the button at the right of the commit details header (shown while the pointer is over the header or the message) opens it; a click on the text only selects it. Only commits that were not pushed can be edited; for the others (and stashes) the window opens read-only with the reason (pushed ones point to interactive rebase) and **Copy message**. The window's sparkle builds the commit message prompt for that commit.
- **Pin to bottom** is the arrow button in the status bar left of the details panel toggle (and a palette action) instead of an icon in the details header; there is no Preferences entry for it. At the bottom the staging area shows the file lists side by side and a grey bar to share the height with the commit message.
- The commit details do not scroll as a whole: the file list has its own frame and scrollbar, the message box takes at most a fifth of the panel, and **Expand all** / **Collapse all** is one button in the first row of the tree.
- The window has a minimum size (about 1000 × 670 at 100 % zoom, scaled with the zoom) and the panes keep minimum widths and heights.
- The author of unpushed commits can be changed: "Change author…" in the commit menu (also for several selected commits) and the "Authored by" row of the message window (Gitloom extension).
- The diff minimap (Inline and Split) marks the changes right of the scrollbar like GitKraken's; it is not shown in the Hunk view.
- A Graph column narrower than its lanes shows the nodes of the hidden lanes in its last lane instead of cutting them off.
- In the commit details' file list the sort toggle reads "A→Z" / "Z→A", and a click on an unchanged file ("View all files") shows its content in the blame view.
- **Identities instead of profiles:** Preferences → Identities keeps names and emails; each repository uses one through its own Git config ("Committing as …" in the staging area) instead of switching a global profile. The default can keep the global Git config up to date.
- **SSH key per repository** (Preferences → Repository → SSH key) instead of one global key; it is Git's own `core.sshCommand`.
- **Signing** is Preferences → Signing (GPG or SSH) instead of GitKraken's GPG page; there is no key generator.
- **Commit template** lives in Preferences → Repository → Commit; the **commit message prompt** and the **stash message template** are on a global Templates page (Gitloom extensions).
- **External editor** can be set per repository in addition to the global one.
- The start screen has no hosted repositories, boards or account features.
- **Editor** button and Alt+E open the external editor, next to Terminal (Alt+T).
- Git flow needs no `git-flow` tool; a hotfix finished while a release branch exists is merged into the development branch (see [Git flow](git-flow.md)).
