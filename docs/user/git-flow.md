# Git flow

Gitloom supports Git flow like GitKraken: feature, release and hotfix branches with start and finish, without the `git-flow` command-line tool. The settings use git-flow's keys in the repository's `.git/config`, so a repository set up with the `git-flow` CLI, GitKraken or Gitloom works in all of them.

## Set it up

**Preferences → Repository · <name> → Gitflow** (or the command palette, "Initialize Gitflow") applies to the repository of the active tab:

| Field | Default |
|---|---|
| Production branch | `main` if it exists, otherwise `master` |
| Development branch | `develop` |
| Feature / release / hotfix prefix | `feature/`, `release/`, `hotfix/` |
| Version tag prefix | empty (e.g. `v` makes `v1.2.0`) |

**Initialize Gitflow** writes the settings and creates missing branches: the development branch from the production branch, the production branch from the current commit. Afterwards the same page edits the settings (**Save**) or removes them (**Remove Gitflow**; branches stay).

## The GITFLOW section

Once Git flow is set up, the left panel shows a **GITFLOW** section with Features, Releases and Hotfixes (names without the prefix). The branches are listed in LOCAL as well and behave the same in both places: click, double click to check out, right click for the menu, drag onto another branch.

## Start

- The green **+** in the GITFLOW header: Start feature, Start release, Start hotfix.
- The menu of the development branch (Start feature, Start release) or of the production branch (Start hotfix).
- The command palette: "Start feature", "Start release", "Start hotfix".

Type a name (a version for releases and hotfixes). Features and releases start from the development branch, hotfixes from the production branch; the new branch is checked out. Ctrl+Z deletes it again and returns to the previous branch.

## Finish

Right click the branch → **Finish feature …** / **Finish release …** / **Finish hotfix …**, or "Finish current Gitflow branch" in the palette while it is checked out.

| Type | What happens | Options |
|---|---|---|
| Feature | merged into the development branch with a merge commit | Rebase on the development branch first (off), Delete branch (on) |
| Release, hotfix | merged into the production branch, tagged there (annotated), then merged into the development branch | Tag name, tag message, Delete branch (on) |

Nothing is pushed; push the branches and the tag yourself when you are ready. The dialog warns when the production or development branch is behind its upstream, so you can pull first. A finish needs a clean working tree (untracked files are fine) and no merge or rebase in progress.

**Conflicts:** when a merge of the finish stops with conflicts, the banner reads "Finishing release 1.2.0 · Merging …". Resolve the conflicts as usual and **Continue**: the remaining steps (tag, second merge, delete) run by themselves. If Gitloom was closed in between, the banner offers **Continue finishing** when you open the repository again. **Abort** stops the merge and the finish and lists what was already done.

**Undo:** a whole finish is one Ctrl+Z: every branch and the tag go back to where they were, and the branch you were on is checked out again. Ctrl+Y redoes it with the same tag. Undo is refused if one of the branches moved in the meantime (for example a new commit on develop).

A hotfix finished while a release branch exists is merged into the development branch (GitKraken's documented behavior; the `git-flow` CLI merges it into the release branch instead).
