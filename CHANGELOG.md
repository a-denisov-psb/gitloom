# Changelog

All notable changes to this project are documented here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning: [SemVer](https://semver.org).

## [Unreleased]

### Added

- Select several commits in the graph: Ctrl+click toggles, Shift+click selects a range, Shift+↑/↓ extend the selection. The right panel lists the selected commits.

### Changed

- Shift+↑/↓ now extend the commit selection; jumping to the first parent or child on the branch stays on Shift+J/K.

## [0.4.0] - 2026-10-25

### Added

- Detached HEAD: the commit panel reminds you to create a branch, and leaving a detached HEAD with commits that are on no branch or tag asks first, offering "Create branch".
- Undo and redo for checkout, creating, deleting and renaming branches, and creating and deleting tags (local). Nothing is undone if the branch or HEAD moved in the meantime; fetch, pull and push are not undoable.
- Complete context menus on commits (checkout, create branch/tag, copy SHA/message), local branches (checkout, pull, push, upstream, rename, delete, tag, copy name), remote branches, tags, remotes and stashes, in the graph, on ref pills and in the sidebar. Merge, rebase, cherry-pick, revert and reset are shown disabled until a later version. Pull and push also work for branches that are not checked out (pull fast-forwards only).
- Upstream and remotes: set or unset a branch's upstream from its context menu (also offered when Pull finds no upstream); add remotes with "+" in the REMOTE section; right click a remote to fetch it, edit its URL, rename or remove it. Remotes without fetched branches are listed too.
- Tags: "Create tag here" in the commit menu (annotated when a message is given); right click a tag to push it, delete it locally or on a remote, or copy its name. With several remotes Gitloom asks which one.
- Rename and delete branches from their context menu (sidebar and graph): rename inline, optionally renaming the upstream branch on the remote too; delete asks first and offers a force delete for unmerged branches; the checked-out branch cannot be deleted. Remote branches can be deleted on the remote.
- Create branches like in GitKraken: toolbar Branch or Ctrl+B opens a name input at the selected commit (the WIP row means HEAD), also "Create branch here" in the commit menu; invalid or taken names are flagged while typing, Enter creates and checks out the branch, Esc cancels.
- Checkout: double click a local branch (sidebar or graph) to switch to it, a remote branch to create and switch to its tracking branch, or right click a commit for "Checkout this commit" (detached HEAD, after a confirmation). If local changes are in the way, Gitloom offers "Stash and checkout".
- Push from the toolbar: the current branch goes to its upstream; a branch without upstream opens a dialog for remote and branch name and sets the upstream. A rejected push offers Pull or a force push (with lease, after a confirmation); a force push is refused if the remote changed since the last fetch.
- Pull from the toolbar: fast-forward if possible, fast-forward only or rebase, chosen in the Pull dropdown; the last choice becomes the button's mode (stored in the session file). Without an upstream branch a dialog explains why; a pull that stops in a conflict offers to abort the merge or rebase.
- Fetch all remotes from the Pull dropdown or with Ctrl+L, with progress and Cancel in the status bar. Open repositories fetch automatically every 5 minutes (`auto_fetch_minutes` in the session file, 0 = off); auto-fetch skips busy repositories and merges/rebases in progress, yields to your own operations and reports failures once in the status bar.
- Gitloom answers Git's and SSH's credential and host key prompts with a small window (masked for passwords and passphrases, Yes/No for host keys) when no credential helper or ssh-agent can; Gitloom never stores credentials. Authentication errors explain what to check.

### Fixed

- Changes to `.git/config` made outside Gitloom (new remotes, upstreams) refresh the open repository.
- Remotes added and upstream branches set outside Gitloom (or by a first push) now show up without reopening the repository.
- The banner of a merge started by `git pull` named the local branch twice ("Merging main into main"); it now names the remote branch.
- Context menu entries no longer wrap into the next entry when the label is long.
- Stash details showed "0 changed files" for stashes with only untracked files (or only staged changes): they now list staged, unstaged and untracked files in separate sections, and each file opens its diff.
- The author line in commit and stash details no longer shows a lone "·" when the email is empty.

## [0.3.0] - 2026-10-23

### Fixed

- Shift+J / Shift+K moved like J / K instead of jumping along the branch: iced reports letter keys without modifiers ("j" plus Shift).
- Keyboard shortcuts outside text fields (navigation, tabs, panels, staging, undo, F5, Ctrl+O, …) did nothing: the check whether a text field has focus gave no answer when one of the checked fields was not on screen, which was always the case.
- An open repository no longer causes constant background CPU load (about 0.5 %): file access events from Gitloom's own ignore checks kept re-triggering the change detection.
- Diffs of modified files in repositories with CRLF conversion (`core.autocrlf`, `eol` attributes) or clean filters now match `git diff` instead of showing every line as changed.

### Added

- Commit from the commit panel: summary with a 72-character counter, description, Ctrl+Enter (Ctrl+Shift+Enter stages everything first), Ctrl+Shift+M focuses the summary. Hooks run as usual; failures show the hook output and keep the message. The description is prefilled from `commit.template`, a merge in progress prefills its message and the commit concludes the merge.
- A merge, rebase, cherry-pick or revert in progress shows a banner in the tab ("Merging topic into main", "Rebasing topic onto main (step 2 of 5)"); actions that would break it are disabled with the reason in the tooltip. Committing concludes a merge, cherry-pick or revert with Git's prepared message.
- Partial (blobless) clones: file contents that were never downloaded are fetched through `git` when a diff needs them ("Fetching missing content…"); without `git` the diff explains that the content is not available locally.
- When another Git process holds a lock (`index.lock`), the failed operation offers "Retry"; Gitloom never deletes the lock.
- Undo and redo for commit, amend and discard (toolbar with the action named in the tooltip, Ctrl+Z, Ctrl+Y / Ctrl+Shift+Z). Undoing a commit keeps its changes staged; nothing is undone if the repository changed in the meantime.
- Stashes: toolbar Stash (includes untracked files) and Pop; stashes appear in the graph attached to their base commit; right click on a stash (sidebar or graph) offers Apply, Pop and Delete.
- "Amend previous commit" checkbox: fills the message from HEAD, allows message-only amends and warns when HEAD is already pushed.
- Discard changes of a file (hover button, context menu), a hunk, selected lines or all changes (with staged changes, the dialog asks whether to discard them too), always after a confirmation; the content before the discard is kept for undo. File context menu: Ignore (appends to `.gitignore`) and Open file location.
- Stage or unstage single hunks ("Stage hunk" in the hunk header) and selected lines (click, Shift+click, Ctrl+click, then "Stage selected lines") in the diff view; the diff reloads at the same position.
- Stage and unstage files: hover button on a file row, "Stage all changes" / "Unstage all changes", context menu, S / U for the shown file and Ctrl+Shift+S / Ctrl+Shift+U for all. After staging the shown file, the next unstaged file's diff opens.
- Changes made outside Gitloom (editor, build, `git` in a terminal) appear automatically within a second; build output in ignored folders causes no refresh. Window focus only reloads the working tree status while the file watcher runs.
- Right click on a changed file opens a context menu ("Copy file path"); failed Git operations show a dialog with the full Git output and a "Copy" button.
- Gitloom looks for `git` (2.34 or newer) at startup; without it the app stays read-only, shows a one-time notice and explains the reason on disabled write actions.

## [0.2.0] - 2026-10-21

### Added

- Real repositories are read with gitoxide in the background: history, branches, tags, stashes, working tree status and commit details. The UI stays responsive while loading.
- Refresh with F5 and automatically when the window regains focus; load errors are shown in the tab with a "Retry" button.
- Open repositories from the start screen (native folder dialog, Ctrl+O, or a typed path) or the command line (`gitloom <path>...`); a subfolder opens its repository, an already open repository switches to its tab.
- Tab shortcuts: Ctrl+T, Ctrl+W, Ctrl+Tab / Ctrl+Shift+Tab, Ctrl+1…9 (Cmd on macOS).
- Session restore: open tabs, active tab and pane sizes come back on the next start; the start screen lists up to 20 recent repositories. A missing repository can be removed from the recent list.
- Ctrl+J / Ctrl+K toggle the left panel and the commit detail panel.
- The commit list renders only visible rows, so repositories with 100 000 loaded commits scroll smoothly; selecting a branch in the sidebar scrolls its commit into view. Author, date and SHA columns hide automatically when the graph needs the space.
- Keyboard navigation in the graph: ↓/J and ↑/K, Shift+↓/Shift+J to the first parent, Shift+↑/Shift+K to the child on the same branch, Ctrl+Home/Ctrl+End (Cmd+↑/↓ on macOS) to the first/last commit, Ctrl+Alt+F focuses the sidebar filter, Esc closes the diff. Shortcuts pause while a text field has focus.
- Read-only diff view for commit, staged, unstaged and untracked files: hunk headers, old/new line numbers, colored added/removed lines, clear messages for binary and very large files. Esc or × returns to the graph at the same scroll position.
- Lane graph: stable lanes with curved merge and fork lines, colored per branch, commit nodes with author initials, small merge nodes and a hollow WIP node with a dashed line to HEAD. Local and remote branches on the same commit share one pill; further references collapse into "+N" with a tooltip.

- Project documentation: roadmap, M1 plan, architecture, decisions, UX parity reference, handover.
- Tooling: workspace lints, rustfmt and clippy config, check scripts, CI workflow, Claude Code setup.

### Changed

- No sample data anymore: actions that are not available yet (Undo, Redo, Pull, Push, Branch, Stash, Pop, Terminal, pickers, staging and committing) are disabled and explain "Not available yet".
- Detached HEAD shows as `HEAD (abc1234)` in the branch picker and as a HEAD pill in the graph; local branches show commits ahead/behind their upstream (↑/↓); a truncated history ends with "History truncated at 100 000 commits".

## [0.1.0] - 2026-10-20

### Added

- Layout shell modeled on GitKraken: repository tabs, toolbar, sidebar, commit graph table with WIP row, staging panel, commit details, diff placeholder, status bar, resizable panes.
- `git-core` crate with domain types, `GitBackend` trait and sample backend.
