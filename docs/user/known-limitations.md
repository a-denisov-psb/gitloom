# Known limitations

- **Screen readers** are not supported: the UI toolkit (iced 0.14) does not expose an accessibility tree. Keyboard access, visible focus and zoom up to 200 % are available.
- **Buttons are not focusable** with Tab; actions are reachable through shortcuts, the command palette (Ctrl+P) and context menus (Shift+F10). Tab moves between input fields only.
- **No automatic updates:** the opt-in update check only tells you about a new version.
- **macOS builds are not notarized** and **Windows builds are not signed yet**; see [Getting started](getting-started.md#install) for the first start.
- **Graph columns** cannot be resized yet.
- **UTF-16 files** (with a byte order mark, e.g. written by PowerShell 5) are shown as text, but their hunks, blocks and lines cannot be staged, unstaged or discarded one by one (`git apply` treats them as binary); stage, unstage or discard the whole file. With the attribute `working-tree-encoding=UTF-16LE-BOM` the diff is not available (shown as filtered file); `working-tree-encoding=UTF-16` works.
- **Terminal-based diff and merge tools** (for example `vimdiff`) do not work: Gitloom runs Git without a console window, so these tools get no window. Use a graphical tool (Preferences → External tools).
- **Interactive rebase with conflicts:** if a step stops for a conflict, reword and squash messages prepared for later steps are lost; Git then stops at those steps and Continue uses the original message.
- **Revert of a commit without changes** fails with Git's "nothing to commit" message.
- **Autostash conflicts:** when re-applying stashed changes after a merge or rebase conflicts, the stash stays in the list without a separate notice.
- **Partial clones:** commit file lists may show a rename as deletion plus addition when the contents are not available locally.
- **History limit:** the graph shows at most 100 000 commits.
- **Undo history** is lost when Gitloom closes; network operations cannot be undone.
- **Not included by design:** accounts, cloud workspaces, pull request and issue tracker integrations, AI features, an integrated terminal.
