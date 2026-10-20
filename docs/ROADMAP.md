# Roadmap

## Current milestone

**M1 – Real repository and lane graph** → [`milestones/M1-real-repository.md`](milestones/M1-real-repository.md)

## Vision

A fast, native, open-source Git client that GitKraken users can use without relearning anything: same layout, same interactions, same shortcuts. Leaner than current GitKraken: no accounts, no cloud, no AI, no integrations, no subscription.

## Milestones

| ID | Version | Name | Status |
|---|---|---|---|
| M0 | 0.1.0 | Layout shell | Done |
| M1 | 0.2.0 | Real repository (read-only) and lane graph | Next |
| M2 | 0.3.0 | Working copy: stage, commit, stash | Planned |
| M3 | 0.4.0 | Branches and remotes | Planned |
| M4 | 0.5.0 | Merge, rebase, conflicts, undo | Planned |
| M5 | 0.6.0 | Productivity parity | Planned |
| M6 | 1.0.0 | Release readiness | Planned |

Only the current milestone is planned in task-level detail. The next one is detailed when the current one is about to finish.

### M0 – Layout shell (done)

iced shell with tabs, toolbar, sidebar, graph table, staging and detail panel, diff placeholder, status bar. Sample data only.

### M1 – Real repository (read-only) and lane graph

Open any local non-bare repository and see real history with a correct lane graph, refs, working tree status, commit details and read-only diffs. Session restore. No demo data left in the app.

### M2 – Working copy

- Stage and unstage by file, hunk and line; discard changes
- Commit, amend, commit message templates; commit box behavior like GitKraken
- Stash, pop, apply, drop; stashes shown in the graph
- File watcher with automatic refresh
- Introduce `CliBackend` for write operations (hooks, signing, credential helpers work as in plain Git)
- Undo for commit and discard (reflog based)

### M3 – Branches and remotes

- Checkout (local, remote → tracking branch), detached HEAD handling
- Create, rename, delete branches; tags
- Fetch, pull (fast-forward, merge, rebase), push including force-with-lease
- Ahead/behind indicators, upstream management
- Context menus in graph and sidebar
- Drag and drop a branch onto another branch → merge/rebase menu

### M4 – Merge, rebase, conflicts

- Merge, rebase, interactive rebase, cherry-pick (multi), revert, reset (soft, mixed, hard)
- Conflict resolution view (ours, theirs, output)
- Full undo/redo for supported actions

### M5 – Productivity parity

- Command palette, commit search, file history and blame
- Hide and solo branches
- Proper icon set (SVG), avatars (opt-in, cached), light and dark theme, zoom
- Settings view, external diff/merge tool, open in terminal/file manager

### M6 – Release readiness (1.0)

- Installers: Windows (MSI, winget), macOS (signed, notarized dmg), Linux (AppImage, Flatpak, deb)
- Opt-in update check, crash logs, accessibility pass, user documentation

## Non-goals

- Pull request, issue tracker or Jira integrations
- Accounts, cloud workspaces, telemetry by default
- AI features
- Web or Electron based UI

## Ideas backlog

Own ideas that go beyond GitKraken. To be collected and assigned to milestones with the maintainer.

- _(open)_
