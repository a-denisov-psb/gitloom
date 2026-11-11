# Roadmap

## Current milestone

**M9 – Fixes and consistency** → [`milestones/M9-fixes-and-consistency.md`](milestones/M9-fixes-and-consistency.md)

Single release line (ADR-042): everything from M7 on ships with **1.0.0**. Milestones end without a release; Claude Code continues with the next planned milestone. 1.0.0 (M6-15) is released from `develop` after M13 and the maintainer's full run of `docs/RELEASE-CHECKLIST.md`, only when the maintainer asks.

## Vision

A fast, native, open-source Git client that GitKraken users can use without relearning anything: same layout, same interactions, same shortcuts. Leaner than current GitKraken: no accounts, no cloud, no AI, no integrations, no subscription.

Safe to use for beginners: every local action can be undone and lands exactly in the state before (ADR-043), and no situation leaves the user without a visible way back (ADR-044).

## Milestones

| ID | Version | Name | Status |
|---|---|---|---|
| M0 | 0.1.0 | Layout shell | Done |
| M1 | 0.2.0 | Real repository (read-only) and lane graph | Done |
| M2 | 0.3.0 | Working copy: stage, commit, stash | Done |
| M3 | 0.4.0 | Branches and remotes | Done |
| M4 | 0.5.0 | Merge, rebase, conflicts, undo | Done |
| M5 | 0.6.0 | Productivity parity | Done |
| M6 | 1.0.0 | Release readiness | Done except M6-10 (SignPath), M6-13 (checklist run) and M6-15 (release) |
| M7 | 1.0.0 | Left panel tree, diff view modes and Git flow | Done |
| M8 | 1.0.0 | Repository settings and tools | Done |
| M9 | 1.0.0 | Fixes and consistency | Current |
| M10 | 1.0.0 | Clone, init and Git settings | Outline |
| M11 | 1.0.0 | Graph columns, themes and syntax highlighting | Outline |
| M12 | 1.0.0 | Workflow polish | Outline |
| M13 | 1.0.0 | Git flow graph layout | Outline |

The planning chat details the next milestones ahead of time so that Claude Code never waits (ADR-042).

### M0 – Layout shell (done)

iced shell with tabs, toolbar, sidebar, graph table, staging and detail panel, diff placeholder, status bar. Sample data only.

### M1 – Real repository (read-only) and lane graph (done)

Open any local non-bare repository and see real history with a correct lane graph, refs, working tree status, commit details and read-only diffs. Session restore. No demo data left in the app.

### M2 – Working copy (done)

The daily loop without a terminal: file watcher, stage/unstage and discard by file, hunk and line, commit and amend, stash with stashes in the graph, undo/redo for commit, amend and discard. All writes go through the new `CliBackend`. Context menu and modal infrastructure moved here from M3. Details: [`milestones/M2-working-copy.md`](milestones/M2-working-copy.md).

### M3 – Branches and remotes (done)

Checkout (local, remote → tracking branch, detached HEAD), create/rename/delete branches and tags, fetch (manual and automatic), pull with mode selection, push with upstream setup and force-with-lease, upstream and remote management, context menus on commits, branches, tags and remotes, undo/redo for ref operations. Network operations show progress, can be cancelled and use Gitloom as askpass helper; credentials are never stored. Details: [`milestones/M3-branches-remotes.md`](milestones/M3-branches-remotes.md).

### M4 – Merge, rebase, conflicts (done)

Merge and fast-forward, rebase and interactive rebase (pick, reword, squash, fixup, drop, reorder), cherry-pick (one or many), revert, reset soft/mixed/hard, a conflict resolution view (ours, theirs, editable output), continue/skip/abort for running operations, drag and drop of branches, multi-selection in the graph and undo/redo for all of them. Gitloom acts as Git's editor, so no command waits for a terminal. Details: [`milestones/M4-merge-rebase.md`](milestones/M4-merge-rebase.md).

### M5 – Productivity parity (done)

