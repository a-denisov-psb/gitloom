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

Once Git flow is set up, the left panel shows a **GITFLOW** section at the top, like GitKraken: the development and production branches and the folders "feature", "release" and "hotfix" (named after the prefixes) with the branches inside, all in alphabetical order. The branches are listed in LOCAL as well and behave the same in both places: click, double click to check out, right click for the menu, drag onto another branch.

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

**Conflicts:** when a merge of the finish stops with conflicts, the right panel is titled "Finishing release 1.2.0 · Merging …". Resolve the conflicts as usual and **Commit and merge**: the remaining steps (tag, second merge, delete) run by themselves. If Gitloom was closed in between, a bar offers **Continue finishing** when you open the repository again. **Abort finish** (or **Abort** in that bar) stops the merge and the finish and lists what was already done.

**Undo:** a whole finish is one Ctrl+Z: every branch and the tag go back to where they were, and the branch you were on is checked out again. Ctrl+Y redoes it with the same tag. Undo is refused if one of the branches moved in the meantime (for example a new commit on develop).

A hotfix finished while a release branch exists is merged into the development branch (GitKraken's documented behavior; the `git-flow` CLI merges it into the release branch instead).

## Gitflow layout

With Git flow initialized, the graph can be drawn like the classic Git flow diagram: **production** at the left, then **hotfix**, **release**, **development** and the **features** at the right, one color per branch type (green, cyan, blue, violet; features in pink, red, orange and yellow, one per branch). Turn it on in the gear menu above the graph (**Gitflow layout**), with "Toggle Gitflow layout" in the command palette, or in Preferences → Repository → Gitflow; the choice is stored in the repository (`gitloom.flowLayout`). Without Git flow the entries are disabled ("Initialize Gitflow first").

How a commit gets its type:

1. The first-parent chains of the production and the development branch are production and development (production wins where both meet).
2. Every other branch: its commits down to the production or development chain, typed by its prefix (`feature/`, `release/`, `hotfix/`; other names count as features).
3. Branches deleted long ago are found from the merges that brought them in: the name from Git's merge message ("Merge branch 'feature/login' into develop", "Merge remote-tracking branch …", "Merge pull request #… from …"), otherwise from where they fork off and where they are merged (from production into production: hotfix; from development into production: release; from development into development: feature).
4. Commits no merge reaches (for example only tagged) are **unknown** and gray.

The tip of a deleted branch gets a faded label with its name (or its type), and the tooltip of every node and faded label says where the type comes from ("from the merge message", "inferred from the history"). Selection, menus, search and hide/solo work as usual; hidden branches are left out before the types are determined.

Limits: fast-forward, squash and rebase merges leave no trace of the branch, so those commits count as development; edited merge messages fall back to the shape of the history.

