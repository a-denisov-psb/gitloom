# Getting started

## Install

Gitloom needs **Git 2.34 or newer** on the `PATH` for everything that changes a repository (commit, checkout, fetch, …). Without Git you can still browse history; write actions are disabled and say why.

Download the package for your system from the [releases page](https://github.com/a-denisov-psb/gitloom/releases). `SHA256SUMS` lists the checksum of every file.

- **Windows (x64, x86):** run the `.msi` installer (asks for administrator rights) or unpack the portable `.zip` anywhere and start `gitloom.exe`. Until the binaries are signed, SmartScreen may warn: **More info** → **Run anyway**.
- **macOS 11 or newer (Apple silicon and Intel):** open the `.dmg` and drag Gitloom to Applications. The app is not notarized by Apple, so the first start is blocked: right click the app → **Open** → **Open**, or System Settings → Privacy & Security → **Open Anyway**.
- **Linux x64 (glibc 2.35 or newer):** make the AppImage executable (`chmod +x Gitloom-*.AppImage`) and run it. The `.tar.gz` contains the binary, a desktop entry (`io.github.a_denisov_psb.Gitloom.desktop`), AppStream metadata and an icon; copy the desktop entry to `~/.local/share/applications/` and the icon to `~/.local/share/icons/hicolor/256x256/apps/` to get a menu entry.

## Open a repository

- Start screen: **Open repository** (native folder dialog), a typed path, or a recent repository.
- **Ctrl+O** opens the folder dialog, **Ctrl+Shift+O** the list of recent repositories.
- From a terminal: `gitloom <path> [<path>...]` opens each path in its own tab. A subfolder opens its repository; an already open repository switches to its tab. On Windows the terminal returns to the prompt right away.

Gitloom remembers the open tabs, the active tab and the pane sizes and restores them on the next start. The start screen lists up to 20 recent repositories; a missing one can be removed from the list.

## The main window

![Main window](../screenshots/m5-main-dark-100.png)

| Area | What it shows |
|---|---|
| Tabs (top) | One tab per repository; **+** or Ctrl+T opens a new tab, Ctrl+W closes it, Ctrl+Tab / Ctrl+1…9 switch |
| Toolbar | Repository and branch, Undo/Redo, Pull/Push, Branch, Stash/Pop, Terminal, Preferences (gear) |
| Left panel | Local branches, remote branches per remote, tags, stashes, submodules; filter on top (Ctrl+Alt+F); Ctrl+J hides it |
| Graph | All commits with the lane graph, branch and tag pills, the "WIP" row for your uncommitted changes |
| Right panel | Details of the selected commit, or the staging area when the WIP row is selected; Ctrl+K hides it |
| Status bar | Messages, running operations (with Cancel), notices |

All dividers can be dragged. The diff of a file replaces the graph; **Esc** returns to the graph at the same position.

## Finding things

- **Ctrl+P** – command palette: type part of any action's name.
- **Ctrl+/** – list of all keyboard shortcuts with a filter.
- **Ctrl+F** – search commits by message, author, email or SHA.
- **Shift+F10** (or the Menu key) – context menu of the selected commit.