Action registry with shortcut list (Ctrl+/), settings view, light and dark theme, zoom and fullscreen, SVG icon set, command palette, commit search, hide and solo branches, file history and blame, opt-in avatars, external diff/merge tools, open in terminal and file manager. Details: [`milestones/M5-productivity.md`](milestones/M5-productivity.md).

### M6 – Release readiness (1.0)

Product identity in one place (name "Gitloom" kept, ADR-033), license GPL-3.0-or-later with third-party notices and About dialog, app icon, crash reports (local only), opt-in update check, accessibility pass, packaging (AppImage, tar.gz, universal dmg, msi/zip for Windows x64 and x86) and a tag-driven release workflow, Windows signing via SignPath Foundation, macOS ad-hoc signature, public repository with release candidates rc.1 … rc.4, user documentation, a consolidated Windows walkthrough and the open GitKraken checks. Open: M6-10 (SignPath), M6-13 (checklist run, now the final check before 1.0.0) and M6-15 (1.0.0, after M13). Details: [`milestones/M6-release-readiness.md`](milestones/M6-release-readiness.md).

### M7 – Left panel tree, diff view modes and Git flow (done)

Branch folders, icons, highlighted checked-out branch and hidden-branch counter in the left panel like GitKraken; Hunk, Inline and Split diff views with staging in every mode; GitKraken's Git flow: initialize per repository (git-flow compatible config), GITFLOW section in the left panel, start and finish of features, releases and hotfixes with conflict resume and undo. Ships with 1.0.0 (ADR-042). Details: [`milestones/M7-diff-modes-gitflow.md`](milestones/M7-diff-modes-gitflow.md).

### M8 – Repository settings and tools (done)

Preferences group "Repository · <name>" (Identity, SSH key, Gitflow); identities with a default kept in the global Git config; "Committing as …" in the commit panel; SSH key per repository via `core.sshCommand`; external editor for the repository and single files. Ships with 1.0.0 (ADR-042). Details: [`milestones/M8-repository-settings.md`](milestones/M8-repository-settings.md).

### M9 – Fixes and consistency (current)

Stash pop/apply that stops in conflicts becomes a normal conflict with a banner, "Drop stash" and Undo back to the exact state before; discard and reset work while files are conflicted; reset on the HEAD commit; sharp icons at fractional zoom; left panel order and GITFLOW content like GitKraken; character counter inside the summary field. Details: [`milestones/M9-fixes-and-consistency.md`](milestones/M9-fixes-and-consistency.md).

### M10 – Clone, init and Git settings (outline)

- Start screen like GitKraken's New Tab: Open a repo, Clone a repo, Start a local repo; recent repositories with name and path; Customize (Preferences, Manage identities); Resources (user guide, report an issue; links open the browser). No hosted-repo creation, no boards.
- Clone: URL, parent folder, name, full path preview, progress and cancel, askpass, submodules recursive; opens in a tab.
- Init ("Start a local repo"): name, folder, default branch name (`init.defaultBranch`, Preferences → General), optional README and `.gitignore`; opens in a tab.
- Commit signing (GPG and SSH): Preferences → Signing with key choice, format, GPG program, "Sign commits/tags by default" (Git config); signature status in the commit details. No key generator.
- Preferences → Repository → Commit: commit template (prefills summary and description, respects and writes `commit.template`, "Remove comments") and the message prompt template. "Copy commit message prompt…" (staged area context menu, palette): modal with preview, size, truncation note and Copy; content: instruction, commit template, branch, last 5–10 summaries, file list with +/−, staged diff with a size limit. No AI inside Gitloom.
- Small options: auto-prune on fetch, delete `.orig` files after merging (`mergetool.keepBackup`), `core.longpaths` (Windows), toolbar labels on/off.

### M11 – Graph columns, themes and syntax highlighting (outline)

