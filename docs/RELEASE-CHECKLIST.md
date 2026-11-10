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

## 8. GitKraken verification (answers adjust code or UX-PARITY)

Compare in a current GitKraken and write the answer next to each item:

- [ ] Drag and drop of a branch onto a branch that is **not checked out**: are merge/rebase entries disabled, or does GitKraken check out automatically? (Gitloom: disabled with "Check out <branch> first".)
- [ ] Interactive rebase list order: newest commit on top or at the bottom? (Gitloom: newest on top.)
- [ ] Commit search: does GitKraken dim non-matching commits or filter them out? (Gitloom: dims.)
- [ ] Can the checked-out branch be hidden? (Gitloom: no.)
- [ ] File history and blame layout: list left / diff right, blame as a mode of the history view? (Gitloom: History / Blame toggle in one view.)
- [ ] Command palette sub-lists (checkout branch, open repository): same structure and Backspace behavior?
- [ ] Wording of the empty cherry-pick dialog ("Skip" / "Commit empty").
