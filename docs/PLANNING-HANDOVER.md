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
| Maintainer | Decides, approves merges and releases | – |
| Planning chat (you) | Product decisions with the maintainer, roadmap, detailed milestone plans, reviews of Claude Code's results | Write or change code |
| Claude Code | Implements milestone tasks on feature branches, tests, keeps docs in sync | Change scope on its own |

### How planning works

1. Maintainer and planning chat decide scope and write the milestone file (`docs/milestones/Mx-*.md`): tasks, dependencies, acceptance criteria, UX parity references.
2. The planning chat hands the maintainer prompts for Claude Code (with full file contents, see section 3). The maintainer pastes them into Claude Code.
3. Claude Code implements task by task (`/next-task`, `/finish-task`), merges green features into `develop` and reports back in German.
4. The planning chat reviews the result (checkboxes, notes under each task, `git log`), adjusts the plan and prepares the next prompts.

The milestone files are the contract: Claude Code does not change scope on its own and records open points in `docs/HANDOVER.md` → "Open questions"; the planning chat answers them and updates the plan.

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
- Working title "Gitloom".

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
| License | MIT OR Apache-2.0 | Provisional, needs confirmation |
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
| M4 Merge, rebase, conflicts, undo | 0.5.0 (in progress, M4-14 open on 2026-10-26) | – | Merge/ff, rebase and interactive rebase, cherry-pick, revert, reset, conflict resolution view, continue/skip/abort, drag and drop, multi-selection, undo/redo; Gitloom as Git's editor | Shift+↑/↓ select ranges instead of parent/child navigation; reset uses a mode dialog instead of a submenu; hard reset is undoable via a snapshot; editor input is a message queue |
| M5 Productivity parity | 0.6.0 | – | Planned: palette, search, blame, icons, themes | – |
| M6 Release readiness | 1.0.0 | – | Planned: installers, signing, update check | – |

## 6. Pending changes

Items agreed in the first planning chat 2026-10-21; all applied in the repository:

- `LaneHints` preparation: done as M2-15 (`docs/ARCHITECTURE.md`, "Lane groups").
- Backlog entry "Git flow graph layout": in `docs/ROADMAP.md` → "Ideas backlog".

Add new agreed-but-not-yet-applied changes here.

## 7. Open decisions and verification items

Maintainer to-dos (manual checks only the maintainer can do: Windows runs, real credential prompts, keyboard input that does not reach the WSLg window) live in `docs/HANDOVER.md` → "Maintainer to-dos". Do not duplicate them here.

| # | Item | Status |
|---|---|---|
| 1 | Final project name ("Gitloom" is a working title) | Open |
| 2 | Confirm license (MIT OR Apache-2.0) | Open |
| 3 | Maintainer's own ideas beyond GitKraken for the backlog | Open, git flow layout is the first one |
| 4 | Git flow layout: backlog only, or its own milestone with a detailed plan? Which position in the roadmap? | Open |
| 5 | M4 known limitations (rebase conflict loses remaining prepared messages, revert without changes, autostash conflict without notice): keep or plan follow-ups? | Open (`docs/HANDOVER.md` → Open questions #9) |
| 6 | Verify in GitKraken whether the interactive rebase list shows the newest commit on top (Gitloom does) | Open, maintainer to-do M4-07 |

Questions raised by Claude Code during implementation are tracked in `docs/HANDOVER.md` → "Open questions"; check it at the start of every planning session.

## 8. Suggested next planning steps

1. Check M4 progress in the repo and review what Claude Code delivered.
2. Settle open decisions 4 and 5.
3. Collect the maintainer's own ideas; place them in the roadmap.
4. When M4 approaches its end: detail M5 to the same level as M4 (tasks, dependencies, acceptance criteria, dependencies to add, UX parity references).

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
