# Daily workflow

## The graph

Every commit is a row: branch and tag pills, the lane graph, the message, author, date and SHA. Each branch keeps its lane and color; merges and forks are curved lines. Local and remote branches on the same commit share one pill (a cloud icon names the remotes in its tooltip); more references collapse into "+N". Local branches show how many commits they are ahead (↑) or behind (↓) their upstream. Stashes are dashed squares with a stash icon attached to their base commit, labelled "WIP on <branch>" (or the message you gave). Large histories are cut off at 100 000 commits ("History truncated").

- Click a commit for its details on the right; click a file there to see its diff.
- Ctrl+click and Shift+click (or Shift+↑/↓) select several commits; the right panel lists them.
- ↓/J and ↑/K move the selection, Shift+J/K follow the branch, Ctrl+Home/Ctrl+End jump to the first/last commit.
- Clicking a branch in the left panel jumps to its commit. Branch and tag names with `/` are grouped into folders (click a folder or remote to collapse it; remembered per repository); every level lists branches (or tags) first, then folders. The checked-out branch is highlighted and has a check next to its branch icon.
- The small panel icons at the left and right end of the status bar hide and show the left panel and the commit panel (also Ctrl+J and Ctrl+K); a filled side means the panel is shown.

**Columns:** drag the border at the right of any column header to change that column's width (only that one changes); a double click sets the width that fits the visible rows, on the Commit message border it goes back to filling the free space. A table wider than the window scrolls sideways: the scrollbar below the rows, Shift+wheel or a horizontal wheel; the header moves along. The gear at the right end of the header shows or hides Author, Date and SHA and has **Reset column widths**. Widths are kept across restarts.

**Search** (Ctrl+F or the magnifier above the graph): type part of a message, author, email or SHA. Other commits are dimmed, "n of m" counts the matches, Enter / Shift+Enter jump to the next / previous one, Esc closes the search.

**Hide and solo branches:** the eye that appears when you hover a branch in the left panel hides it from the graph; the branch menu also has **Solo**, which shows only soloed branches (and HEAD). A remote's menu hides or shows all its branches. This is remembered per repository. While anything is hidden or soloed, "Viewing x/y · Show All" above the filter says how many branches are shown, and **Show All** shows them all again.

![Graph with the commit details](../screenshots/m5-graph-dark.png)

## Diffs

Code is syntax highlighted (shown plain for a moment first); font, size, tabs, line numbers and word wrap are set in Preferences → Editor.

Clicking a file shows its diff in the center. Three views, switched in the diff header (or the palette, "Diff: … view"); the choice is remembered:

- **Hunk**: only the changed parts with 3 lines around them, each hunk with a header.
- **Inline**: the whole file; a bar above every change block offers **Stage block** / **Unstage block** / **Discard block**.
- **Split**: old (left) and new (right) side by side with the same bars; changed lines are paired, added or deleted files show "File added" / "File deleted" on the empty side.

In Inline and Split the arrows in the header jump to the previous / next change, and opening a file shows its first change. Old and new line numbers, + and − markers and colors are the same in all views. Binary and very large files get a short explanation instead. In partial (blobless) clones, missing file contents are fetched through Git when a diff needs them.

![Diff view](../screenshots/m5-diff-light.png)

**External** (or Ctrl+D) opens the diff in your diff tool, **History** opens the file history.

## Staging

Select the **WIP** row at the top of the graph. The right panel lists unstaged, staged and conflicted files; changes made outside Gitloom (editor, build, a terminal) appear within a second.

- Hover a file for **Stage** / **Unstage** and **Discard**; or right click it.
- **Stage all changes** / **Unstage all changes**, Ctrl+Shift+S / Ctrl+Shift+U.
- With a file's diff open: **S** stages it, **U** unstages it; the next file's diff opens.
- In the diff: **Stage hunk** in a hunk header (Hunk view) or **Stage block** above a change (Inline, Split), or select lines (click, Shift+click, Ctrl+click; in Split removed lines on the left, added lines on the right) and **Stage selected lines**.
- **Discard** works per file, hunk, selected lines or for all changes. It always asks first and can be undone (Ctrl+Z). "Discard all" with staged changes asks whether to discard those too.
- File menu: **Ignore** (adds the file to `.gitignore`), **Open file location**, **Copy file path**, **File history**.

## Committing

Write a summary (the counter inside the field shows the remaining characters of 72; it turns yellow at 10 and red below 0, committing still works) and an optional description, then **Commit** or Ctrl+Enter; Ctrl+Shift+Enter stages everything first. Ctrl+Shift+M jumps to the summary. Hooks run as usual; if one fails, its output is shown and your message is kept. An empty message is prefilled from the commit template (Preferences → Repository → Commit): its first line becomes the summary, the rest the description; comment lines are removed when that option is on, also from the message you commit. A commit that fails to sign says "Signing failed" and points to Preferences → Signing; signed commits show their status after the author in the commit details.

**Copy commit message prompt…** (the sparkle button right of "Committing as …", right click the Staged Files header or a staged file, or the command palette) builds a prompt for a tool of your choice: the instruction from Preferences → Templates, the commit template, the branch, the last 10 commit summaries, the staged files with their added/removed lines and the staged diff (up to 60,000 characters; binary files and files over 256 KB are listed without their diff). The dialog shows the size and what was left out; **Copy** (or Ctrl+C) puts it on the clipboard. Check the diff for secrets before you share it. Gitloom itself sends nothing anywhere.

**Amend previous commit** fills the message from the last commit; you can amend only the message. Gitloom warns when that commit is already pushed.

With a detached HEAD, the commit panel reminds you to create a branch.

**Committing as <name> <email>** above the summary shows the identity the next commit uses; the tooltip says whether it comes from the repository's or the global Git config. Click it to use one of your saved identities in this repository (written to `user.name`/`user.email` of the repository config), **Use global identity** to remove the repository's own values, **Save as identity** to keep the current one, or **Manage identities…**. The same choices are in the command palette ("Choose identity…") and in Preferences → Repository → Identity. Without a name and email, the line reads "No Git identity set" and Commit, Amend and Continue stay disabled until you choose or add one. Saved identities and the default live in Preferences → Identities ([Settings](settings.md)).

## Stashing

**Stash** in the toolbar stashes all changes including untracked files (with the stash message template of Preferences → Templates when it is on, else Git's default message, shown as "WIP on <branch>"); **Pop** applies and removes the newest stash. Right click a stash (left panel or graph) for **Apply**, **Pop** and **Delete**. Selecting a stash shows its staged, unstaged and untracked files. Pop and Apply can be undone; if they conflict, see [Stash conflicts](merge-rebase-conflicts.md#stash-conflicts).

## File history and blame

Right click a file → **File history**, the **History** button of a diff, or Ctrl+Shift+H to pick any tracked file. The list shows every commit that changed the file, following renames, with its diff; **Show in graph** jumps to the commit.

![File history](../screenshots/m5-history-dark-100.png)

Switch to **Blame** to see author, commit and age for each block of lines, for the working copy or any commit of the history; clicking a block shows that commit.

## Terminal, editor and file manager

**Terminal** in the toolbar or Alt+T opens your terminal in the repository; Alt+O (or "Open file location") opens the file manager. The terminal command can be set in Preferences → External tools.

With an external editor set (Preferences → External tools), **Editor** in the toolbar, Alt+E or "Open repository in editor" in the palette opens the repository in it; **Open in external editor** in the context menu of a file (unstaged, staged, commit files, file history) opens that file. Deleted files cannot be opened.
