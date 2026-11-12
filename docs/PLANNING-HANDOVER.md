# Gitloom – Planning Handover

Handover from the first planning chat (claude.ai) to a new planning chat in Claude Cowork. Initial state: 2026-10-21. Maintained in the repository since 2026-10-26; sections 5 and 6 are updated at every release (`/release`).

## 1. How to use this document

You are the planning partner for Gitloom. You plan with the maintainer; Claude Code implements in the repository.

Setup in Cowork:

1. The maintainer grants you access to the local repository folder `gitloom/`.
2. Read in this order: this file → `docs/HANDOVER.md` → `docs/ROADMAP.md` → current milestone file in `docs/milestones/` → `docs/DECISIONS.md` → `docs/ARCHITECTURE.md` → `docs/UX-PARITY.md` → `CLAUDE.md`.
3. Check actual progress yourself: the checkboxes in the milestone file and `git log` on `develop` are the source of truth, not section 5 of this file.

## 2. Roles and workflow

| Who | Does | Does not |
|---|---|---|
| Maintainer | Decides scope, reviews milestone reports, does maintainer-only actions | Approve single tasks, merges or releases (ADR-034) |
| Planning chat (you) | Product decisions with the maintainer, roadmap, detailed milestone plans, reviews of Claude Code's results | Write or change code |
| Claude Code | Works through a whole milestone autonomously (one chat per milestone): tasks, plan-gap decisions, merges, release | Extend scope with new features |

### How planning works

1. Maintainer and planning chat decide scope and write the milestone file (`docs/milestones/Mx-*.md`): tasks, dependencies, acceptance criteria, UX parity references.
2. The planning chat hands the maintainer prompts for Claude Code (with full file contents, see section 3). The maintainer pastes them into Claude Code.
3. The maintainer starts one Claude Code chat per milestone. Claude Code implements task by task (`/next-task`, `/finish-task`) without asking back, merges green features into `develop`, reports one line per task and releases the milestone itself (`/release`, ADR-034) with a final report.
4. The planning chat reviews the milestone report (checkboxes, deviation notes, new ADRs, `git log`), adjusts the plan and prepares the next milestone.

The milestone files are the contract: Claude Code does not extend scope. It decides plan gaps itself (most conservative, GitKraken-closest variant) and documents them as ADR, milestone note or `docs/UX-PARITY.md` deviation; it stops only for the reasons in ADR-034. Skipped maintainer-only tasks land in `docs/HANDOVER.md` → "Maintainer to-dos". The planning chat reviews these decisions after the milestone.

### Rules for editing files

- You only edit planning documents: `docs/ROADMAP.md`, `docs/milestones/*`, `docs/DECISIONS.md`, `docs/UX-PARITY.md`, `docs/ARCHITECTURE.md`, `docs/HANDOVER.md` → "Open questions".
- Never edit code, `CLAUDE.md`, `.claude/`, scripts or Cargo files without the maintainer's explicit request.
- Before editing, ask the maintainer whether Claude Code is in the middle of a task. Claude Code works on feature branches; uncommitted changes from you would end up in its branch.
- After editing, the maintainer tells Claude Code: "Commit the planning changes in docs/ on `feature/plan-<topic>` and merge into develop after my approval." (`/next-task` refuses to start on a dirty working tree, which is a useful guard.)
- Keep all repository content in English.

## 3. Maintainer preferences

- Communicate in German. Short and precise, most important information first.
- Prompts for Claude Code always with full file contents (no "change X somewhere", no diffs the maintainer has to apply by hand).
- Bullets instead of prose where possible; tables for comparisons and key-value info; compact cells.
- No intro phrases, no repeated summaries at the end.
- When something is hard to understand: explain with analogies, not long text.
- Ambiguous requests: offer clickable options (interactive choice tool) instead of open questions.
- Documents always as Markdown, never Word/Excel/PowerPoint.
- Code, identifiers, comments, README and project docs always in English (all repository content).
- Copyable content (commands, config, templates) always in code blocks.
- Commands with placeholders: only ask for values not yet known; one choice question per placeholder with two realistic dummy values.

## 4. Product summary

- Open-source, cross-platform Git client in Rust that looks and behaves like (older) GitKraken, without subscription and without bloat.
- **Hard requirement:** layout and interaction must match GitKraken so long-time users feel at home immediately. Colors and icons are free; no GitKraken name, logo or assets.
- Native UI, no webview (performance and footprint).
- Name "Gitloom" (kept for 1.0, ADR-033).

### Key decisions (details in `docs/DECISIONS.md`)

