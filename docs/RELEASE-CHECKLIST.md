# Release checklist (1.0)

Consolidated manual checks for the first release (M6-13). They replace the M2–M5 items formerly under "Maintainer to-dos" in `HANDOVER.md`. Run them against the **packaged build** of a release candidate (the `.msi`/`.zip`, `.dmg`, `.AppImage` from the release workflow), not `cargo run`. Use a throwaway clone and, for remote checks, a throwaway branch and tag on GitHub (e.g. `e2e-windows`).

Mark each item `[x]` (works), or `[!]` with a short note (problem). Claude Code fixes the findings and re-runs the affected tests.

Platforms: **W** = Windows (required), **M** = macOS (if a Mac is available; otherwise CI only), **L** = Linux (AppImage).

## 1. Install and start

- [ ] W: `.msi` installs (administrator prompt), Start menu entry with icon, app starts; uninstall works. Note the SmartScreen behavior of the unsigned (later: signed) installer.
- [ ] W: portable `.zip` unpacked to a user folder starts without installation.
- [ ] W: app icon in the window title, taskbar and Alt+Tab; Explorer → Properties → Details shows product name, version and copyright.
- [ ] W x86: the x86 `.msi`/`.zip` starts on 32-bit Windows or as 32-bit process (best effort).
- [ ] M: `.dmg` → drag to Applications; first start blocked by Gatekeeper, right click → Open works (as described in the README); icon in the Dock.
- [ ] L: AppImage starts on Ubuntu 22.04 (or newer); icon in the window list.
- [ ] All: Help → About (palette "About Gitloom" and Preferences → About): version and commit match the release; License, Third-party licenses (opens the shipped file), User guide, Repository and Report an issue open correctly.
- [ ] All: `SHA256SUMS` matches the downloaded files (`certutil -hashfile <file> SHA256` on Windows).

## 2. Keyboard (all views; Windows required)

- [ ] Ctrl+/ shortcut list opens, filters, Esc closes.
- [ ] Ctrl+P palette: ↑/↓ wrap, Enter runs, Esc closes; "Checkout branch…" then Backspace twice returns to the actions; Ctrl+Shift+O lists recent repositories. Pull, Push, Stash and Pop stash run from the palette.
- [ ] Ctrl+F search: typing dims non-matching rows and shows "n of m", Enter / Shift+Enter jump, Esc closes and keeps the selection.
- [ ] Ctrl+B creates a branch: type a name (an invalid one shows a red border with the reason), Enter creates and checks it out, Esc cancels.
- [ ] Graph: ↑/↓/J/K, Shift+J/K, Shift+↑/↓ extend the selection, Ctrl+Home/End; Shift+F10 (and the Menu key) opens the commit menu, ↑/↓ and Enter run an entry, Esc closes.
- [ ] Forms: "+" in the REMOTE section (type name, Tab to URL, Enter), rename a remote and a branch from their context menus (type the new name, Enter; with an upstream "Rename remote too"); Esc closes each form with one press.
- [ ] Commit panel: type a summary, Tab to the description, Ctrl+Enter commits; Ctrl+Shift+M focuses the summary; Ctrl+Shift+Enter stages all and commits.
- [ ] Interactive rebase panel: P/R/S/F/D, ↑/↓, Alt+↑/↓, typing into a reword message.
- [ ] "Edit commit message" with a typed change (HEAD and an older commit) and "Squash N commits" after a Ctrl/Shift+click multi-selection.
- [ ] Ctrl+= / Ctrl+- / Ctrl+0 zoom, F11 fullscreen, Ctrl+, Preferences, Ctrl+J / Ctrl+K panels, Ctrl+T / Ctrl+W / Ctrl+Tab / Ctrl+1…9 tabs.
- [ ] Focus ring visible in Dark and Light on every input field.

## 3. Daily workflow on Windows (formerly M2)

- [ ] No console window flashes up when Gitloom runs `git`.
- [ ] No console window when starting Gitloom from the Start menu, the zip and a terminal; the terminal returns to the prompt right away (M6-16).
- [ ] HTTPS credential prompt and interactive rebase still work with the release build (M6-16).
- [ ] Line staging in a CRLF repository (`core.autocrlf=true`): stage and unstage single lines and hunks; `git diff --cached` matches.
- [ ] Commit, amend (message only and with changes), undo and redo of both.
- [ ] Stash with an untracked file, Pop; discard a file and a hunk, then undo.