- Graph columns: drag column borders, double click for optimal width, column menu (gear in the header) to show/hide branch/tag, graph, message, author, date, SHA; replaces the automatic hiding of columns.
- Theme "Classic Dark": the palette gains style tokens (row tint in the lane color, filled ref pills with a line to the node, header bands, column separators, panel depth); Dark and Light keep their current look through token values.
- Stash node like GitKraken: dashed square with a stash icon in its own lane right of the base commit, dotted connector; an empty stash message shows as "WIP on <branch>" (graph and STASHES section).
- Syntax highlighting in diffs, blame, file history and the conflict view: syntect with the `two-face` syntax set; whole old/new files highlighted in the background and mapped to diff lines, cached per blob, size limit, colors per theme.
- Preferences → Editor like GitKraken: font, font size, tab size, line numbers, word wrap, syntax highlighting on/off.

### M12 – Workflow polish (outline)

- File lists: a click on the selected file deselects it (the diff closes); hover shows only Stage/Unstage, per-file Discard moves to the context menu (deviation, maintainer decision 2026-11-10).
- Remotes: "Open on GitHub/GitLab/Bitbucket" (other hosts "Open in browser") in the remote context menu and the palette; repository start page only, URL derived offline.
- Conflict view details: "conflict 1 of N" with arrows, side headers "Commit <sha> on <branch>", panel header "Merging X into Y", "Resolved Files" and "Mark all resolved".
- Submodules: list with status (not initialized, out of date, modified), Update/Initialize, out-of-date hint after checkout and pull, double click opens in a new tab, section hidden without submodules. No add/remove.
- Errors as non-modal toasts (bottom left like GitKraken) with plain-language text, Details and Copy; a modal only when a decision is needed; plain-language texts for common Git errors.
- Undo coverage (ADR-043): stash push and drop, stage/unstage, "mark resolved", the local part of pull, remotes and upstream, repository settings in the Git config.

### M13 – Git flow graph layout (outline)

When Git flow is initialized, an optional layout draws the graph like the classic Git flow diagram: fixed lane groups from left to right (production, hotfix, release, development, feature/other), one color per type, built on `LaneHints` (M2-15).

- Membership of commits not on a live branch: production and development are their first-parent chains (production wins where both meet); every other first-parent segment is classified by where it forks off and where it is merged: from production into production (and development) = hotfix; from development into production (and development) = release; from development only into development = feature; unmerged = branch prefix, else feature.
- Priority: live branch prefix > Git's default merge message ("Merge branch 'feature/x' into develop") > topology > feature. "Merge branch 'develop' of …" counts as development.
- Group widths over the whole history (as `LaneHints` does), never per viewport. Unknown segments go to the feature group in a neutral color with the tooltip "Branch type unknown".
- Toggle in the graph column menu (M11), the palette and Preferences → Repository → Gitflow; disabled with a reason when Git flow is not initialized. Optional: names of deleted branches from merge messages as faded labels. Gitloom extension (UX-PARITY).

## Non-goals

- Anything that needs a hosting provider's API or an account: pull requests, issues, boards, CI status, creating hosted repositories. Links that open the provider's website are fine.
- Accounts, cloud workspaces, telemetry by default
- AI features (a copyable prompt for the user's own tools is fine, M10)
- Web or Electron based UI

## Ideas backlog

Own ideas and deferred items. To be assigned to milestones with the maintainer.

- **Interactive rebase keeps prepared messages after a conflict** (M4 known limitation).
- **Revert without changes** (M4 known limitation): offer "Commit empty" instead of Git's error.
- **Autostash conflict notice** (M4 known limitation).
- **Toolbar picker dropdowns** like GitKraken instead of the palette lists (M6-17).
- **Automatic identity** by remote URL or path (`includeIf`).
- **Signing key per identity**.
- **SSH and GPG key generators** in the Preferences; "Use local SSH agent" switch.
- **Commit message generator command**: run a user-defined command with the staged diff on stdin and use its output as a suggestion (same prompt builder as M10; no AI inside Gitloom).
- **Undo history across restarts** and a list of the last actions in the palette.
- **Git LFS** (initialize, status).
- **Desktop notifications**.
- **Integrated terminal** instead of opening the system terminal.
- **macOS notarization** with an Apple Developer ID.
- **More packages**: Flatpak, `.deb`/`.rpm`, Homebrew cask, winget.
- **Automatic updates**: download and install instead of a notice.
- **Screen reader support** once iced exposes an accessibility tree.
