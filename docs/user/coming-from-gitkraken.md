# Coming from GitKraken

Gitloom copies GitKraken's layout and interactions on purpose: tabs, toolbar, left panel, graph with the WIP row, commit panel, double click to check out, drag a branch onto another, right click for context menus, and the same keyboard shortcuts. If something works differently, it is listed in [UX-PARITY.md](../UX-PARITY.md) together with the reason.

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
| Toggle left / right panel | Ctrl+J / Ctrl+K |
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
- Two toolbar buttons at the right hide and show the side panels (GitKraken: shortcuts and the View menu only).
- **Classic Dark** brings back the look of older GitKraken versions; Preferences → Editor has no "End of Line Character" (there is no in-app file editor).
- "About Gitloom" is in the command palette and at the bottom of the Preferences.
- The repository and branch pickers in the toolbar open the centered command palette lists instead of a dropdown below the picker.
- **Identities instead of profiles:** Preferences → Identities keeps names and emails; each repository uses one through its own Git config ("Committing as …" in the commit panel) instead of switching a global profile. The default can keep the global Git config up to date.
- **SSH key per repository** (Preferences → Repository → SSH key) instead of one global key; it is Git's own `core.sshCommand`.
- **Signing** is Preferences → Signing (GPG or SSH) instead of GitKraken's GPG page; there is no key generator.
- **Commit template** lives in Preferences → Repository → Commit; the **commit message prompt** and the **stash message template** are on a global Templates page (Gitloom extensions).
- **External editor** can be set per repository in addition to the global one.
- The start screen has no hosted repositories, boards or account features.
- **Editor** button and Alt+E open the external editor, next to Terminal (Alt+T).
- Git flow needs no `git-flow` tool; a hotfix finished while a release branch exists is merged into the development branch (see [Git flow](git-flow.md)).