## 4. Branches and remotes on Windows (formerly M3)

- [ ] Walkthrough against the GitHub remote: Fetch all, create branch (Ctrl+B), push with upstream, pull (fast-forward), rejected push → Force push (once without and once after a fetch), create/push/delete a tag, rename a branch including the remote, delete remote and local branch, checkout of a remote branch. Afterwards GitHub has no leftovers.
- [ ] Credentials: HTTPS with Git Credential Manager (no Gitloom prompt when GCM has the credential); with the credential removed, Gitloom's prompt window appears and Esc cancels it.
- [ ] L: SSH with a passphrase-protected key without ssh-agent: Gitloom's prompt asks for the passphrase; an unknown host key asks Yes/No.
- [ ] Remove, rename and add a remote in a terminal while Gitloom is open: the left panel follows (M6-18).

## 5. History editing on Windows (formerly M4)

- [ ] Merge with a conflict resolved in the conflict view; rebase with a conflict (Continue, Skip, Abort); interactive rebase with reorder, squash, reword and drop; cherry-pick of three commits via Ctrl/Shift+click; revert; reset soft/mixed/hard. Undo and redo each; compare the trees with the Git CLI.
- [ ] Git for Windows runs Gitloom as editor during rebase and reword (no hanging command, no terminal).

## 6. Productivity on Windows (formerly M5)

- [ ] Dark and Light at 100 % and 200 %: all views readable, no cut-off buttons.
- [ ] Alt+T opens Windows Terminal (else PowerShell/cmd) in the repository; Alt+O and "Open file location" open Explorer with the file selected.
- [ ] M: Alt+T opens Terminal.app, Alt+O Finder.
- [ ] Hide/solo in the sidebar, file history and blame, avatars on/off, an external diff tool (Ctrl+D) and merge tool ("Open in merge tool").

## 7. New in 1.0

- [ ] Update check: off by default (no request; check with a firewall log or Resource Monitor if in doubt). Turned on in an `rc.1` build after `rc.2` is published: the status bar shows "Gitloom 1.0.0-rc.2 is available", Release notes opens the release page, Dismiss hides it until the next version.
- [ ] Crash report: not triggerable in release builds; if a crash happens during the checks, the next start shows the notice, Copy report and Open report folder work.
- [ ] Privacy: with avatars and update check off, Gitloom opens no network connection besides Git's (auto-fetch).
- [ ] Context menus with long branch names (create a branch with a 60+ character name, drag it onto another branch, right click it in the left panel and in the graph) are as wide as their longest entry; entries cut with "…" show the full text as tooltip.
- [ ] The empty remote `github-fc` (empty `[remote "github-fc"]` section in `.git/config`) is no longer listed; afterwards remove the section with `git config --remove-section remote.github-fc`.
- [ ] In a repository whose path contains a space: "Open file location" on a file in a subfolder selects it in Explorer; "Copy file path" copies the full Windows path.
- [ ] Left panel with more branches than fit: the scrollbar does not cover the counts.
- [ ] Graph at 125 % Windows scaling and zoom 110 %/120 %: no gaps between rows in lines or row backgrounds.
- [ ] Toolbar: icon and label of every button (Undo … Pop, Terminal, Settings) are centered, also at 125 % scaling and in the compact toolbar of a narrow window.

## 7a. New in 1.1 (M7; Windows required, Linux checked in CI)

