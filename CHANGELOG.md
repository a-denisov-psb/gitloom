# Changelog

All notable changes to this project are documented here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning: [SemVer](https://semver.org).

## [Unreleased]

### Added

- Theme "Classic Dark" (Preferences → Appearance) with the colors and graph look of older GitKraken versions: rows tinted in their lane color, branch and tag labels filled with the lane color and joined to their commit by a line, rings around the commit dots, column separators in the header and shadows at the side panels.
- Clone a repository from the start screen: URL, where to clone to, folder name (derived from the URL), full path preview and "Clone submodules"; progress and Cancel in the dialog, credentials asked like for fetch; the clone opens in the tab. A failed or cancelled clone leaves no new folder behind.
- Start a local repository from the start screen: name, folder, default branch, README.md and a `.gitignore` template (C++, Go, Java, JetBrains, macOS, Node, Python, Rust, VisualStudio, Windows from github/gitignore); the files are committed as "Initial commit" when a Git identity is set.
- Preferences → General → Default branch name (`init.defaultBranch` in the global Git config).
- Preferences → Signing: GPG or SSH format, signing key (GPG secret keys listed, SSH keys from `~/.ssh` or a file), GPG program, "Sign commits/tags by default" in the global Git config; values set in the repository are shown as overrides. A commit that fails to sign says "Signing failed" and points to these settings.
- Preferences → Repository → Commit edits the commit template (Git's `commit.template`; without one, `.git/gitloom/commit-template.txt` is created) with "Apply this template to commit messages" and "Remove comments from commit messages"; an empty commit message is prefilled with its summary and description.
- "Copy commit message prompt…" (Staged Files header and staged file menu, command palette) builds a prompt for a tool of your choice: instruction, commit template, branch, recent commits, staged files with +/- counts and the staged diff within a size limit; preview, size and a reminder to check for secrets before copying. The instruction is edited in Preferences → Templates. No AI inside Gitloom.
- Preferences → Repository → External editor: a repository can use its own editor (presets or a custom command) instead of the global one; Alt+E, the toolbar button, the palette and the file menus follow it.
- Stash message template (Preferences → Templates, off by default): presets such as `{branch} · {datetime}` or a custom text with `{branch}`, `{date}`, `{time}`, `{datetime}`, `{sha}`, `{summary}`, with a preview; the toolbar and palette Stash use it instead of Git's default message.
- Small options like GitKraken: Preferences → General "Prune remote branches on fetch" (on; fetch, auto-fetch and pull use `--prune`) and "Support long paths" (Windows, `core.longpaths`); External tools "Delete .orig files after merging"; Appearance "Show toolbar labels".
- Commit details show the signature status after the author line ("Signed by …", unverified trust, key expired or revoked, key not available, "Bad signature").
- The left panel groups local and remote branches into folders at `/` like GitKraken: folders and remotes collapse with a click (remembered per repository), branch rows show a branch icon and the last name segment (full name in the tooltip), the checked-out branch is highlighted.
- Diff header: Hunk / Inline / Split toggle, also in the command palette; the choice is remembered.
- Inline diff view: the whole file with each change block marked by a bar with "Stage block" / "Unstage block" / "Discard block"; line selection works as in the Hunk view; "Previous change" / "Next change" arrows in the header, and opening a file shows its first change.
- Split diff view: old and new side by side with paired changed lines, the same block bars, line selection (removed lines on the left, added on the right) and change arrows; added and deleted files show the empty side as "File added" / "File deleted".
- Git flow (git-flow compatible): Preferences → Gitflow initializes, edits or removes it for the active repository; a GITFLOW section in the left panel lists features, releases and hotfixes; start them with the green "+", the menu of the development or production branch, or the command palette; finish them from their branch menu or the palette (merge commits, tag for releases and hotfixes, optional rebase and branch deletion). A finish that hits a conflict continues by itself after the conflict is resolved, also after a restart ("Continue finishing"). A finish is one Undo/Redo step.
- Diff views scroll horizontally (Shift+wheel, horizontal wheel, touchpad, scrollbar below the text) in Hunk, Inline and Split; line numbers and markers stay in place.
- While branches are hidden or soloed, "Viewing x/y · Show All" above the filter tells how many are shown and shows all again; LOCAL and REMOTE count "shown/total".
- Preferences → Identities: saved commit identities (name and email) with a default that can keep the global Git config up to date; on first use the global identity is imported. "Manage identities" in the command palette.
- Commit panel: "Committing as <name> <email>" shows the identity the next commit uses; a click (or "Choose identity…" in the palette, or Preferences → Repository → Identity) uses a saved identity in the repository, returns to the global one or saves the current one. Without an identity, Commit, Amend and Continue are disabled with "Choose an identity first".
- Preferences → Repository → SSH key: use a key from `~/.ssh` (or any key file) for fetch, pull and push of a repository (`core.sshCommand` in its Git config), or the system default; a custom SSH command is only replaced after confirmation. Authentication errors name the key.
- Preferences → External tools → External editor: presets for VS Code, VSCodium, Sublime Text, IntelliJ IDEA and Zed or a custom command with `{path}`. "Open repository in editor" (Alt+E, command palette, toolbar button "Editor" while an editor is set) and "Open in external editor" in the file context menus.

- Graph columns can be resized like GitKraken: drag the border at the right of a column header (Branch / Tag, Graph, Author, Date, SHA); a double click sets the width that fits the visible rows (Graph: all lanes, at most half the table). The commit message fills the rest and keeps at least 120 px; a Graph column narrower than its lanes fades out at the right edge. Widths are kept across restarts.
- Gear at the right end of the graph header: show or hide Author, Date and SHA, and "Reset column widths"; the same options are in Preferences → Appearance ("Show commit author / date and time / SHA in graph").
- Release packages for Windows on ARM64 (portable zip) and Linux ARM64 (AppImage, tar.gz), best effort, and a `.deb` package for Linux x64.

- A stash pop or apply that stops in conflicts is no longer an error: a banner says the stash was kept, offers Undo and, once the conflicts are resolved, "Drop stash" / "Keep stash"; the conflict view names the sides "Updated upstream" and "Stashed changes".
- Stash pop and apply can be undone (Ctrl+Z, toolbar, banner) and redone, with and without conflicts: files, index and stash list return exactly to the state before.
- Conflicted files without a running operation show a banner that explains how to get out, with "Discard conflicted files…".
- Conflicted files can be discarded (context menu, "Discard all", the banner) after a confirmation; undo brings back both sides and the conflict.
- "Reset <branch> to this commit…" is also offered on the HEAD commit: Mixed clears the index (also conflicts) and keeps the files, Hard discards uncommitted changes; Soft is disabled there. Undo restores conflicted files as well.

### Changed

- Author, Date and SHA no longer hide by themselves in a narrow window; the table is clipped at the right instead, and the column headers line up exactly with the rows.
- Release binaries are built with full link-time optimization (Cargo profile `dist`): smaller and slightly faster.
- The start screen is laid out like GitKraken's New Tab: Open a repo, Clone a repo and Start a local repo on the left; recent repositories, Customize (Preferences, Manage identities) and Resources (user guide, report an issue, release notes) on the right.
- The character counter of the commit summary sits inside the field at the right edge like GitKraken; it turns to the warning color at 10 characters left and red below 0.
- Left panel sections in GitKraken's order: GITFLOW (when initialized), LOCAL, REMOTE, STASHES, TAGS, SUBMODULES. GITFLOW shows a tree like GitKraken: the development and production branches and folders "feature", "release", "hotfix", listed alphabetically.
- Graph lines are thinner and turn with rounded 90° corners like GitKraken's instead of curves; WIP and stash edges are dotted.
- Preferences: the settings of the active tab's repository sit in a group "Repository · <name>" below the global categories (Gitflow for now); on the start screen the group is disabled.

### Fixed

- Commit and stash details say "1 changed file" instead of "1 changed files".
- Icons stay sharp at fractional zoom levels and display scale factors (110 %, 125 %, 150 %): their size and position snap to whole physical pixels.
- UTF-16 text files with a byte order mark (for example written by PowerShell 5) are shown as text with their encoding instead of "Binary file not shown"; their lines, blocks and hunks cannot be staged one by one, the whole file can.

## [1.0.0-rc.4] - 2026-11-10

Fourth release candidate of Gitloom 1.0: centered toolbar buttons.

### Fixed

- Toolbar buttons center their icon and label.

## [1.0.0-rc.3] - 2026-11-10

Third release candidate of Gitloom 1.0: fixes for context menus, remotes, file locations, scrollbars and the graph.

### Fixed

- Context menus are as wide as their longest entry (up to a maximum; longer entries end in "…" with the full text as tooltip), so long branch names no longer stick out of the menu.
- Empty `[remote "…"]` sections in the Git configuration no longer show up as remotes that cannot be removed.
- Windows: "Open file location" selects the file in Explorer, also in folders with spaces in their names.
- "Copy file path" copies the full path of the file.
- Scrollbars no longer cover counts and buttons in the left panel and other lists.
- No more thin gaps between the rows of the commit graph at display scales like 125 %.

## [1.0.0-rc.2] - 2026-11-09

Second release candidate of Gitloom 1.0: no console window on Windows, working repository and branch pickers, and remotes that stay in sync with the Git configuration.

### Fixed

- Windows: no console window opens next to Gitloom.
- The repository and branch pickers in the toolbar open the recent repositories and the branch list.
- The left panel no longer shows remotes that were removed or renamed outside Gitloom: changes to any Git config file (also several quick writes, rewrites by tools and the global config) are picked up without a restart.
- Remotes defined in the global Git configuration are marked in the left panel; Remove, Rename and Edit URL are disabled for them, since Git cannot change them from the repository.
- Removing, renaming or editing a remote that no longer exists reloads the repository and says so in the status bar instead of showing Git's error.

## [1.0.0-rc.1] - 2026-11-06

First release candidate of Gitloom 1.0: installers for Linux, macOS and Windows, a new license (GPL-3.0-or-later), crash reports, an opt-in update check, an accessibility pass and a user guide.

### Added

- Keyboard access: Shift+F10 or the Menu key opens the context menu of the selected commit (↑/↓ and Enter choose an entry); Tab / Shift+Tab move between input fields; Pull, Push, Stash and Pop stash are in the command palette.
- Update check, opt-in (Preferences → General, off by default): once a day Gitloom asks GitHub for the latest release and shows "Gitloom X is available" in the status bar with a link to the release notes. Nothing is downloaded or installed.
- Crash reports: if Gitloom crashes, a local report (version, OS, message, backtrace) is written to the data folder; the next start offers "Open report folder" and "Copy report" in the status bar. Nothing is uploaded.
- App icon for the window, taskbar/dock and packages; Windows executables carry version information; Linux desktop entry and AppStream metadata.
- User guide in `docs/user/` (getting started, coming from GitKraken, daily workflow, branches and remotes, merge/rebase/conflicts, undo, settings, privacy, troubleshooting, known limitations), linked from the README and the About dialog.
- About dialog (command palette "About Gitloom", or the button at the bottom of the Preferences): version, commit, license, links to the repository and the issue tracker, and all third-party components with their licenses.

### Changed

- Gitloom is now licensed under the GNU General Public License v3.0 or later (previously MIT OR Apache-2.0). Third-party licenses are listed in `THIRD-PARTY-LICENSES.md`.

### Fixed

- Esc closes a dialog, form or menu with one press also while one of its input fields has focus.
- Focused input fields show a clearly visible focus ring in both themes; icon-only buttons (zoom, disabled Stash/Pop in the compact toolbar) have tooltips.

## [0.6.1] - 2026-11-06

### Fixed

- Cancelling a network operation can no longer hang when a Git helper process outlives the cancel (seen on Windows); an operation cancelled before it started no longer starts Git.

## [0.6.0] - 2026-11-04

### Added

- Open the repository in a terminal (Alt+T or the toolbar's Terminal button) or in the file manager (Alt+O); the terminal command can be set in Preferences → External tools.
- External diff and merge tools: Ctrl+D or "External" opens the shown diff in your diff tool, "Open in merge tool" resolves a conflict in your merge tool (Git's configuration, or a tool chosen in Preferences → External tools).
- Avatars from Gravatar, opt-in (Preferences → Appearance, off by default): round author pictures in the graph, the commit details and the file history. Only a hash of the email address is sent; results are cached for seven days. When off, Gitloom makes no network requests besides Git's own.
- Blame: the file history has a Blame mode (working copy or any commit of the history) with author, commit and age per block of lines; clicking a block shows that commit.
- File history: right click a file → "File history", the "History" button of the diff, or Ctrl+Shift+H to pick a file. Shows every commit that changed the file (following renames) with its diff; "Show in graph" jumps to the commit.
- Hide and solo branches: the eye on hover in the left panel or the branch menu hides a branch from the graph, "Solo" shows only the soloed branches (and HEAD); a remote's menu hides or shows all its branches. Remembered per repository.
- Commit search (Ctrl+F or the magnifier above the graph): message, author, email or SHA; other commits are dimmed, Enter / Shift+Enter jump between matches.
- Command palette (Ctrl+P): run any action by typing part of its name; open a recent repository (Ctrl+Shift+O) or check out a branch from the palette.
- Keyboard shortcut list (Ctrl+/): all shortcuts grouped like GitKraken's, with a filter.
- Icons (Lucide) in the toolbar, sidebar, graph, file lists and banners instead of text symbols; a combined local/remote pill shows a cloud icon with the remote names as tooltip.
- Zoom (Ctrl+= / Ctrl+- / Ctrl+0, 70–200 %, remembered) and fullscreen (F11).
- Light theme: Preferences → Appearance → Dark, Light or System (follows the operating system).
- Preferences (Ctrl+, or the gear button): the auto-fetch interval can be changed (0 turns it off). Preferences are stored in `settings.json` next to the session; the interval set before is taken over.

### Fixed

- In a narrow window or at high zoom the toolbar shows icons only, so no button is cut off.
- A context menu that fits neither below nor above the cursor (small window, high zoom) no longer jumps to the top of the window.

## [0.5.0] - 2026-10-26

### Added

- Select several commits in the graph: Ctrl+click toggles, Shift+click selects a range, Shift+↑/↓ extend the selection. The right panel lists the selected commits.
- Merge and fast-forward from the branch context menu ("Merge X into Y", "Fast-forward Y to X", and for remote branches "Fast-forward L to origin/L" for a local branch that is behind). Local changes that block a merge can be stashed and re-applied automatically.
- A rebase over merge commits asks first, the interactive rebase panel names the merges it drops, and conflicts in files larger than 2 MB offer taking one side's file.
- Undo and redo for merge, fast-forward, rebase, interactive rebase, cherry-pick, revert, reset (a hard reset restores the discarded changes), reword and squash. An operation that stopped with conflicts gets its entry when it is completed; aborted operations leave none.
- Drag a branch (graph pill or sidebar) onto another branch: fast-forward, merge, rebase or interactive rebase from the menu that opens.
- Reset the current branch to a commit (soft, mixed or hard) from the commit menu; a hard reset is confirmed and lists the changes that will be lost.
- Cherry-pick one commit (commit menu) or several (multi-selection, applied oldest first) and revert a commit; merge commits ask for the parent. An empty result offers "Skip" or "Commit empty".
- "Edit commit message" in the commit menu (HEAD is amended, older commits are reworded by a rebase) and "Squash N commits" for a contiguous multi-selection, both with a message dialog.
- Interactive rebase: "Interactive rebase from here" in the commit menu opens a panel to pick, reword, squash, fix up, drop and reorder (drag or Alt+↑/↓) the commits; reword and squash messages are edited inline before starting.
- Rebase from the branch context menu ("Rebase Y onto X"); conflicts stop in the banner and the conflict view, local changes can be stashed and re-applied automatically.
- Conflict resolution view: click a conflicted file to pick hunks or single lines from "ours" and "theirs", edit the output, then "Save and mark resolved". Binary and delete/modify conflicts take one side's file. Line endings are kept.
- Continue, Skip and Abort for merges, rebases, cherry-picks and reverts in progress, from the banner or the commit panel ("Continue rebase"); the commit message can be edited before continuing. Conflicted files get their own section in the staging panel.

### Changed

- Shift+↑/↓ now extend the commit selection; jumping to the first parent or child on the branch stays on Shift+J/K.

### Fixed

- Right click on a commit in a scrolled graph opened its menu off screen (and a drop from a scrolled graph likewise); opening a menu or dialog no longer resets the graph's scroll position.

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
