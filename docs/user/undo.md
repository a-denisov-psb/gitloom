# Undo and redo

**Undo** and **Redo** in the toolbar (or Ctrl+Z and Ctrl+Y / Ctrl+Shift+Z) name the action in their tooltip. Gitloom's goal is that every local action can be undone and lands exactly in the state before (ADR-043).

| Can be undone | Notes |
|---|---|
| Commit and amend | The changes of an undone commit stay staged. |
| Discard (file, staged file, hunk, lines, all) | The content before the discard is restored. Discarding a conflicted file is undone as well: both sides and the conflict come back. |
| Checkout, create/delete/rename branch, create/delete tag | Local only: a branch renamed on the remote stays renamed there. Restored annotated tags are new tag objects with the same message. |
| Deleting several branches and tags at once (left panel selection) | All local branches and tags of one bulk delete come back in one step, with their upstreams, also the ones deleted after "Delete anyway". Remote branches of the same delete cannot be undone (see below). |
| Edit commit message (click the message in the commit details) | The branch goes back to its commits before the edit; refused if the branch moved since. Only commits that were not pushed can be edited this way: changing a pushed commit would force everyone who pulled it to rebase (use interactive rebase and a force push for that, on purpose). |
| Merge, fast-forward, rebase, interactive rebase, cherry-pick, revert, reword, squash | An operation that stopped with conflicts gets its entry when it is completed; an aborted operation leaves none. |
| Reset (soft, mixed, hard), also on the HEAD commit | A hard reset restores the discarded changes; conflicted files cleared by a reset come back as conflicts. |
| Pop stash, Apply stash | With or without conflicts: the files, the index and the stash list return exactly to the state before; a popped stash goes back to its place in the list. If you already edited a conflicted file, Gitloom asks before throwing that work away. Redo applies the same stash again (not possible once it was deleted). |
| Stash (push) | Undo applies the stash back (staged changes stay staged) and removes it from the list, but only while the working copy is still exactly as the stash left it. Redo stashes again with the same message. |
| Delete stash | The stash goes back to its place in the list. |
| Rename stash | The old name comes back, at the same place in the list; refused if the stash list changed above it. |
| Stage, unstage (file, lines, hunk, block, all) | The index entries of the affected files return to what they were; refused if they changed since. |
| Pull (the checked-out branch, or another branch from its menu) | The local part is undone like a merge, fast-forward or rebase: the branch goes back to where it was. The fetched remote-tracking branches stay (see Fetch below). |
| Add, remove, rename a remote; edit its URL | Removing a remote is undone with all its settings, its remote-tracking branches and the upstreams that used it. |
| Set upstream, unset upstream | The branch's previous upstream (or none) comes back. |
| Update submodule(s) | Each submodule goes back to the commit it had checked out (detached, as Git leaves submodules); a submodule that was not initialized before is de-initialized again. Refused if a submodule has local changes or moved since. |
| "Save and mark resolved", "Mark all resolved", taking one side of a conflict | The conflict comes back: the unmerged index entries and the file content as before; refused if the file changed since. |

Nothing is undone if the repository changed in the meantime (for example HEAD or the branch moved, or local changes would be overwritten); Gitloom says so instead. The undo history is per tab and is lost when Gitloom closes.

## Cannot be undone

| Action | Why |
|---|---|
| Push, deleting or renaming a branch or tag on a remote (also in a bulk delete) | The remote is shared: others may already have fetched the change. Push the old state again if needed. |
| Fetch (also the fetch part of a pull) | It only updates the remote-tracking branches to what the remote has; your work is not touched. Undoing it would hide the remote's real state. Fetch, pull and push clear the redo list. |
| Clone and "Start a local repo" | They create a new folder; delete it yourself if you no longer need it. |
| Settings in the Preferences and the Git config (identity, SSH key, Gitflow, signing, commit template, default branch name, external editor, prune, long paths, "Trust this repository…" (`safe.directory`) and others) | They are settings, not changes to your work: change them back in the Preferences. |