- [ ] Left panel: folders collapse and stay collapsed after a restart; "Viewing x/y · Show All" appears when hiding a branch; drag and drop, double click and the branch menu work on branches inside folders.
- [ ] Diff views: switch Hunk / Inline / Split on an unstaged, a staged and a commit diff; stage, unstage and discard a block and selected lines in Inline and Split (also a CRLF file); the arrows jump between changes; the mode survives a restart.
- [ ] A 2 MB text file scrolls smoothly in Inline and Split.
- [ ] Git flow: initialize in Preferences on a repository without `develop`; start a feature from the green "+", finish it with "Delete branch"; start and finish a release (tag created, merged into main and develop); Ctrl+Z undoes the finish, Ctrl+Y redoes it.
- [ ] Release finish with a conflict in the merge into develop: resolve in the conflict view, Continue, the finish completes by itself; close Gitloom while the conflict is open, reopen, "Continue finishing" completes it.
- [ ] A repository initialized with the `git-flow` CLI (Git for Windows ships it) shows the GITFLOW section; one initialized by Gitloom works with `git flow feature start` in a terminal.
- [ ] Diff with a line longer than the view: Shift+wheel and the horizontal scrollbar scroll it in Hunk, Inline and Split; line selection still works after scrolling.
- [ ] A file created in PowerShell 5 with `echo test > utf16.txt` shows its text; line staging is disabled with the hint, staging the whole file works.
- [ ] Graph lines at 125 % Windows scaling look like GitKraken's (corners, width, dashes) and have no gaps.

## 7b. New in 1.2 (M8; Windows required, Linux checked in CI)

- [ ] Identity: on a fresh profile the global identity appears in Preferences → Identities as default; add a second identity, choose it in the commit panel ("Committing as …"), commit: author and committer of the new commit are that identity; "Use global identity" removes `user.name`/`user.email` from `.git/config`.
- [ ] Default identity with "Keep the global Git config updated" on: setting another default changes `git config --global user.name`; off: nothing changes.
- [ ] Without any identity (empty global config): the commit panel shows "No Git identity set", Commit is disabled with "Choose an identity first".
- [ ] SSH key with a passphrase: choose it in Preferences → Repository → SSH key, fetch an SSH remote: the passphrase prompt appears, the fetch works; "System default" restores the previous behavior; a hand-written `core.sshCommand` is only replaced after "Replace…".
- [ ] External editor VS Code (`code.cmd`): Alt+E, the toolbar button "Editor", the palette and "Open in external editor" on a file in a folder with spaces open VS Code; no console window flashes up.

## 7c. New in M9 (Windows required, Linux checked in CI)

- [ ] Stash conflict: stash a staged change, merge a branch that changes the same lines, Pop: the banner "Stash '…' applied with conflicts. The stash was kept." appears, no error dialog; the conflict view says "Updated upstream" / "Stashed changes"; Stash, Pop and Apply are disabled with the reason.
- [ ] Undo in the banner (and Ctrl+Z) after that pop: files, staged changes and the stash list are exactly as before (`git status`, `git stash list`); Ctrl+Y applies it again. Edit a conflicted file first: Undo asks before discarding the edit.
- [ ] Resolve the stash conflict and stage it: the banner offers Drop stash / Keep stash; Drop stash removes the stash.
- [ ] Undo of a clean Pop of an older stash (`stash@{1}`): it is back at its position in the list.
- [ ] Discard a conflicted file (context menu) and "Discard conflicted files…" in the banner after a restart: the confirmation names both sides; Ctrl+Z brings the conflict back.
- [ ] Reset on the HEAD commit with conflicted files: Soft disabled with "HEAD is already at this commit"; Mixed keeps the files and clears the conflicts; Ctrl+Z restores them; Hard likewise.
- [ ] Icons at 110 % and 125 % (Windows scaling and Gitloom zoom): toolbar, left panel headers (the "+" of REMOTE and GITFLOW) and file lists are sharp, not blurry.
- [ ] Left panel order: GITFLOW, LOCAL, REMOTE, STASHES, TAGS, SUBMODULES; GITFLOW is a tree (develop, feature, master, release) like GitKraken.
- [ ] Commit summary counter inside the field at the right edge; yellow at 10 left, red below 0; a long summary never runs under it.

## 7d. New in M10 (Windows required, Linux checked in CI)

