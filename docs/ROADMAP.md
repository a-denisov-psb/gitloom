# Roadmap

## Current milestone

**M6 – Release readiness (1.0)** → [`milestones/M6-release-readiness.md`](milestones/M6-release-readiness.md) (maintainer steps open; 1.0 line on `master`, ADR-039)

**M7 – Left panel tree, diff view modes and Git flow (1.1)** → [`milestones/M7-diff-modes-gitflow.md`](milestones/M7-diff-modes-gitflow.md) (implemented; release 1.1.0 from `release/1.1.0` after 1.0.0, ADR-040)

**M8 – Repository settings and tools (1.2)** → [`milestones/M8-repository-settings.md`](milestones/M8-repository-settings.md) (planned; runs on `develop`, ADR-040)

## Vision

A fast, native, open-source Git client that GitKraken users can use without relearning anything: same layout, same interactions, same shortcuts. Leaner than current GitKraken: no accounts, no cloud, no AI, no integrations, no subscription.

## Milestones

| ID | Version | Name | Status |
|---|---|---|---|
| M0 | 0.1.0 | Layout shell | Done |
| M1 | 0.2.0 | Real repository (read-only) and lane graph | Done |
| M2 | 0.3.0 | Working copy: stage, commit, stash | Done |
| M3 | 0.4.0 | Branches and remotes | Done |
| M4 | 0.5.0 | Merge, rebase, conflicts, undo | Done |
| M5 | 0.6.0 | Productivity parity | Done |
| M6 | 1.0.0 | Release readiness | In progress (maintainer steps open) |
| M7 | 1.1.0 | Left panel tree, diff view modes and Git flow | Implemented, release after 1.0.0 |
| M8 | 1.2.0 | Repository settings and tools | Planned |
| M9 | 1.3.0 | Graph columns, themes and syntax highlighting | Outline |

Only the current and the next milestone are planned in task-level detail; later milestones are outlines until they are next.

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

Product identity in one place (name "Gitloom" kept, ADR-033), license GPL-3.0-or-later with third-party notices and About dialog, app icon, crash reports (local only), opt-in update check, accessibility pass, packaging (AppImage, tar.gz, universal dmg, msi/zip for Windows x64 and x86) and a tag-driven release workflow, Windows signing via SignPath Foundation, macOS ad-hoc signature, public repository with the first release candidate, user documentation, a consolidated Windows walkthrough and the open GitKraken checks. Fixes for rc.2: no console window on Windows, working repository/branch pickers, remotes in sync with the Git configuration. Fixes for rc.3: context menu width, empty remote sections, file location and path, scrollbars, graph row gaps. Details: [`milestones/M6-release-readiness.md`](milestones/M6-release-readiness.md).

### M7 – Left panel tree, diff view modes and Git flow (1.1)

Branch folders, icons, highlighted checked-out branch and hidden-branch counter in the left panel like GitKraken; Hunk, Inline and Split diff views with staging in every mode; GitKraken's Git flow: initialize per repository (git-flow compatible config), GITFLOW section in the left panel, start and finish of features, releases and hotfixes with conflict resume and undo. Details: [`milestones/M7-diff-modes-gitflow.md`](milestones/M7-diff-modes-gitflow.md).

### M8 – Repository settings and tools (1.2)

Preferences group "Repository · <name>" (Identity, SSH key, Gitflow) like GitKraken's repo-specific preferences; identities (name and email) with a default kept in the global Git config; "Committing as …" in the commit panel with a per-repository choice written to the repository config; SSH key per repository via `core.sshCommand`; external editor for the repository (palette, Alt+E, toolbar) and single files (file context menus). Details: [`milestones/M8-repository-settings.md`](milestones/M8-repository-settings.md).

### M9 – Graph columns, themes and syntax highlighting (1.3, outline)

- Graph columns: drag column borders, double click for optimal width, column menu (gear in the header) to show/hide branch/tag, graph, message, author, date, SHA; replaces the automatic hiding of columns.
- Theme "Classic Dark": the palette gains style tokens (row tint in the lane color, filled ref pills with a line to the node, header bands, column separators, panel depth); Dark and Light keep their current look through token values.
- Syntax highlighting in diffs, blame, file history and the conflict view: syntect with the `two-face` syntax set (languages like VS Code's built-ins); whole old/new files are highlighted in the background and mapped to diff lines (needs M7-02), cached per blob, size limit, colors per theme, switch in Preferences → Appearance.

## Non-goals

- Pull request, issue tracker or Jira integrations
- Accounts, cloud workspaces, telemetry by default
- AI features
- Web or Electron based UI

## Ideas backlog

Own ideas and deferred items. To be assigned to milestones with the maintainer.

- **Git flow graph layout** (after M7; builds on the Git flow configuration model of M7-06): if git flow is initialized (`gitflow.*` config),
  render the graph like the classic git flow diagram: fixed lane groups from left to right
  (master, hotfix/*, release/*, develop, feature/*), one color per branch type.
  Branch membership is inferred from git flow config, first-parent chains of branch tips
  and merge commit messages (to recover deleted feature branches). Toggle in the toolbar;
  falls back to the default layout when git flow is not configured.
  Prepared in M2-15 via `LaneHints`.
- **Interactive rebase keeps prepared messages after a conflict** (M4 known limitation):
  today the remaining reword/squash messages are lost when a step stops for a conflict;
  Git then stops at those steps and Continue uses the prefilled original message.
- **Revert without changes** (M4 known limitation): Git fails with "nothing to commit";
  offer "Commit empty" like the empty cherry-pick dialog instead of showing Git's error.
- **Autostash conflict notice** (M4 known limitation): when re-applying an autostash after
  merge/rebase conflicts, show a dedicated notice that the stash was kept.
- **Toolbar picker dropdowns**: open the repository and branch lists as a dropdown below the pickers like GitKraken instead of the palette (M6-17).
- **Automatic identity**: choose the identity (M8) automatically by remote URL or path, like `includeIf`.
- **Signing key per identity** (GPG/SSH commit signing).
- **SSH key generator** in the Preferences.
- **Integrated terminal** instead of opening the system terminal.
- **macOS notarization** with an Apple Developer ID (removes the Gatekeeper prompt).
- **More packages**: Flatpak, `.deb`/`.rpm`, Homebrew cask, winget.
- **Automatic updates**: download and install instead of a notice.
- **Screen reader support** once iced exposes an accessibility tree.
