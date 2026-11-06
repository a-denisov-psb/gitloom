# Undo and redo

**Undo** and **Redo** in the toolbar (or Ctrl+Z and Ctrl+Y / Ctrl+Shift+Z) name the action in their tooltip.

| Can be undone | Notes |
|---|---|
| Commit and amend | The changes of an undone commit stay staged. |
| Discard (file, hunk, lines, all) | The content before the discard is restored. |
| Checkout, create/delete/rename branch, create/delete tag | Local only: a branch renamed on the remote stays renamed there. Restored annotated tags are new tag objects with the same message. |
| Merge, fast-forward, rebase, interactive rebase, cherry-pick, revert, reword, squash | An operation that stopped with conflicts gets its entry when it is completed; an aborted operation leaves none. |
| Reset (soft, mixed, hard) | A hard reset restores the discarded changes. |

Nothing is undone if the repository changed in the meantime (for example HEAD or the branch moved, or local changes would be overwritten); Gitloom says so instead. Fetch, pull and push cannot be undone and clear the redo list. The undo history is per tab and is lost when Gitloom closes.
