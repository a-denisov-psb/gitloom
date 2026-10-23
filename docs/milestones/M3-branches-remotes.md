# M3 – Branches and remotes

Target version: **0.4.0**

## Goal

Everything around branches, tags and remotes works from the UI like in GitKraken: checkout, create, rename and delete branches and tags, fetch, pull and push with progress, authentication prompts, upstream and remote management, and context menus on commits and branches.

## Scope

In scope:

- Network operations with progress and cancel, authentication prompts (askpass)
- Fetch (manual and automatic), pull with mode selection, push incl. upstream setup and force-with-lease
- Checkout of local branches, remote branches (tracking branch) and commits (detached HEAD)
- Create, rename, delete branches (local and remote); create, delete, push tags
- Upstream and remote management
- Context menus for commits, branches, tags and remotes
- Undo/redo for checkout, branch and tag operations

Out of scope (later):

- Merge, rebase, interactive rebase, cherry-pick, revert, reset, conflict resolution → M4 (shown disabled in menus)
- Drag and drop of branches → M4 (moved from M3, because its menu consists of merge/rebase actions)
- Hide/solo branches, search, command palette, in-app terminal → M5
- Hosting integrations (pull requests, issues) → non-goal
- Storing credentials in Gitloom → never; Git credential helpers do that

## Scope changes against the roadmap

- Drag and drop moves from M3 to M4.

## Milestone acceptance

- [ ] Against a local bare remote and against the real GitHub remote of this repo: fetch, pull (all modes), push (incl. new branch with upstream), create/rename/delete branch, tag create/push/delete — all from the UI, results identical to the CLI.
- [ ] HTTPS with a credential manager and SSH with a passphrase-protected key work on Windows and Linux (maintainer check); a missing credential shows the askpass dialog instead of hanging.
- [ ] Every network operation shows progress and can be cancelled; the UI never blocks.
- [ ] Context menus on commits, branches, tags, remotes and stashes are complete; M4 actions are disabled with tooltip.
- [ ] `./scripts/check.sh` green on all CI platforms, docs and CHANGELOG up to date.
- [ ] Released as `v0.4.0` (M3-15).

## Decisions taken in planning

| Topic | Decision |
|---|---|
| Auto-fetch | Enabled by default, every 5 minutes, configurable in the session/settings file (UI in M5). Fetches all remotes of open tabs, only while the window is open. Never runs in parallel with a user operation. |
| Checkout with conflicting local changes | Dialog: "Stash and checkout" or "Cancel". The stash is not popped automatically. |
| Credentials | Never stored by Gitloom. Git credential helpers and ssh-agent are used as configured; Gitloom only answers askpass prompts. |
| Branch button / Ctrl+B | Inline name input at the selected commit (GitKraken); Enter creates the branch and checks it out. |

## Task overview

| ID | Task | Depends on |
|---|---|---|
| M3-01 | Network operation runner: progress, cancel | – |
| M3-02 | Authentication: askpass helper | M3-01 |
| M3-03 | Fetch and auto-fetch | M3-01 |
| M3-04 | Pull | M3-03 |
| M3-05 | Push | M3-01 |
| M3-06 | Checkout | – |
| M3-07 | Create branch | M3-06 |
| M3-08 | Rename and delete branches | M3-05 |
| M3-09 | Tags | M3-05 |
| M3-10 | Upstream and remote management | M3-05 |
| M3-11 | Context menus | M3-06 … M3-10 |
| M3-12 | Undo/redo for refs | M3-06 … M3-09 |
| M3-13 | Detached HEAD polish | M3-06, M3-07 |
| M3-14 | End-to-end check and docs | all others |
| M3-15 | Release v0.4.0 | all |

---

## Tasks

### [ ] M3-01 Network operation runner: progress, cancel