| Topic | Decision | Why |
|---|---|---|
| UI | Rust + iced 0.14 | Native, no webview; iced chosen over egui (cleaner architecture) and Slint (license) |
| Git reads | gitoxide (`gix` 0.88) | Fast, no external binary needed |
| Git writes | `git` CLI (from M2) | Hooks, signing, credentials, SSH behave exactly like Git |
| Threading | `ThreadSafeRepository` + `spawn_blocking`, UI never blocks | gix `Repository` is not `Sync` |
| Graph | Pure crate `git-graph`, Kahn ordering, stable lanes, per-row segments | Testable, virtualizable |
| Demo data | Only in tests; unfinished actions disabled with tooltip | App must not look like a demo |
| Branching | Git flow: `master` releases, `develop` integration, `feature/<task-id>-<slug>` | Maintainer's choice |
| License | GPL-3.0-or-later | Switched in M6-03 (ADR-008) |
| Privacy | No telemetry, no accounts, offline by default | Scope |

### Verified technical facts

| Item | Fact |
|---|---|
| iced | 0.14.0, needs Rust ≥ 1.88 (MSRV of the project) |
| gix | 0.88, `default-features = false` requires the `sha1` feature explicitly |
| Other crates | rfd 0.17, directories 6.0, serde 1.0, tokio via iced feature |
| Build | Works with Rust 1.91; first debug build takes minutes |
| Double click | Detected in `App::update` with a 400 ms timer (iced buttons swallow events) |
| Shortcuts | Taken from GitKraken's official shortcut docs (April 2026), listed in `docs/UX-PARITY.md` |

## 5. Progress and milestone history

The table is a snapshot; the milestone files and `git log` are authoritative. `/release` adds one line per released milestone.

### Milestone history

One line per milestone: version, date, scope, notable deviations.

