# Undo and redo

**Undo** and **Redo** in the toolbar (or Ctrl+Z and Ctrl+Y / Ctrl+Shift+Z) name the action in their tooltip. Gitloom's goal is that every local action can be undone and lands exactly in the state before (ADR-043).

| Can be undone | Notes |
|---|---|
| Commit and amend | The changes of an undone commit stay staged. |
| Discard (file, hunk, lines, all) | The content before the discard is restored. Discarding a conflicted file is undone as well: both sides and the conflict come back. |
| Checkout, create/delete/rename branch, create/delete tag | Local only: a branch renamed on the remote stays renamed there. Restored annotated tags are new tag objects with the same message. |
| Merge, fast-forward, rebase, interactive rebase, cherry-pick, revert, reword, squash | An operation that stopped with conflicts gets its entry when it is completed; an aborted operation leaves none. |
| Reset (soft, mixed, hard), also on the HEAD commit | A hard reset restores the discarded changes; conflicted files cleared by a reset come back as conflicts. |
| Pop stash, Apply stash | With or without conflicts: the files, the index and the stash list return exactly to the state before; a popped stash goes back to its place in the list. If you already edited a conflicted file, Gitloom asks before throwing that work away. Redo applies the same stash again (not possible once it was deleted). |

Nothing is undone if the repository changed in the meantime (for example HEAD or the branch moved, or local changes would be overwritten); Gitloom says so instead. The undo history is per tab and is lost when Gitloom closes.

## Cannot be undone

| Action | Why |
|---|---|
| Push, deleting or renaming a branch or tag on a remote | The remote is shared: others may already have fetched the change. Push the old state again if needed. |
| Fetch (also the fetch part of a pull) | It only updates the remote-tracking branches to what the remote has; your work is not touched. Undoing it would hide the remote's real state. Fetch, pull and push clear the redo list. |
| Stash (push) and deleting a stash | Not yet; planned (M12). A deleted stash's commit stays in the repository for a while and can be recovered with `git fsck --lost-found`. |
| Stage, unstage, "Save and mark resolved" | Not yet; planned (M12). Staging or unstaging again reverses the first two. |
| Adding, removing, renaming remotes; setting the upstream | Not yet; planned (M12). Change them back in the left panel. |
| Settings in the Preferences and the Git config (identity, SSH key, Gitflow and others) | They are settings, not changes to your work: change them back in the Preferences. |