- Long-running Git commands (`fetch`, `pull`, `push`, `clone` later) run with `--progress`; stderr is parsed incrementally into phase + percentage (e.g. "Receiving objects: 45%").
- Status bar shows operation name, phase, percentage and a Cancel button. Multiple tabs: each shows its own.
- Cancel kills the process and all children (Windows: job object or process tree kill; Unix: process group). After cancel: refresh, message "Cancelled", no error dialog.
- Network operations count as write operations (ADR-013: one at a time per repo). A user-started operation cancels a running auto-fetch and then runs.

Acceptance:
- Unit tests for progress parsing with recorded stderr samples (Git 2.34 and current).
- Integration test: cancelling a fetch from a slow local remote (e.g. large generated repo) leaves no process behind.

### [ ] M3-02 Authentication: askpass helper

- Gitloom acts as its own askpass program: `gitloom --askpass "<prompt>"` opens a small modal window (iced), prints the answer to stdout, exit code 1 on cancel. Password fields are masked.
- Every Git network command gets `GIT_ASKPASS` and `SSH_ASKPASS` pointing to the running executable, `SSH_ASKPASS_REQUIRE=force` (OpenSSH ≥ 8.4), and keeps `GIT_TERMINAL_PROMPT=0`.
- Credential helpers (Git Credential Manager, osxkeychain, libsecret) and ssh-agent keep working unchanged; askpass is only the fallback.
- Host key confirmation prompts ("Are you sure you want to continue connecting") are shown with Yes/No.
- Authentication failure → error dialog with Git output and a hint (credential helper, SSH key).
- Record in DECISIONS (askpass design).

Acceptance:
- Integration test with a fake askpass invocation (prompt in, answer out, cancel).
- Maintainer check: HTTPS with GCM on Windows, SSH with passphrase key on Linux (Maintainer to-dos).

### [ ] M3-03 Fetch and auto-fetch