| Milestone | Version | Date | Scope | Notable deviations |
|---|---|---|---|---|
| M0 Layout shell | 0.1.0 | 2026-10-20 | Complete GitKraken-like layout with sample data (tabs, toolbar, sidebar, graph table, staging, details, diff placeholder, status bar, resizable panes); tooling, CI workflow, Claude Code setup | – |
| M1 Real repository (read-only) + lane graph | 0.2.0 | 2026-10-21 | `git-graph` (topo order, lanes), `GixBackend` (log/refs/stashes/status/diffs), async loading, open flow, session restore, virtualized graph, keyboard, diff view, demo data removed, performance work | Start screen path input as fallback for missing folder dialogs (ADR-010); J/K direction confirmed by maintainer |
| M2 Working copy | 0.3.0 | 2026-10-23 | File watcher, stage/unstage/discard by file, hunk and line, commit and amend, stashes in the graph, undo/redo, `CliBackend`, context menu and modal infrastructure (moved from M3), `LaneHints` (M2-15) | Discard asks for confirmation also for hunks and lines; "Discard all" asks whether to discard staged changes too; partial clones show renames as delete + add when blobs are missing |
| M3 Branches and remotes | 0.4.0 | 2026-10-25 | Checkout, branch/tag create/rename/delete, fetch (manual + auto), pull modes, push incl. force-with-lease, upstream and remote management, context menus, undo/redo for ref operations, progress + cancel, Gitloom as askpass helper | Askpass mode detected via `GITLOOM_ASKPASS` env (ADR-019); credentials never stored |
| M4 Merge, rebase, conflicts, undo | 0.5.0 | 2026-10-26 | Merge/ff, rebase and interactive rebase, cherry-pick, revert, reset, conflict resolution view, continue/skip/abort, drag and drop, multi-selection, undo/redo; Gitloom as Git's editor | Shift+↑/↓ select ranges instead of parent/child navigation; reset uses a mode dialog instead of a submenu; hard reset is undoable via a snapshot; editor input is a message queue; released with keyboard parts and the Windows walkthrough still open (maintainer to-dos); three known limitations moved to the backlog |
| M5 Productivity parity | 0.6.0 | 2026-11-04 | Action registry and shortcut list, settings, light/dark theme, zoom and fullscreen, Lucide icons, command palette, commit search, hide/solo branches, file history and blame, opt-in avatars, external diff/merge tools, terminal and file manager, end-to-end check | Released with the Windows and keyboard to-dos still open (bundled in M6-13); GitHub Actions did not run from 2026-10-26 until the billing was fixed, CI of `develop` verified before the release |
| Hotfix | 0.6.1 | 2026-11-06 | Network cancel could hang (v0.6.0 Windows CI timed out) | – |
| M6 release candidate 2 | 1.0.0-rc.2 | 2026-11-09 | M6-16 no console window on Windows, M6-17 toolbar pickers, M6-18 remotes in sync with the Git configuration | M6-17 pickers open the palette lists instead of a dropdown (UX-PARITY deviation, dropdown in the backlog) |
| M6 release candidate 3 | 1.0.0-rc.3 | 2026-11-10 | M6-19 context menu width (backport), M6-20 empty remote sections, M6-21 file location and path, M6-22 scrollbars, M6-23 graph row gaps | – |
| M6 release candidate 4 | 1.0.0-rc.4 | 2026-11-10 | Toolbar buttons center icon and label (maintainer finding) | Tag pushed by the maintainer |
| M9 Fixes and consistency | 1.0.0 (unreleased) | 2026-11-11 | Stash pop/apply conflicts with banner, undo/redo by snapshot and Drop stash; discard of conflicted files; reset on HEAD; unmerged index entries recorded around resets; sharp icons at fractional scales; left panel order and GITFLOW tree like GitKraken; summary counter inside the field; undo list with "Cannot be undone" | M9-01 and M9-02 on one branch (the outcome travels in the undo entry); the banner's Drop stash deletes without a second confirmation; GITFLOW keeps the remainder after the prefix as one label; icon sharpness on wgpu still to confirm on Windows (checklist 7c) |
| M11 Graph columns, themes and syntax highlighting | 1.0.0 (unreleased) | 2026-11-11 | Style tokens (an extra theme built and removed again, ADR-054), column widths with horizontal scrolling and gear menu, stash node, Preferences → Editor, word wrap, syntax highlighting (syntect + two-face), tree order and tag folders, start buttons, Preferences modal, panel toggles, README screenshot fixture | Column model changed by planning update 3 (every column own width, horizontal scrolling); OneHalfDark/GitHub as code themes (no VS Dark+ in two-face); pure Rust regex engine (1 MB highlight 3.4 s in the background) |
| M10 Clone, init and Git settings | 1.0.0 (unreleased) | 2026-11-11 | New Tab start screen, clone, init with `.gitignore` templates, default branch name, signing settings and signature badge, commit template, commit message prompt, Templates page with stash message template, prune/.orig/long paths/toolbar labels, external editor per repository | Template's first line now prefills the summary, apply/remove-comments default on; SSH signatures without allowed signers file read "key not available"; Preferences → Templates also holds the prompt (planning update 1) |
| M12 Workflow polish | 1.0.0 (unreleased) | 2026-11-12 | File lists deselect, toasts and plain-language errors, remote links, conflict navigation, operation state like GitKraken, submodules, undo for the remaining local actions, stash folders and rename, multi-selection and bulk delete, palette settings, commit prompt button, WSL notice and trust prompt | Panel toggles moved to the status bar, Classic Dark removed (ADR-054), planning update 4 (M13-06, M13-07) |
| M6 Release readiness | 1.0.0 | in progress | Done 2026-11-06 by Claude Code: identity, GPL-3.0-or-later with cargo-deny and notices, About dialog, app icon, crash reports, opt-in update check, accessibility pass, packaging (cargo-packager), release workflow (dry run green), user guide; public-repo files and the release checklist prepared | Open for the maintainer: M6-10 SignPath, M6-11 approval, M6-13 checklist run, M6-14 rc.1 (tag and going public), M6-15. MSI installs per machine (ADR-031); "About" in palette and Preferences (no gear menu); headless UI checks with `iced_test` (ADR-035) because the Windows desktop was locked |

## 6. Pending changes

Items agreed in the first planning chat 2026-10-21; all applied in the repository:

- `LaneHints` preparation: done as M2-15 (`docs/ARCHITECTURE.md`, "Lane groups").
- Backlog entry "Git flow graph layout": in `docs/ROADMAP.md` → "Ideas backlog".
- M5 detailed plan (2026-10-26): `docs/milestones/M5-productivity.md`, ADR-023 … ADR-027, ROADMAP backlog entries.
- M6 detailed plan (2026-11-03): `docs/milestones/M6-release-readiness.md`, ADR-028 … ADR-030, ADR-008/016 updated, ROADMAP backlog entries.

- rc.2 scope, M7 left panel tree, M8/M9 outlines (2026-11-09): M6-17, M6-18, M7-14 in the milestone files, ROADMAP M8/M9, backlog updated.
- rc.3 fixes and M7 follow-ups (2026-11-10): M6-19 … M6-23 on the 1.0 line, M7-15 … M7-18 on `develop`.
- M8 detailed plan and the 1.1 line (2026-11-10): `docs/milestones/M8-repository-settings.md`, ADR-040, ADR-041, ROADMAP, UX-PARITY rows and deviations; `release/1.1.0` branched from `develop`.
- Single release line, undo principle and roadmap M9–M13 (2026-11-11): ADR-042 … ADR-044, ROADMAP, `docs/milestones/M9-fixes-and-consistency.md`, CLAUDE.md and `.claude/commands/` adjusted; `release/1.1.0` removed.

