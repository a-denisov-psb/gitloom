# Roadmap

## Current milestone

**M4 – Merge, rebase, conflicts** → [`milestones/M4-merge-rebase.md`](milestones/M4-merge-rebase.md)

## Vision

A fast, native, open-source Git client that GitKraken users can use without relearning anything: same layout, same interactions, same shortcuts. Leaner than current GitKraken: no accounts, no cloud, no AI, no integrations, no subscription.

## Milestones

| ID | Version | Name | Status |
|---|---|---|---|
| M0 | 0.1.0 | Layout shell | Done |
| M1 | 0.2.0 | Real repository (read-only) and lane graph | Done |
| M2 | 0.3.0 | Working copy: stage, commit, stash | Done |
| M3 | 0.4.0 | Branches and remotes | Done |
| M4 | 0.5.0 | Merge, rebase, conflicts, undo | In progress |
| M5 | 0.6.0 | Productivity parity | Planned |
| M6 | 1.0.0 | Release readiness | Planned |

Only the current milestone is planned in task-level detail. The next one is detailed when the current one is about to finish.

### M0 – Layout shell (done)

iced shell with tabs, toolbar, sidebar, graph table, staging and detail panel, diff placeholder, status bar. Sample data only.

### M1 – Real repository (read-only) and lane graph (done)

Open any local non-bare repository and see real history with a correct lane graph, refs, working tree status, commit details and read-only diffs. Session restore. No demo data left in the app.

### M2 – Working copy (done)

The daily loop without a terminal: file watcher, stage/unstage and discard by file, hunk and line, commit and amend, stash with stashes in the graph, undo/redo for commit, amend and discard. All writes go through the new `CliBackend`. Context menu and modal infrastructure moved here from M3. Details: [`milestones/M2-working-copy.md`](milestones/M2-working-copy.md).

### M3 – Branches and remotes (done)

Checkout (local, remote → tracking branch, detached HEAD), create/rename/delete branches and tags, fetch (manual and automatic), pull with mode selection, push with upstream setup and force-with-lease, upstream and remote management, context menus on commits, branches, tags and remotes, undo/redo for ref operations. Network operations show progress, can be cancelled and use Gitloom as askpass helper; credentials are never stored. Details: [`milestones/M3-branches-remotes.md`](milestones/M3-branches-remotes.md).

### M4 – Merge, rebase, conflicts

Merge and fast-forward, rebase and interactive rebase (pick, reword, squash, fixup, drop, reorder), cherry-pick (one or many), revert, reset soft/mixed/hard, a conflict resolution view (ours, theirs, editable output), continue/skip/abort for running operations, drag and drop of branches, multi-selection in the graph and undo/redo for all of them. Gitloom acts as Git's editor, so no command waits for a terminal. Details: [`milestones/M4-merge-rebase.md`](milestones/M4-merge-rebase.md).

### M5 – Productivity parity

- Command palette, commit search, file history and blame
- Hide and solo branches
- Proper icon set (SVG), avatars (opt-in, cached), light and dark theme, zoom
- Settings view, external diff/merge tool, open in terminal/file manager

### M6 – Release readiness (1.0)

- Automated release builds via GitHub Actions on every version tag (`v*.*.*`,
  including hotfixes), attached to a draft GitHub release with SHA256 checksums:
  - Linux: AppImage + `.tar.gz` (built on the oldest supported Ubuntu LTS for glibc compatibility)
  - macOS: universal binary (arm64 + x86_64 via `lipo`) as signed and notarized `.app` in `.dmg`
  - Windows x64: portable `.zip` + `.msi`, signed
  - Windows x86: portable `.zip` + `.msi`, best effort (a failing x86 build must not block the release)
  - Packaging tool candidate: `cargo-packager` (AppImage, dmg, msi from one config)
- Code signing: Apple Developer ID (notarization), Windows code signing certificate or cloud signing service
- Opt-in update check, crash logs, accessibility pass, user documentation

## Non-goals

- Pull request, issue tracker or Jira integrations
- Accounts, cloud workspaces, telemetry by default
- AI features
- Web or Electron based UI

## Ideas backlog

Own ideas that go beyond GitKraken. To be collected and assigned to milestones with the maintainer.

- **Git flow graph layout** (after M3): if git flow is initialized (`gitflow.*` config),
  render the graph like the classic git flow diagram: fixed lane groups from left to right
  (master, hotfix/*, release/*, develop, feature/*), one color per branch type.
  Branch membership is inferred from git flow config, first-parent chains of branch tips
  and merge commit messages (to recover deleted feature branches). Toggle in the toolbar;
  falls back to the default layout when git flow is not configured.
  Prepared in M2-15 via `LaneHints`.