- Toolbar Pull dropdown entry "Fetch all" and Ctrl+L: `git fetch --all` (prune follows the user's `fetch.prune` config).
- Auto-fetch every 5 minutes per open tab (setting in the settings file, 0 = off); skipped while another operation runs or the repo is in a merge/rebase state; errors are shown once as a quiet notice, not as a dialog.
- After fetch: refs refresh, ahead/behind updated.

Acceptance:
- Integration tests against a local bare remote.
- Unit tests for the scheduler (interval, skip rules, cancel by user operation).

### [ ] M3-04 Pull

- Toolbar "Pull" with dropdown (GitKraken): Fetch all; Pull (fast-forward if possible); Pull (fast-forward only); Pull (rebase). The chosen mode becomes the default of the button (stored in settings).
- Commands: `git pull --ff`, `git pull --ff-only`, `git pull --rebase`.
- No upstream → dialog explaining it, with "Set upstream" (M3-10).
- Conflicts → repository state banner (M2-13); dialog explains that resolution comes in M4 and offers "Abort" via `git merge --abort` / `git rebase --abort`.

Acceptance:
- Integration tests for each mode incl. ff-only failure and conflict + abort.

### [ ] M3-05 Push

- Toolbar "Push": pushes the current branch to its upstream.
- No upstream → dialog: remote (default `origin`), remote branch name (default = local name) → `git push -u`.
- Rejected (non fast-forward) → dialog: "Pull" or "Force push" (uses `--force-with-lease`, confirmation required).
- Push tags: via tag context menu (M3-09); no automatic `--tags`.
- Ahead/behind refresh after push.

Acceptance:
- Integration tests: normal, new branch with upstream, rejected → force-with-lease, force-with-lease refused when the remote moved.

### [ ] M3-06 Checkout

- Double click local branch (sidebar and graph pill) → `git switch <branch>`.
- Double click remote branch → create local tracking branch with the same name and switch; if a local branch with that name exists, switch to it (and show ahead/behind).
- Commit context menu "Checkout this commit" → confirmation, detached HEAD.
- Local changes that block the checkout → dialog "Stash and checkout" / "Cancel" (see planning decisions).
- Replace the M2 double-click detection timer only if a cleaner iced 0.14 mechanism exists; otherwise keep it.

Acceptance:
- Integration tests for all variants incl. blocked checkout + stash.

### [ ] M3-07 Create branch

- Toolbar "Branch" and Ctrl+B: inline text input in the branch/tag column of the selected commit (WIP row → HEAD). Enter creates and checks out, Esc cancels.
- Commit context menu "Create branch here".
- Validation with `git check-ref-format --branch`; duplicate names rejected inline.

Acceptance:
- Unit tests for validation; integration tests for create + checkout.

### [ ] M3-08 Rename and delete branches

- Rename local branch (context menu, inline input); if it has an upstream, offer to rename the remote branch too (push new, delete old).
- Delete local branch: confirmation; unmerged branch → second confirmation with "Force delete" (`-D`). The checked-out branch cannot be deleted (menu item disabled).
- Delete remote branch: `git push <remote> --delete <branch>`, confirmation.

Acceptance:
- Integration tests incl. unmerged force delete and remote delete against a bare remote.

### [ ] M3-09 Tags

- Commit context menu "Create tag here": name + optional message (annotated if a message is given).
- Tag context menu: Delete local, Delete on remote, Push to remote, Copy name.
- Sidebar tags section shows local tags; remote-only tags are not listed.

Acceptance:
- Integration tests for create (both kinds), push, delete local/remote.

### [ ] M3-10 Upstream and remote management

- Branch context menu: Set upstream (choose remote branch), Unset upstream.
- Remote section: "+" adds a remote (name, URL); remote context menu: Edit URL, Rename, Remove (confirmation), Fetch this remote.

Acceptance:
- Integration tests for each operation.

### [ ] M3-11 Context menus

Complete menus using the M2 infrastructure. M4 actions are visible but disabled with tooltip "Not available yet".

| Target | Items |
|---|---|
| Commit | Checkout this commit, Create branch here, Create tag here, Copy SHA, Copy message; (M4) Cherry-pick, Revert, Reset to this commit |
| Local branch | Checkout, Pull (if upstream), Push, Set/Unset upstream, Rename, Delete, Create tag here, Copy name; (M4) Merge into current, Rebase current onto |
| Remote branch | Checkout (tracking branch), Delete on remote, Copy name; (M4) Merge into current, Rebase current onto |
| Tag | Push, Delete local, Delete on remote, Copy name |
| Remote | Fetch, Edit URL, Rename, Remove |
| Stash | (from M2) Apply, Pop, Delete |

- Right click works in graph rows, ref pills and sidebar entries.

Acceptance:
- Unit tests for menu building (enabled/disabled rules, e.g. current branch cannot be deleted).

### [ ] M3-12 Undo/redo for refs

Extend the M2 undo stack (ADR-014):
- Checkout: undo switches back to the previous branch/commit (only if the working tree state allows it).
- Create branch: undo deletes it (if unchanged); delete branch: undo recreates it at the recorded commit with its upstream.
- Rename branch: undo renames back. Tags: create/delete like branches (local only).
- Network operations (fetch, pull, push, remote deletes) are not undoable; their entries clear the redo stack.

Acceptance:
- Integration tests for each undo/redo pair and for failing preconditions.

### [ ] M3-13 Detached HEAD polish

- Detached HEAD: banner-free, but toolbar branch picker shows `HEAD (abc1234)`, graph HEAD marker, commit panel hint "You are in detached HEAD; create a branch to keep your commits".
- Leaving detached HEAD with commits not on any branch → confirmation offering "Create branch" first.

Acceptance:
- Integration test for the "unreachable commits" detection.

### [ ] M3-14 End-to-end check and docs

- Full walkthrough against the GitHub remote of this repo (on a throwaway branch) on Linux; maintainer repeats on Windows (Maintainer to-dos).
- Update `docs/UX-PARITY.md`, `CHANGELOG.md`, `docs/PERFORMANCE.md` (fetch/push overhead, auto-fetch idle cost).

### [ ] M3-15 Release v0.4.0

Same procedure as before (`/release`), only when the maintainer asks.
