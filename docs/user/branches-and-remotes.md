# Branches and remotes

## Checkout

- Double click a local branch (left panel or graph pill) to switch to it.
- Double click a remote branch to create its tracking branch and switch to it (an existing local branch with that name is checked out instead).
- **Checkout branch…** in the command palette lists all branches.
- Right click a commit → **Checkout this commit** for a detached HEAD (after a confirmation). Leaving a detached HEAD with commits that are on no branch or tag asks first and offers **Create branch**.

If local changes are in the way, Gitloom offers **Stash and checkout**; the stash stays in the list.

## Create, rename and delete branches

- **Branch** in the toolbar or Ctrl+B opens a name field at the selected commit (the WIP row means HEAD); also **Create branch here** in the commit menu. Invalid or taken names are flagged while you type; Enter creates and checks out the branch, Esc cancels.
- Right click a branch → **Rename…** (inline; with an upstream you can rename the remote branch too) or **Delete** (asks first; unmerged branches offer a force delete; the checked-out branch cannot be deleted). Remote branches can be deleted on the remote.

## Tags

**Create tag here** in the commit menu creates a lightweight tag, or an annotated one if you enter a message. Right click a tag to push it, delete it locally or on a remote, or copy its name. With several remotes Gitloom asks which one.

## Fetch, pull and push

- **Fetch all** (Pull dropdown or Ctrl+L) updates all remotes, with progress and **Cancel** in the status bar.
- **Auto-fetch** runs every 5 minutes for open repositories (Preferences → General; 0 turns it off). It skips busy repositories and merges or rebases in progress, and reports a failure once.
- **Pull** uses the mode chosen in its dropdown: fast-forward if possible, fast-forward only, or rebase; the last choice becomes the button's mode. Without an upstream, a dialog explains what to do; a pull that stops in a conflict offers to abort.
- **Push** sends the current branch to its upstream. Without an upstream, a dialog asks for remote and branch name and sets the upstream. A rejected push offers **Pull** or a force push (with lease, after a confirmation); a force push is refused if the remote changed since your last fetch.
- Pull and push also work for branches that are not checked out (pull only fast-forwards them), from the branch's context menu.

Fetch, pull and push cannot be undone.

## Upstream and remotes

- Branch menu: **Set upstream** / **Unset upstream** (also offered when Pull finds no upstream).
- **+** in the REMOTE section adds a remote (name and URL).
- Right click a remote: **Fetch**, **Edit URL**, **Rename**, **Remove**, **Hide all** / **Show all** branches, and **Open on GitHub** / **Open on GitLab** / **Open on Bitbucket** (**Open in browser** for other hosts) for remotes with a web address; local remotes (paths, `file://`) have no such entry. The command palette has **Open remote in browser** (origin, else the only remote, else a list).
- Adding, removing, renaming a remote, editing its URL and setting or unsetting an upstream can be undone.

Changes made outside Gitloom (new remotes, upstreams, `.git/config`) show up automatically.

## Selecting several branches and tags

Ctrl+click (⌘+click on macOS) adds or removes a branch or tag row in LOCAL, REMOTE, GITFLOW or TAGS; Shift+click selects all rows between the last clicked one and this one (rows in collapsed folders are left out). Esc clears the selection. Right click on a selected row: **Delete <n> branches…** (or tags, or branches and tags) and **Hide in graph** / **Show in graph**. The dialog lists everything grouped (local branches, remote branches per remote, tags); the checked-out branch is listed but skipped. Branches that are not merged are named in a second question (**Delete anyway**). Local branches and tags come back with one Undo; remote branches are deleted with one push per remote and cannot be undone. If something cannot be deleted, a notification lists it and the rest stays deleted.

## Submodules

The SUBMODULES section lists every submodule, nested ones indented under their parent, with an icon for its state (tooltip): up to date, **not initialized**, **out of date** (the checked-out commit differs from the one the repository records), **modified** (local changes) or **conflict**.

- Double click a submodule to open it in its own tab (or switch to that tab).
- Right click: **Update** (`git submodule update --init --recursive`, with progress and Cancel like fetch; credentials are asked the same way), **Open in new tab**, **Open in file manager**, **Copy path**. Right click on the section header: **Update all submodules**.
- After a checkout, pull, merge or rebase that leaves submodules out of date, a notification "Submodules are out of date" offers **Update**.

Adding, removing or moving submodules is done with the Git command line.

## SSH key per repository

Preferences → Repository → SSH key chooses the key Git uses for this repository's fetch, pull and push: **System default** (ssh-agent and `~/.ssh/config`), one of the keys in `~/.ssh` (files with a matching `.pub`), or any key file (**Browse…** or a typed path). Gitloom writes `core.sshCommand = ssh -i "<key>" -o IdentitiesOnly=yes` to the repository's Git config, so `git` in a terminal uses the same key; the key itself is never copied. A `core.sshCommand` you set yourself is shown and only replaced after **Replace…**; one from the global Git config is shown as inherited. A key with a passphrase asks for it in Gitloom's prompt; the passphrase is not stored.

## Credentials

Gitloom never stores credentials. Git's credential helpers and ssh-agent are used first; when they cannot answer, Gitloom shows a small prompt window (masked for passwords and passphrases, Yes/No for SSH host keys). See [Troubleshooting](troubleshooting.md#credentials).
