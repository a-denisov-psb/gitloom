# Settings

**Preferences** (gear button or Ctrl+,) open as a window over the app (the whole window when it is small); categories are on the left. Changes apply immediately. **×** or Esc closes them; Tab stays inside.

![Preferences](../screenshots/m6-settings-general-dark.png)

| Category | Setting |
|---|---|
| General | **Auto-fetch interval** in minutes (default 5, 0 turns it off). **Prune remote branches on fetch** (on: fetch, auto-fetch and pull remove remote branches deleted on the remote; off: Git's `fetch.prune` applies). **Support long paths** (Windows: `core.longpaths` in the global Git config). **Default branch name** of new repositories (`init.defaultBranch`, global Git config; empty removes it). **Check for updates** (off by default, see [Privacy](privacy.md)). |
| Appearance | **Theme**: Dark, Light, Classic Dark (the look of older GitKraken versions: rows tinted in their lane color, filled branch labels joined to their commit) or System (follows the operating system; Dark if it reports nothing). **Zoom** 70–200 %. **Load avatars from Gravatar** (off by default). **Show toolbar labels** (on; off shows icons with the names as tooltips). **Show commit author / date and time / SHA in graph** (also in the gear menu above the graph). |
| Editor | For diffs, blame, file history and the conflict view (not the commit message fields): **Font** (the monospace fonts installed; Default is the system's), **Font size** 8–24, **Tab size** 1–8 (tabs are shown as that many columns; files are not changed), **Show line numbers**, **Word wrap** (long lines continue on the next rows in diffs, no horizontal scrolling then), **Syntax highlighting** (colors per theme; files over 1 MB or 20,000 lines stay plain). |
| Identities | Saved **identities** (name and email) for commits: **Add identity**, **Edit**, **Remove** and **Set as default**. **Keep the global Git config updated with the default identity** (on by default): setting or editing the default writes `user.name` and `user.email` to the global Git config; off, Gitloom never changes it. On first use Gitloom imports the identity of the global Git config as the first identity and default. |
| Signing | **Format** GPG (OpenPGP) or SSH; **Signing key** (GPG secret keys of the GPG program with expired ones marked, **Refresh**; SSH: public keys from `~/.ssh` or **Browse…**); **GPG program** (empty: `gpg`); **Sign commits by default**, **Sign tags by default**. All in the global Git config (`gpg.format`, `user.signingkey`, `gpg.program`, `commit.gpgSign`, `tag.gpgSign`); values the repository sets itself are listed as "Overridden in this repository". Passphrases are asked by GPG's pinentry or the SSH agent, never by Gitloom. |
| Templates | **Commit message prompt (all repositories)**: the instruction of "Copy commit message prompt…", with **Reset to default**. **Stash message**: **Use stash message template** (off: Git's default message), a preset such as `{branch} · {datetime}` or **Custom** with `{branch}` (`detached` without a branch), `{date}`, `{time}`, `{datetime}`, `{sha}`, `{summary}`; a preview shows the message for the active repository; an unknown placeholder or an empty custom text falls back to Git's default. |
| External tools | **Diff tool** and **merge tool**: Git's configuration (`diff.tool`, `merge.tool`) or a tool Git knows. **Terminal command** for Alt+T, with `{path}` for the repository folder; empty means the first terminal found. **External editor**: None, VS Code, VSCodium, Sublime Text, IntelliJ IDEA, Zed or Custom; a preset fills the command (`code {path}`), which you can edit; without `{path}` the path is added at the end. On Windows `code` finds `code.cmd`, and backslashes in paths are kept. **Delete .orig files after merging** (off; on: the merge tool keeps no `.orig` backup). |

Below the global categories, the group **Repository · <name>** holds the settings of the active tab's repository:

- **Identity**: the name and email the repository's commits use, with where they come from (repository or global Git config), and the same choices as "Committing as …" in the commit panel (see [Daily workflow](daily-workflow.md#committing)).
- **SSH key**: the key for fetch, pull and push (see [Branches and remotes](branches-and-remotes.md#ssh-key-per-repository)).
- **Commit**: the commit template like GitKraken's: **Apply this template to commit messages**, **Remove comments from commit messages** (lines starting with `core.commentChar`, `#` by default), **Summary**, description, **Save commit template**. It is Git's `commit.template` file; without one, saving creates `.git/gitloom/commit-template.txt` for this repository. A template in the global Git config is changed only after "This template is used by all repositories. Change it?".
- **External editor**: **Use global setting** or a preset or custom command for this repository only (stored in its Git config); Alt+E, the toolbar, the palette and the file menus use it.
- **Gitflow** (see [Git flow](git-flow.md)).

 The group follows the active tab; on the start screen its categories are disabled ("Open a repository first").

![About dialog](../screenshots/m6-about-dark.png)

At the bottom of the categories, **About Gitloom** shows the version, commit, license and third-party components.

## Zoom and fullscreen

Ctrl+= (or Ctrl++), Ctrl+- and Ctrl+0 zoom in 10 % steps from 70 to 200 %; the zoom is remembered. F11 toggles fullscreen.

## Files on disk

| File | Linux | macOS | Windows |
|---|---|---|---|
| Preferences, session (open tabs, recent repositories, hidden branches, update check) | `~/.config/gitloom/` | `~/Library/Application Support/Gitloom/` | `%APPDATA%\Gitloom\config\` |
| Crash reports | `~/.local/share/gitloom/crashes/` | `~/Library/Application Support/Gitloom/crashes/` | `%APPDATA%\Gitloom\data\crashes\` |
| Avatar cache | `~/.cache/gitloom/avatars/` | `~/Library/Caches/Gitloom/avatars/` | `%LOCALAPPDATA%\Gitloom\cache\avatars\` |

`settings.json` and `session.json` are plain JSON. A file that cannot be read is kept as `.bak` and defaults are used.