- 2026-11-11 planning update 3: graph column widths with horizontal scrolling (M11-16, replaces the message-fills rule of M11-02), commit prompt button (M12-13), WSL notice, polling watcher and trust prompt (M12-14), ADR-053 remote backend readiness; WSL mode and SSH remote in the ideas backlog.
- 2026-11-11 planning update: tree order branches first (M11-11), Templates page and stash template (M10-10), external editor per repository (M10-11), Preferences modal, panel toggles, README screenshot, stash folders and rename, bulk delete, palette settings, M14 commit details with ADR-051 (only unpushed commits editable), custom commands after 1.0.0.

Add new agreed-but-not-yet-applied changes here.

## 7. Open decisions and verification items

Maintainer to-dos (manual checks only the maintainer can do: Windows runs, real credential prompts, keyboard input that does not reach the WSLg window) live in `docs/HANDOVER.md` → "Maintainer to-dos". Do not duplicate them here.

| # | Item | Status |
|---|---|---|
| 1 | Final project name ("Gitloom" is a working title) | decided 2026-11-05: keep "Gitloom" despite the collision with the GitLoom product (ADR-033, supersedes the rename decision of 2026-11-03); repository `github.com/a-denisov-psb/gitloom` |
| 2 | Confirm license (MIT OR Apache-2.0) | decided 2026-11-03: GPL-3.0-or-later (ADR-008), switch in M6-03 |
| 3 | Maintainer's own ideas beyond GitKraken for the backlog | Open, git flow layout is the first one |
| 4 | Git flow layout: backlog only, or its own milestone with a detailed plan? Which position in the roadmap? | decided 2026-10-26: backlog, after 1.0; 2026-11-06: Git flow start/finish planned in M7 (ADR-037), the graph layout stays in the backlog; 2026-11-11: graph layout planned as M13 |
| 5 | M4 known limitations (rebase conflict loses remaining prepared messages, revert without changes, autostash conflict without notice): keep or plan follow-ups? | decided 2026-10-26: backlog (ROADMAP) |
| 6 | Verify in GitKraken whether the interactive rebase list shows the newest commit on top (Gitloom does) | Open, maintainer to-do M4-07 |
| 7 | Profiles like GitKraken's (workspace with own tabs and settings) or identities only? | decided 2026-11-09: identities only (name and email per repository), M8 |
| 8 | Order after M7 | decided 2026-11-09: M8 repository settings and tools, then M9 graph columns, themes and syntax highlighting |
| 9 | Intermediate releases before 1.0.0? | decided 2026-11-11: none (ADR-042); everything is tested once at the end |
| 10 | AI features | decided 2026-11-11: none (non-goal); only a copyable commit message prompt (M10) |
| 11 | Provider integrations (issues, boards, CI, pull requests) | decided 2026-11-11: none; only "Open on <provider>" links (M12) |
| 12 | Undo and beginners | decided 2026-11-11: undo everything local (ADR-043), no dead ends (ADR-044) |

Questions raised by Claude Code during implementation are tracked in `docs/HANDOVER.md` → "Open questions"; check it at the start of every planning session.

## 8. Suggested next planning steps

1. M9 is done; M10 runs on `develop`, M11 … M14 are planned (ADR-042).
2. After M14: the maintainer runs the whole release checklist on Windows and answers the open GitKraken items; then 1.0.0 (M6-15).
3. M6-10 (SignPath) can happen any time before 1.0.0.

## 9. Document map

| File | Content | Edited by |
|---|---|---|
| `CLAUDE.md` | Rules for Claude Code | Maintainer request only |
| `docs/PLANNING-HANDOVER.md` | This file: state for the planning chat | Claude Code (at release), planning |
| `docs/HANDOVER.md` | State for Claude Code, maintainer to-dos, open questions | Claude Code, planning (open questions) |
| `docs/ROADMAP.md` | Milestones, non-goals, ideas backlog, current milestone pointer | Planning |
| `docs/milestones/Mx-*.md` | Tasks with checkboxes per milestone | Planning (plan), Claude Code (checkboxes, notes) |
| `docs/ARCHITECTURE.md` | Crates, threading, graph layout spec | Planning + Claude Code |
| `docs/DECISIONS.md` | ADRs | Planning + Claude Code |
| `docs/UX-PARITY.md` | GitKraken layout, interactions, shortcuts, deviations | Planning + Claude Code |
| `CONTRIBUTING.md` | Setup, git flow, commit style | Rarely |
| `CHANGELOG.md` | User-visible changes | Claude Code |