- [ ] Start screen: Open a repo, a recent repository, Preferences, Manage identities and the three Resources links work.
- [ ] Clone over HTTPS with credentials (the askpass prompt appears, nothing is stored) and over SSH with a key and passphrase; progress shows; Cancel during "Receiving objects" leaves no folder; a non-empty target is refused; the clone opens in the tab.
- [ ] Start a local repo with README and the VisualStudio `.gitignore`: "Initial commit" exists on the chosen branch; with an empty global identity the files stay uncommitted with the notice.
- [ ] Preferences → General → Default branch name writes `init.defaultBranch`; a new repository uses it.
- [ ] GPG signing with Gpg4win: the secret key is listed, "Sign commits by default" on, a commit asks for the passphrase through pinentry and shows "Signed by …" in the details (or "unverified trust"); a wrong GPG program shows "Signing failed" with the hint.
- [ ] SSH signing with a key in the agent: commit, the details show a signature badge ("key not available" until `gpg.ssh.allowedSignersFile` is set).
- [ ] Commit template: save in Repository → Commit, the next empty commit message is prefilled (summary and description), comments are removed; a global `commit.template` asks before it is changed.
- [ ] Copy commit message prompt from the Staged Files header: preview, size, Copy pastes the whole text.
- [ ] Stash with every stash message preset and a custom template; `git stash list` shows the rendered text; off gives Git's default.
- [ ] External editor per repository: set one in Repository → External editor; Alt+E, the toolbar button and "Open in external editor" use it; "Use global setting" goes back.
- [ ] Support long paths on: `git config --global core.longpaths` is true; a file in a path over 260 characters can be staged. Prune on: a branch deleted on the remote disappears after Fetch. Delete .orig files: the merge tool leaves no `.orig`. Toolbar labels off: icons with tooltips.

## 7e. New in M11 (Windows required, Linux checked in CI)

- [ ] Classic Dark: rows tinted in the lane color, filled branch and tag labels joined to their commit by a line, ringed nodes, header separators, panel shadows; selected and hovered rows stay readable.
- [ ] Columns: resize every column incl. Commit message; only that column changes; a double click fits it (Commit message: back to filling); a horizontal scrollbar appears when the table is wider than the window, Shift+wheel and a horizontal touchpad gesture scroll it and the header moves along; the gear menu hides Author/Date/SHA and resets the widths; widths survive a restart.
- [ ] Stashes: dashed square with the stash icon, "WIP on <branch>" in the graph, STASHES and the details; "Stash" without a template writes Git's default message.
- [ ] Preferences → Editor: Consolas (and Cascadia Mono if installed) in the font list and applied to diffs, blame, file history and the conflict view; font size, tab size, line numbers, word wrap and syntax highlighting take effect at once.
- [ ] Syntax highlighting in Hunk, Inline and Split, blame, file history and the conflict view (both columns and the editable output); a multi-line comment is right across hunks; a file over 1 MB shows "Too large for syntax highlighting"; scrolling a wrapped 2 MB diff stays fluid.
- [ ] Left panel: branches before folders in LOCAL, REMOTE, GITFLOW and TAGS; tag folders collapse; the checked-out branch has a check next to its branch icon, also inside a folder.
- [ ] Start screen: the three buttons have one size and the accent color in all themes, the "Ctrl+O" line is readable.
- [ ] Preferences open as a centered window over the dimmed app; × and Esc close it, a click beside it does not, Tab stays inside; a small window shows them full size.
- [ ] Toolbar toggles hide and show the left and the commit panel, in step with Ctrl+J / Ctrl+K.
- [ ] The README screenshot matches the app.

## 8. GitKraken verification (answers adjust code or UX-PARITY)

Compare in a current GitKraken and write the answer next to each item:

- [ ] Drag and drop of a branch onto a branch that is **not checked out**: are merge/rebase entries disabled, or does GitKraken check out automatically? (Gitloom: disabled with "Check out <branch> first".)
- [ ] Interactive rebase list order: newest commit on top or at the bottom? (Gitloom: newest on top.)
- [ ] Commit search: does GitKraken dim non-matching commits or filter them out? (Gitloom: dims.)
- [ ] Can the checked-out branch be hidden? (Gitloom: no.)
- [ ] File history and blame layout: list left / diff right, blame as a mode of the history view? (Gitloom: History / Blame toggle in one view.)
- [ ] Command palette sub-lists (checkout branch, open repository): same structure and Backspace behavior?
- [ ] Wording of the empty cherry-pick dialog ("Skip" / "Commit empty").
